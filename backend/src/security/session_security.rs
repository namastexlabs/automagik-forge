use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    auth::{generate_jwt_token, hash_token, JwtConfig},
    models::{
        user::User,
        user_session::{SessionType, UserSession},
    },
    security::audit_logger::{AuditLogger, AuditResult, CreateAuditEvent, AuditEventType, AuditSeverity},
};

#[derive(Error, Debug)]
pub enum SessionSecurityError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    #[error("Session not found")]
    SessionNotFound,
    #[error("Session expired")]
    SessionExpired,
    #[error("Invalid session")]
    InvalidSession,
    #[error("Token rotation failed: {0}")]
    TokenRotationFailed(String),
    #[error("Concurrent session limit exceeded")]
    ConcurrentSessionLimit,
}

/// Session security configuration
#[derive(Debug, Clone)]
pub struct SessionSecurityConfig {
    pub max_concurrent_web_sessions: u32,
    pub max_concurrent_mcp_sessions: u32,
    pub token_rotation_threshold_hours: i64,
    pub cleanup_interval_hours: i64,
    pub force_logout_on_security_event: bool,
}

impl Default for SessionSecurityConfig {
    fn default() -> Self {
        Self {
            max_concurrent_web_sessions: 3,
            max_concurrent_mcp_sessions: 5,
            token_rotation_threshold_hours: 12, // Rotate tokens after 12 hours
            cleanup_interval_hours: 1, // Clean up expired sessions every hour
            force_logout_on_security_event: true,
        }
    }
}

/// Enhanced session security manager
pub struct SessionSecurity {
    db_pool: SqlitePool,
    jwt_config: JwtConfig,
    audit_logger: AuditLogger,
    config: SessionSecurityConfig,
}

impl SessionSecurity {
    pub fn new(
        db_pool: SqlitePool,
        jwt_config: JwtConfig,
        audit_logger: AuditLogger,
        config: Option<SessionSecurityConfig>,
    ) -> Self {
        Self {
            db_pool,
            jwt_config,
            audit_logger,
            config: config.unwrap_or_default(),
        }
    }

