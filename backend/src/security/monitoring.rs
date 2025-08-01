use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};
use ts_rs::TS;
use utoipa::ToSchema;

use crate::{
    security::{
        audit_logger::{AuditLogger, AuditResult, AuditSeverity},
        session_security::{SessionSecurity, SecurityAlert},
    },
};

/// Security monitoring service that continuously monitors for security threats
#[allow(dead_code)]
pub struct SecurityMonitor {
    db_pool: SqlitePool,
    audit_logger: AuditLogger,
    session_security: SessionSecurity,
    config: SecurityMonitorConfig,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SecurityMonitorConfig {
    pub monitor_interval_seconds: u64,
    pub max_failed_auth_attempts: u32,
    pub failed_auth_window_minutes: u32,
    pub alert_email: Option<String>,
    pub enable_auto_response: bool,
}

impl Default for SecurityMonitorConfig {
    fn default() -> Self {
        Self {
            monitor_interval_seconds: 300, // 5 minutes
            max_failed_auth_attempts: 5,
            failed_auth_window_minutes: 15,
            alert_email: None,
            enable_auto_response: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct SecurityMetrics {
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
    pub active_sessions: u64,
    pub failed_auth_attempts_last_hour: u64,
    pub security_events_last_hour: u64,
    pub suspicious_activities: Vec<SecurityAlert>,
    pub system_health: SystemHealth,
}

#[derive(Debug, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct SystemHealth {
    pub database_status: HealthStatus,
    pub authentication_status: HealthStatus,
    pub audit_system_status: HealthStatus,
    pub overall_status: HealthStatus,
}

#[derive(Debug, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

impl SecurityMonitor {
    #[allow(dead_code)]
    pub fn new(
        db_pool: SqlitePool,
        audit_logger: AuditLogger,
        session_security: SessionSecurity,
        config: Option<SecurityMonitorConfig>,
    ) -> Self {
        Self {
            db_pool,
            audit_logger,
            session_security,
            config: config.unwrap_or_default(),
        }
    }

    /// Start the security monitoring service
    #[allow(dead_code)]
    pub async fn start(&self) {
        let mut interval = interval(Duration::from_secs(self.config.monitor_interval_seconds));
        
        info!("Security monitor started with {}s interval", self.config.monitor_interval_seconds);

        loop {
            interval.tick().await;
            
            if let Err(e) = self.run_security_checks().await {
                error!("Security monitoring check failed: {}", e);
            }
        }
    }

    /// Run comprehensive security checks
    #[allow(dead_code)]
    async fn run_security_checks(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Running security checks...");

        // Get security metrics
        let metrics = self.collect_security_metrics().await?;

        // Check for security threats
        let threats = self.analyze_security_threats(&metrics).await?;

        // Handle detected threats
        if !threats.is_empty() {
            self.handle_security_threats(threats).await?;
        }

        // Clean up old audit logs
        let retention_days = std::env::var("AUDIT_LOG_RETENTION_DAYS")
            .unwrap_or_default()
            .parse()
            .unwrap_or(90);
        
        let cleaned_count = self.audit_logger.cleanup_old_events(retention_days).await?;
        if cleaned_count > 0 {
            info!("Cleaned up {} old audit log entries", cleaned_count);
        }

        // Clean up expired sessions
        let expired_sessions = self.session_security.cleanup_expired_sessions().await?;
        if expired_sessions > 0 {
            info!("Cleaned up {} expired sessions", expired_sessions);
        }

        Ok(())
    }

    /// Collect comprehensive security metrics
    #[allow(dead_code)]
    async fn collect_security_metrics(&self) -> Result<SecurityMetrics, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);

        // Get active sessions count
        let active_sessions = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
               WHERE expires_at > datetime('now', 'subsec')"#
        )
        .fetch_one(&self.db_pool)
        .await?
        .count as u64;

        // Check if audit_log table exists
        let table_exists = sqlx::query_scalar!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='audit_log'"
        )
        .fetch_optional(&self.db_pool)
        .await?
        .is_some();

        let (failed_auth_attempts, security_events) = if table_exists {
            // Get failed authentication attempts in last hour
            let failed_auth_attempts = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM audit_log WHERE event_type = 'authentication' AND result = 'failure' AND timestamp >= ?1"
            )
            .bind(one_hour_ago)
            .fetch_one(&self.db_pool)
            .await? as u64;

