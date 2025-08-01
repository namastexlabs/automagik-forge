use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use ts_rs::TS;
use uuid::Uuid;
use utoipa::ToSchema;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, sqlx::Type)]
#[sqlx(type_name = "audit_event_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    AdminAction,
    UserManagement,
    WhitelistChange,
    TokenAccess,
    SecurityViolation,
    ConfigChange,
    DataAccess,
    RateLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, sqlx::Type)]
#[sqlx(type_name = "audit_result", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AuditResult {
    Success,
    Failure,
    Error,
    Blocked,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct AuditEvent {
    pub id: Uuid,
    pub event_type: AuditEventType,
    pub user_id: Option<Uuid>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub resource: String,
    pub action: String,
    pub result: AuditResult,
    pub details: Option<String>, // JSON serialized details
    pub severity: AuditSeverity,
    
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, sqlx::Type)]
#[sqlx(type_name = "audit_severity", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AuditSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateAuditEvent {
    pub event_type: AuditEventType,
    pub user_id: Option<Uuid>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub resource: String,
    pub action: String,
    pub result: AuditResult,
    #[ts(type = "any")]
    pub details: Option<serde_json::Value>,
    pub severity: AuditSeverity,
}

/// Audit logger for security-relevant events
#[derive(Debug, Clone)]
pub struct AuditLogger {
    db_pool: SqlitePool,
}

impl AuditLogger {
    pub fn new(db_pool: SqlitePool) -> Self {
        Self { db_pool }
    }

    /// Log an audit event
    pub async fn log_event(&self, event: CreateAuditEvent) -> Result<Uuid, sqlx::Error> {
        let event_id = Uuid::new_v4();
        let details_json = event.details.map(|d| d.to_string());
        
        // Convert enums to strings for database storage
        let event_type_str = match event.event_type {
            AuditEventType::Authentication => "authentication",
            AuditEventType::Authorization => "authorization",
            AuditEventType::AdminAction => "admin_action",
            AuditEventType::UserManagement => "user_management",
            AuditEventType::WhitelistChange => "whitelist_change",
            AuditEventType::TokenAccess => "token_access",
            AuditEventType::SecurityViolation => "security_violation",
            AuditEventType::ConfigChange => "config_change",
            AuditEventType::DataAccess => "data_access",
        };

        let result_str = match event.result {
            AuditResult::Success => "success",
            AuditResult::Failure => "failure",
            AuditResult::Error => "error",
            AuditResult::Blocked => "blocked",
        };

        let severity_str = match event.severity {
            AuditSeverity::Low => "low",
            AuditSeverity::Medium => "medium",
            AuditSeverity::High => "high",
            AuditSeverity::Critical => "critical",
        };

        // Clone values for logging before they get moved into the database query
        let ip_address_log = event.ip_address.clone();
        let resource_log = event.resource.clone();
        let action_log = event.action.clone();

        // Check if audit_log table exists before trying to insert
        let table_exists = sqlx::query_scalar!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='audit_log'"
        )
        .fetch_optional(&self.db_pool)
        .await?
        .is_some();

        if table_exists {
            sqlx::query(
                r#"INSERT INTO audit_log (
                    id, event_type, user_id, ip_address, user_agent, 
                    resource, action, result, details, severity, timestamp
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#
            )
            .bind(event_id)
            .bind(event_type_str)
            .bind(event.user_id)
            .bind(event.ip_address)
            .bind(event.user_agent)
            .bind(event.resource)
            .bind(event.action)
            .bind(result_str)
            .bind(details_json)
            .bind(severity_str)
            .bind(Utc::now())
            .execute(&self.db_pool)
            .await?;
        } else {
            tracing::debug!("Audit log table does not exist, skipping database logging");
        }

        // Also log to structured logging for immediate visibility
        let _log_level = match event.severity {
            AuditSeverity::Critical => tracing::Level::ERROR,
            AuditSeverity::High => tracing::Level::WARN,
            AuditSeverity::Medium => tracing::Level::INFO,
            AuditSeverity::Low => tracing::Level::DEBUG,
        };