    /// Create a new secure session with concurrent session limits
    pub async fn create_session(
        &self,
        user_id: Uuid,
        session_type: SessionType,
        client_info: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(String, UserSession), SessionSecurityError> {
        // Check concurrent session limits
        self.enforce_concurrent_session_limits(user_id, session_type).await?;

        // Generate JWT token
        let session_id = Uuid::new_v4();
        let jwt_token = generate_jwt_token(user_id, session_id, session_type, &self.jwt_config)?;
        let token_hash = hash_token(&jwt_token);

        // Create session in database
        let session = UserSession::create_with_defaults(
            &self.db_pool,
            user_id,
            token_hash,
            session_type,
            client_info,
        ).await?;

        // Log session creation
        self.audit_logger.log_authentication(
            Some(user_id),
            ip_address,
            user_agent,
            "session_created",
            AuditResult::Success,
            Some(serde_json::json!({
                "session_id": session_id,
                "session_type": session_type
            })),
        ).await?;

        info!("New session created for user {} (type: {:?})", user_id, session_type);

        Ok((jwt_token, session))
    }

    /// Rotate JWT token for an existing session
    pub async fn rotate_token(
        &self,
        current_token: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<String, SessionSecurityError> {
        let token_hash = hash_token(current_token);

        // Find current session
        let session = UserSession::find_valid_by_token_hash(&self.db_pool, &token_hash)
            .await?
            .ok_or(SessionSecurityError::SessionNotFound)?;

        // Generate new JWT token
        let new_jwt_token = generate_jwt_token(
            session.user_id,
            session.id,
            session.session_type,
            &self.jwt_config,
        )?;
        let new_token_hash = hash_token(&new_jwt_token);

        // Update session with new token hash
        sqlx::query!(
            "UPDATE user_sessions SET token_hash = $1 WHERE id = $2",
            new_token_hash,
            session.id
        )
        .execute(&self.db_pool)
        .await?;

        // Log token rotation
        self.audit_logger.log_event(CreateAuditEvent {
            event_type: AuditEventType::TokenAccess,
            user_id: Some(session.user_id),
            ip_address,
            user_agent,
            resource: "session_token".to_string(),
            action: "token_rotated".to_string(),
            result: AuditResult::Success,
            details: Some(serde_json::json!({
                "session_id": session.id,
                "session_type": session.session_type
            })),
            severity: AuditSeverity::Low,
        }).await?;

        info!("Token rotated for session {} (user: {})", session.id, session.user_id);

        Ok(new_jwt_token)
    }

    /// Check if a session needs token rotation
    pub async fn needs_token_rotation(&self, session: &UserSession) -> bool {
        let rotation_threshold = Duration::hours(self.config.token_rotation_threshold_hours);
        let session_age = Utc::now() - session.created_at;
        session_age >= rotation_threshold
    }

    /// Revoke a specific session
    pub async fn revoke_session(
        &self,
        session_id: Uuid,
        reason: &str,
        admin_user_id: Option<Uuid>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), SessionSecurityError> {
        // Get session details for logging
        let session = sqlx::query_as!(
            UserSession,
            r#"SELECT 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                token_hash, 
                session_type as "session_type!: SessionType", 
                client_info, 
                expires_at as "expires_at!: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>" 
            FROM user_sessions WHERE id = $1"#,
            session_id
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(SessionSecurityError::SessionNotFound)?;

        // Delete the session
        UserSession::delete(&self.db_pool, session_id).await?;

        // Log session revocation
        if let Some(admin_id) = admin_user_id {
            self.audit_logger.log_admin_action(
                admin_id,
                ip_address,
                user_agent,
                "session",
                "revoke_session",
                Some(session.user_id),
                AuditResult::Success,
                Some(serde_json::json!({
                    "session_id": session_id,
                    "reason": reason
                })),
            ).await?;
        } else {
            self.audit_logger.log_authentication(
                Some(session.user_id),
                ip_address,
                user_agent,
                "session_revoked",
                AuditResult::Success,
                Some(serde_json::json!({
                    "session_id": session_id,
                    "reason": reason
                })),
            ).await?;
        }

        info!("Session {} revoked for user {} (reason: {})", session_id, session.user_id, reason);

        Ok(())
    }

    /// Revoke all sessions for a user (e.g., on security incident)
    pub async fn revoke_all_user_sessions(
        &self,
        user_id: Uuid,
        reason: &str,
        admin_user_id: Option<Uuid>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<u64, SessionSecurityError> {
        let sessions_deleted = UserSession::delete_all_for_user(&self.db_pool, user_id).await?;

        // Log mass session revocation
        if let Some(admin_id) = admin_user_id {
            self.audit_logger.log_admin_action(
                admin_id,
                ip_address,
                user_agent,
                "sessions",
                "revoke_all_sessions",
                Some(user_id),
                AuditResult::Success,
                Some(serde_json::json!({
                    "sessions_revoked": sessions_deleted,
                    "reason": reason
                })),
            ).await?;
        } else {
            self.audit_logger.log_authentication(
                Some(user_id),
                ip_address,
                user_agent,
                "all_sessions_revoked",
                AuditResult::Success,
                Some(serde_json::json!({
                    "sessions_revoked": sessions_deleted,
                    "reason": reason
                })),
            ).await?;
        }

        warn!("All sessions ({}) revoked for user {} (reason: {})", sessions_deleted, user_id, reason);

        Ok(sessions_deleted)
    }

    /// Enforce concurrent session limits
    async fn enforce_concurrent_session_limits(
        &self,
        user_id: Uuid,
        session_type: SessionType,
    ) -> Result<(), SessionSecurityError> {
        let max_sessions = match session_type {
            SessionType::Web => self.config.max_concurrent_web_sessions,
            SessionType::Mcp => self.config.max_concurrent_mcp_sessions,
        };

        let current_session_count = UserSession::count_active_by_user_and_type(
            &self.db_pool,
            user_id,
            session_type,
        ).await? as u32;

        if current_session_count >= max_sessions {
            // Remove oldest sessions to make room
            let sessions_to_remove = current_session_count - max_sessions + 1;
            self.remove_oldest_sessions(user_id, session_type, sessions_to_remove).await?;
        }

        Ok(())
    }

    /// Remove oldest sessions for a user
    async fn remove_oldest_sessions(
        &self,
        user_id: Uuid,
        session_type: SessionType,
        count: u32,
    ) -> Result<(), SessionSecurityError> {
        let session_type_str = match session_type {
            SessionType::Web => "web",
            SessionType::Mcp => "mcp",
        };

        let oldest_sessions = sqlx::query!(
            r#"SELECT id as "id!: Uuid" FROM user_sessions 
               WHERE user_id = $1 AND session_type = $2 AND expires_at > datetime('now', 'subsec')
               ORDER BY created_at ASC 
               LIMIT $3"#,
            user_id,
            session_type_str,
            count as i64
        )
        .fetch_all(&self.db_pool)
        .await?;

        for session in oldest_sessions {
            self.revoke_session(
                session.id,
                "concurrent_session_limit",
                None,
                None,
                None,
            ).await?;
        }

        Ok(())
    }

    /// Clean up expired sessions and log cleanup activity
    pub async fn cleanup_expired_sessions(&self) -> Result<u64, SessionSecurityError> {
        let deleted_count = UserSession::cleanup_expired(&self.db_pool).await?;

        if deleted_count > 0 {
            info!("Cleaned up {} expired sessions", deleted_count);
        }

        Ok(deleted_count)
    }

    /// Get session security metrics
    pub async fn get_session_metrics(&self) -> Result<SessionMetrics, SessionSecurityError> {
        let total_active_sessions = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
               WHERE expires_at > datetime('now', 'subsec')"#
        )
        .fetch_one(&self.db_pool)
        .await?
        .count;

        let web_sessions = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
               WHERE expires_at > datetime('now', 'subsec') AND session_type = 'web'"#
        )
        .fetch_one(&self.db_pool)
        .await?
        .count;

        let mcp_sessions = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
               WHERE expires_at > datetime('now', 'subsec') AND session_type = 'mcp'"#
        )
        .fetch_one(&self.db_pool)
        .await?
        .count;

        let sessions_created_today = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
               WHERE created_at >= datetime('now', '-1 day')"#
        )
        .fetch_one(&self.db_pool)
        .await?
        .count;

        Ok(SessionMetrics {
            total_active_sessions: total_active_sessions as u64,
            web_sessions: web_sessions as u64,
            mcp_sessions: mcp_sessions as u64,
            sessions_created_today: sessions_created_today as u64,
        })
    }