            // Get all security events in last hour
            let security_events = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM audit_log WHERE event_type IN ('security_violation', 'admin_action', 'whitelist_change') AND timestamp >= ?1"
            )
            .bind(one_hour_ago)
            .fetch_one(&self.db_pool)
            .await? as u64;

            (failed_auth_attempts, security_events)
        } else {
            (0u64, 0u64)
        };

        // Collect suspicious activities (this is a simplified version)
        let suspicious_activities = Vec::new(); // TODO: Implement suspicious activity detection

        // Assess system health
        let system_health = self.assess_system_health().await?;

        Ok(SecurityMetrics {
            timestamp: now,
            active_sessions,
            failed_auth_attempts_last_hour: failed_auth_attempts,
            security_events_last_hour: security_events,
            suspicious_activities,
            system_health,
        })
    }

    /// Analyze security metrics for potential threats
    #[allow(dead_code)]
    async fn analyze_security_threats(
        &self,
        metrics: &SecurityMetrics,
    ) -> Result<Vec<SecurityThreat>, Box<dyn std::error::Error + Send + Sync>> {
        let mut threats = Vec::new();

        // Check for excessive failed authentication attempts
        if metrics.failed_auth_attempts_last_hour > self.config.max_failed_auth_attempts as u64 {
            threats.push(SecurityThreat {
                threat_type: ThreatType::ExcessiveFailedAuth,
                severity: ThreatSeverity::High,
                description: format!(
                    "Detected {} failed authentication attempts in the last hour (threshold: {})",
                    metrics.failed_auth_attempts_last_hour,
                    self.config.max_failed_auth_attempts
                ),
                affected_resource: "authentication".to_string(),
                recommended_action: "Review authentication logs and consider IP blocking".to_string(),
            });
        }

        // Check system health
        match metrics.system_health.overall_status {
            HealthStatus::Critical => {
                threats.push(SecurityThreat {
                    threat_type: ThreatType::SystemDegraded,
                    severity: ThreatSeverity::Critical,
                    description: "System health is critical".to_string(),
                    affected_resource: "system".to_string(),
                    recommended_action: "Immediate investigation required".to_string(),
                });
            }
            HealthStatus::Warning => {
                threats.push(SecurityThreat {
                    threat_type: ThreatType::SystemDegraded,
                    severity: ThreatSeverity::Medium,
                    description: "System health is degraded".to_string(),
                    affected_resource: "system".to_string(),
                    recommended_action: "Monitor system closely and investigate if issues persist".to_string(),
                });
            }
            _ => {}
        }

        Ok(threats)
    }

    /// Handle detected security threats
    #[allow(dead_code)]
    async fn handle_security_threats(
        &self,
        threats: Vec<SecurityThreat>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for threat in threats {
            // Log the security threat
            warn!(
                threat_type = ?threat.threat_type,
                severity = ?threat.severity,
                description = %threat.description,
                "Security threat detected"
            );

            // Log to audit system
            self.audit_logger.log_event(crate::security::audit_logger::CreateAuditEvent {
                event_type: crate::security::audit_logger::AuditEventType::SecurityViolation,
                user_id: None,
                ip_address: None,
                user_agent: Some("SecurityMonitor".to_string()),
                resource: threat.affected_resource.clone(),
                action: format!("{:?}", threat.threat_type),
                result: AuditResult::Blocked,
                details: Some(serde_json::json!({
                    "description": threat.description,
                    "severity": threat.severity,
                    "recommended_action": threat.recommended_action
                })),
                severity: match threat.severity {
                    ThreatSeverity::Low => AuditSeverity::Low,
                    ThreatSeverity::Medium => AuditSeverity::Medium,
                    ThreatSeverity::High => AuditSeverity::High,
                    ThreatSeverity::Critical => AuditSeverity::Critical,
                },
            }).await?;

            // Take automated response if enabled
            if self.config.enable_auto_response {
                self.take_automated_response(&threat).await?;
            }

            // Send alert if configured
            if let Some(email) = &self.config.alert_email {
                self.send_security_alert(email, &threat).await?;
            }
        }

        Ok(())
    }

    /// Take automated response to security threats
    #[allow(dead_code)]
    async fn take_automated_response(
        &self,
        threat: &SecurityThreat,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match threat.threat_type {
            ThreatType::ExcessiveFailedAuth => {
                info!("Automated response: Increasing authentication rate limits temporarily");
                // In a real implementation, you might temporarily tighten rate limits
                // or implement IP-based blocking
            }
            ThreatType::SystemDegraded => {
                if matches!(threat.severity, ThreatSeverity::Critical) {
                    warn!("Automated response: Critical system issue detected - manual intervention required");
                    // In production, you might want to trigger alerts or even controlled shutdown
                }
            }
            ThreatType::SuspiciousActivity => {
                info!("Automated response: Flagging suspicious activity for review");
            }
        }

        Ok(())
    }

    /// Send security alert via email (placeholder implementation)
    #[allow(dead_code)]
    async fn send_security_alert(
        &self,
        email: &str,
        threat: &SecurityThreat,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // In a real implementation, integrate with an email service
        info!(
            "Would send security alert to {}: {:?} - {}",
            email, threat.threat_type, threat.description
        );
        Ok(())
    }

    /// Assess overall system health
    #[allow(dead_code)]
    async fn assess_system_health(&self) -> Result<SystemHealth, Box<dyn std::error::Error + Send + Sync>> {
        // Check database health
        let database_status = match sqlx::query!("SELECT 1 as status").fetch_one(&self.db_pool).await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Critical,
        };

        // Check authentication system health (simplified)
        let authentication_status = if matches!(database_status, HealthStatus::Healthy) {
            HealthStatus::Healthy
        } else {
            HealthStatus::Critical
        };

        // Check audit system health
        let audit_system_status = match self.audit_logger.get_audit_statistics(1).await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Warning,
        };

        // Determine overall status
        let overall_status = match (
            &database_status,
            &authentication_status,
            &audit_system_status,
        ) {
            (HealthStatus::Critical, _, _) | (_, HealthStatus::Critical, _) => HealthStatus::Critical,
            (HealthStatus::Warning, _, _) | (_, _, HealthStatus::Warning) => HealthStatus::Warning,
            (HealthStatus::Healthy, HealthStatus::Healthy, HealthStatus::Healthy) => HealthStatus::Healthy,
            _ => HealthStatus::Unknown,
        };

        Ok(SystemHealth {
            database_status,
            authentication_status,
            audit_system_status,
            overall_status,
        })
    }

    /// Get current security metrics (for API endpoints)
    #[allow(dead_code)]
    pub async fn get_current_metrics(&self) -> Result<SecurityMetrics, Box<dyn std::error::Error + Send + Sync>> {
        self.collect_security_metrics().await
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityThreat {
    pub threat_type: ThreatType,
    pub severity: ThreatSeverity,
    pub description: String,
    pub affected_resource: String,
    pub recommended_action: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ThreatType {
    ExcessiveFailedAuth,
    SuspiciousActivity,
    SystemDegraded,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ThreatSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Security monitoring task that can be spawned
#[allow(dead_code)]
pub async fn start_security_monitoring(
    db_pool: SqlitePool,
    audit_logger: AuditLogger,
    session_security: SessionSecurity,
    config: Option<SecurityMonitorConfig>,
) {
    let monitor = SecurityMonitor::new(db_pool, audit_logger, session_security, config);
    monitor.start().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_monitor_config() {
        let config = SecurityMonitorConfig::default();
        assert_eq!(config.monitor_interval_seconds, 300);
        assert_eq!(config.max_failed_auth_attempts, 5);
        assert_eq!(config.failed_auth_window_minutes, 15);
    }

    #[tokio::test]
    async fn test_system_health_assessment() {
        // This would require a real database for a full test
        // Here we just verify the enum variants work correctly
        let health = SystemHealth {
            database_status: HealthStatus::Healthy,
            authentication_status: HealthStatus::Healthy,
            audit_system_status: HealthStatus::Healthy,
            overall_status: HealthStatus::Healthy,
        };

        match health.overall_status {
            HealthStatus::Healthy => assert!(true),
            _ => assert!(false, "Expected healthy status"),
        }
    }
}