        // Use a regular tracing macro instead of the dynamic level to avoid const issues
        match event.severity {
            AuditSeverity::Critical => {
                tracing::error!(
                    event_id = %event_id,
                    event_type = %event_type_str,
                    user_id = ?event.user_id,
                    ip_address = ?ip_address_log,
                    resource = %resource_log,
                    action = %action_log,
                    result = %result_str,
                    severity = %severity_str,
                    "Audit event logged"
                );
            },
            AuditSeverity::High => {
                tracing::warn!(
                    event_id = %event_id,
                    event_type = %event_type_str,
                    user_id = ?event.user_id,
                    ip_address = ?ip_address_log,
                    resource = %resource_log,
                    action = %action_log,
                    result = %result_str,
                    severity = %severity_str,
                    "Audit event logged"
                );
            },
            AuditSeverity::Medium => {
                tracing::info!(
                    event_id = %event_id,
                    event_type = %event_type_str,
                    user_id = ?event.user_id,
                    ip_address = ?ip_address_log,
                    resource = %resource_log,
                    action = %action_log,
                    result = %result_str,
                    severity = %severity_str,
                    "Audit event logged"
                );
            },
            AuditSeverity::Low => {
                tracing::debug!(
                    event_id = %event_id,
                    event_type = %event_type_str,
                    user_id = ?event.user_id,
                    ip_address = ?ip_address_log,
                    resource = %resource_log,
                    action = %action_log,
                    result = %result_str,
                    severity = %severity_str,
                    "Audit event logged"
                );
            },
        }