    /// Detect suspicious session activity
    pub async fn detect_suspicious_activity(&self, user_id: Uuid) -> Result<Vec<SecurityAlert>, SessionSecurityError> {
        let mut alerts = Vec::new();

        // Check for too many sessions from different IPs
        // This would require storing IP addresses in sessions - for now, just check session count
        let session_count = UserSession::count_active_by_user_and_type(&self.db_pool, user_id, SessionType::Web).await?;
        
        if session_count > self.config.max_concurrent_web_sessions as i64 {
            alerts.push(SecurityAlert {
                alert_type: SecurityAlertType::ExcessiveSessions,
                description: format!("User has {} active sessions", session_count),
                severity: AuditSeverity::Medium,
                user_id,
            });
        }

        // Check for very old sessions that haven't been rotated
        let old_sessions = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
               WHERE user_id = $1 AND expires_at > datetime('now', 'subsec') 
               AND created_at < datetime('now', '-7 days')"#,
            user_id
        )
        .fetch_one(&self.db_pool)
        .await?
        .count;

        if old_sessions > 0 {
            alerts.push(SecurityAlert {
                alert_type: SecurityAlertType::StaleTokens,
                description: format!("User has {} sessions older than 7 days", old_sessions),
                severity: AuditSeverity::Low,
                user_id,
            });
        }

        Ok(alerts)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub total_active_sessions: u64,
    pub web_sessions: u64,
    pub mcp_sessions: u64,
    pub sessions_created_today: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityAlert {
    pub alert_type: SecurityAlertType,
    pub description: String,
    pub severity: AuditSeverity,
    pub user_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SecurityAlertType {
    ExcessiveSessions,
    StaleTokens,
    SuspiciousActivity,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user_session::{CreateUserSession, SessionType};

    #[tokio::test]
    async fn test_session_security_config() {
        let config = SessionSecurityConfig::default();
        assert_eq!(config.max_concurrent_web_sessions, 3);
        assert_eq!(config.max_concurrent_mcp_sessions, 5);
        assert_eq!(config.token_rotation_threshold_hours, 12);
    }

    #[test]
    fn test_needs_token_rotation() {
        // This would require a real database for a full test
        // Here we just test that the configuration is correct
        let config = SessionSecurityConfig::default();
        assert!(config.token_rotation_threshold_hours > 0);
    }
}