        Ok(event_id)
    }

    /// Log authentication event
    pub async fn log_authentication(
        &self,
        user_id: Option<Uuid>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        action: &str,
        result: AuditResult,
        details: Option<serde_json::Value>,
    ) -> Result<Uuid, sqlx::Error> {
        let severity = match result {
            AuditResult::Success => AuditSeverity::Low,
            AuditResult::Failure => AuditSeverity::Medium,
            AuditResult::Error => AuditSeverity::High,
            AuditResult::Blocked => AuditSeverity::High,
        };

        self.log_event(CreateAuditEvent {
            event_type: AuditEventType::Authentication,
            user_id,
            ip_address,
            user_agent,
            resource: "auth".to_string(),
            action: action.to_string(),
            result,
            details,
            severity,
        }).await
    }

    /// Log admin action
    pub async fn log_admin_action(
        &self,
        admin_user_id: Uuid,
        ip_address: Option<String>,
        user_agent: Option<String>,
        resource: &str,
        action: &str,
        target_user_id: Option<Uuid>,
        result: AuditResult,
        details: Option<serde_json::Value>,
    ) -> Result<Uuid, sqlx::Error> {
        let mut audit_details = HashMap::new();
        if let Some(target_id) = target_user_id {
            audit_details.insert("target_user_id".to_string(), serde_json::Value::String(target_id.to_string()));
        }
        if let Some(existing_details) = details {
            if let serde_json::Value::Object(map) = existing_details {
                audit_details.extend(map);
            }
        }

        let combined_details = if audit_details.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(serde_json::Map::from_iter(audit_details)))
        };

        let severity = match result {
            AuditResult::Success => AuditSeverity::Medium,
            AuditResult::Failure => AuditSeverity::High,
            AuditResult::Error => AuditSeverity::High,
            AuditResult::Blocked => AuditSeverity::Critical,
        };

        self.log_event(CreateAuditEvent {
            event_type: AuditEventType::AdminAction,
            user_id: Some(admin_user_id),
            ip_address,
            user_agent,
            resource: resource.to_string(),
            action: action.to_string(),
            result,
            details: combined_details,
            severity,
        }).await
    }

    /// Log whitelist change
    pub async fn log_whitelist_change(
        &self,
        admin_user_id: Uuid,
        ip_address: Option<String>,
        user_agent: Option<String>,
        action: &str, // add, remove, update
        target_github_id: Option<i64>,
        target_username: Option<&str>,
        result: AuditResult,
    ) -> Result<Uuid, sqlx::Error> {
        let mut details = HashMap::new();
        if let Some(github_id) = target_github_id {
            details.insert("target_github_id".to_string(), serde_json::Value::Number(github_id.into()));
        }
        if let Some(username) = target_username {
            details.insert("target_username".to_string(), serde_json::Value::String(username.to_string()));
        }

        let audit_details = if details.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(serde_json::Map::from_iter(details)))
        };

        self.log_event(CreateAuditEvent {
            event_type: AuditEventType::WhitelistChange,
            user_id: Some(admin_user_id),
            ip_address,
            user_agent,
            resource: "whitelist".to_string(),
            action: action.to_string(),
            result,
            details: audit_details,
            severity: AuditSeverity::High, // Whitelist changes are always high severity
        }).await
    }


    /// Log security violation
    pub async fn log_security_violation(
        &self,
        user_id: Option<Uuid>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        violation_type: &str,
        resource: &str,
        details: Option<serde_json::Value>,
    ) -> Result<Uuid, sqlx::Error> {
        self.log_event(CreateAuditEvent {
            event_type: AuditEventType::SecurityViolation,
            user_id,
            ip_address,
            user_agent,
            resource: resource.to_string(),
            action: violation_type.to_string(),
            result: AuditResult::Blocked,
            details,
            severity: AuditSeverity::Critical,
        }).await
    }

    /// Log token access (when GitHub tokens are decrypted for use)
    pub async fn log_token_access(
        &self,
        user_id: Uuid,
        ip_address: Option<String>,
        user_agent: Option<String>,
        action: &str, // decrypt, rotate, revoke
        result: AuditResult,
    ) -> Result<Uuid, sqlx::Error> {
        self.log_event(CreateAuditEvent {
            event_type: AuditEventType::TokenAccess,
            user_id: Some(user_id),
            ip_address,
            user_agent,
            resource: "github_token".to_string(),
            action: action.to_string(),
            result,
            details: None, // Don't log actual token data
            severity: AuditSeverity::Medium,
        }).await
    }

    /// Get audit events with filtering
    pub async fn get_audit_events(
        &self,
        user_id: Option<Uuid>,
        event_type: Option<AuditEventType>,
        severity: Option<AuditSeverity>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<AuditEvent>, sqlx::Error> {
        let limit = limit.unwrap_or(100).min(1000); // Cap at 1000
        let offset = offset.unwrap_or(0);

        let event_type_str = event_type.map(|et| match et {
            AuditEventType::Authentication => "authentication",
            AuditEventType::Authorization => "authorization",
            AuditEventType::AdminAction => "admin_action",
            AuditEventType::UserManagement => "user_management",
            AuditEventType::WhitelistChange => "whitelist_change",
            AuditEventType::TokenAccess => "token_access",
            AuditEventType::SecurityViolation => "security_violation",
            AuditEventType::ConfigChange => "config_change",
            AuditEventType::DataAccess => "data_access",
        });

        let severity_str = severity.map(|s| match s {
            AuditSeverity::Low => "low",
            AuditSeverity::Medium => "medium",
            AuditSeverity::High => "high",
            AuditSeverity::Critical => "critical",
        });

        let mut query = "SELECT id, event_type, user_id, ip_address, user_agent, resource, action, result, details, severity, timestamp FROM audit_log WHERE 1=1".to_string();
        let mut bind_count = 0;

        if user_id.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND user_id = ${}", bind_count));
        }

        if event_type_str.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND event_type = ${}", bind_count));
        }

        if severity_str.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND severity = ${}", bind_count));
        }

        query.push_str(" ORDER BY timestamp DESC");
        query.push_str(&format!(" LIMIT ${} OFFSET ${}", bind_count + 1, bind_count + 2));

        let mut query_builder = sqlx::query_as::<_, AuditEvent>(&query);

        if let Some(uid) = user_id {
            query_builder = query_builder.bind(uid);
        }
        if let Some(et) = event_type_str {
            query_builder = query_builder.bind(et);
        }
        if let Some(sev) = severity_str {
            query_builder = query_builder.bind(sev);
        }

        query_builder = query_builder.bind(limit as i64).bind(offset as i64);

        query_builder.fetch_all(&self.db_pool).await
    }

    /// Clean up old audit logs (retention policy)
    pub async fn cleanup_old_events(&self, retention_days: u32) -> Result<u64, sqlx::Error> {
        // Check if audit_log table exists
        let table_exists = sqlx::query_scalar!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='audit_log'"
        )
        .fetch_optional(&self.db_pool)
        .await?
        .is_some();

        if !table_exists {
            return Ok(0);
        }

        let cutoff_date = Utc::now() - chrono::Duration::days(retention_days as i64);
        
        let result = sqlx::query("DELETE FROM audit_log WHERE timestamp < ?1")
            .bind(cutoff_date)
            .execute(&self.db_pool)
            .await?;

        let deleted_count = result.rows_affected();
        
        if deleted_count > 0 {
            info!("Cleaned up {} old audit log entries older than {} days", deleted_count, retention_days);
        }

        Ok(deleted_count)
    }

    /// Get audit statistics
    pub async fn get_audit_statistics(&self, days: u32) -> Result<AuditStatistics, sqlx::Error> {
        // Check if audit_log table exists
        let table_exists = sqlx::query_scalar!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='audit_log'"
        )
        .fetch_optional(&self.db_pool)
        .await?
        .is_some();

        if !table_exists {
            return Ok(AuditStatistics {
                total_events: 0,
                failed_auth_attempts: 0,
                security_violations: 0,
                admin_actions: 0,
            });
        }

        let since_date = Utc::now() - chrono::Duration::days(days as i64);

        let total_events = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_log WHERE timestamp >= ?1")
            .bind(since_date)
            .fetch_one(&self.db_pool)
            .await?;

        let failed_auth_attempts = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_log WHERE timestamp >= ?1 AND event_type = 'authentication' AND result = 'failure'"
        )
        .bind(since_date)
        .fetch_one(&self.db_pool)
        .await?;

        let rate_limit_violations = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_log WHERE timestamp >= ?1 AND event_type = 'rate_limit'"
        )
        .bind(since_date)
        .fetch_one(&self.db_pool)
        .await?;

        let security_violations = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_log WHERE timestamp >= ?1 AND event_type = 'security_violation'"
        )
        .bind(since_date)
        .fetch_one(&self.db_pool)
        .await?;

        let admin_actions = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_log WHERE timestamp >= ?1 AND event_type = 'admin_action'"
        )
        .bind(since_date)
        .fetch_one(&self.db_pool)
        .await?;

        Ok(AuditStatistics {
            total_events: total_events as u64,
            failed_auth_attempts: failed_auth_attempts as u64,
            rate_limit_violations: rate_limit_violations as u64,
            security_violations: security_violations as u64,
            admin_actions: admin_actions as u64,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct AuditStatistics {
    pub total_events: u64,
    pub failed_auth_attempts: u64,
    pub rate_limit_violations: u64,
    pub security_violations: u64,
    pub admin_actions: u64,
}

/// Helper function to extract request context for audit logging
pub fn extract_request_context(
    headers: &axum::http::HeaderMap,
) -> (Option<String>, Option<String>) {
    let ip_address = headers.get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .or_else(|| headers.get("cf-connecting-ip"))
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());

    let user_agent = headers.get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    (ip_address, user_agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[sqlx::test]
    async fn test_audit_logger_basic_functionality(pool: SqlitePool) -> sqlx::Result<()> {
        // Create audit_log table for testing
        sqlx::query!(
            r#"CREATE TABLE audit_log (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                user_id TEXT,
                ip_address TEXT,
                user_agent TEXT,
                resource TEXT NOT NULL,
                action TEXT NOT NULL,
                result TEXT NOT NULL,
                details TEXT,
                severity TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )"#
        )
        .execute(&pool)
        .await?;

        let logger = AuditLogger::new(pool);
        let user_id = Uuid::new_v4();

        // Test logging authentication event
        let event_id = logger.log_authentication(
            Some(user_id),
            Some("192.168.1.1".to_string()),
            Some("Test User Agent".to_string()),
            "login",
            AuditResult::Success,
            Some(serde_json::json!({"method": "github_oauth"})),
        ).await?;

        assert!(!event_id.is_nil());

        // Test logging admin action
        let admin_event_id = logger.log_admin_action(
            user_id,
            Some("192.168.1.1".to_string()),
            Some("Admin User Agent".to_string()),
            "users",
            "whitelist_user",
            Some(Uuid::new_v4()),
            AuditResult::Success,
            Some(serde_json::json!({"github_username": "testuser"})),
        ).await?;

        assert!(!admin_event_id.is_nil());

        // Test retrieving events
        let events = logger.get_audit_events(
            Some(user_id),
            None,
            None,
            Some(10),
            None,
        ).await?;

        assert_eq!(events.len(), 2);

        Ok(())
    }

    #[test]
    fn test_extract_request_context() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());
        headers.insert("user-agent", "Test/1.0".parse().unwrap());

        let (ip, user_agent) = extract_request_context(&headers);

        assert_eq!(ip, Some("192.168.1.1".to_string()));
        assert_eq!(user_agent, Some("Test/1.0".to_string()));
    }
}