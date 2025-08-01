use axum::{extract::State, response::Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::{self, ToSchema};

use crate::{
    app_state::AppState,
    models::ApiResponse,
    security::monitoring::{HealthStatus, SecurityMetrics},
};

#[derive(Debug, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct HealthCheckResponse {
    pub status: String,
    pub database: String,
    pub authentication: String,
    pub mcp_server: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub active_sessions: u64,
    
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct DetailedHealthResponse {
    pub basic_health: HealthCheckResponse,
    pub security_metrics: Option<SecurityMetrics>,
    pub system_status: SystemStatus,
}

#[derive(Debug, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct SystemStatus {
    pub memory_usage_mb: Option<u64>,
    pub cpu_usage_percent: Option<f64>,
    pub disk_usage_percent: Option<f64>,
    pub database_size_mb: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Basic health check", body = ApiResponse<HealthCheckResponse>),
        (status = 503, description = "Service unavailable", body = ApiResponse<String>)
    ),
    tag = "health"
)]
pub async fn health_check(
    State(app_state): State<AppState>,
) -> Json<ApiResponse<HealthCheckResponse>> {
    // Start time for uptime calculation (simplified - would use actual start time in production)
    let _start_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Check database connectivity
    let database_status = match sqlx::query!("SELECT 1 as status").fetch_one(&app_state.db_pool).await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    // Get active sessions count
    let active_sessions = match sqlx::query!(
        r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
           WHERE expires_at > datetime('now', 'subsec')"#
    ).fetch_one(&app_state.db_pool).await {
        Ok(result) => result.count as u64,
        Err(_) => 0,
    };

    // Simple uptime calculation (in production, track actual application start time)
    let uptime_seconds = 0; // Placeholder

    let health_response = HealthCheckResponse {
        status: if database_status == "connected" { "healthy" } else { "unhealthy" }.to_string(),
        database: database_status.to_string(),
        authentication: if database_status == "connected" { "operational" } else { "degraded" }.to_string(),
        mcp_server: "running".to_string(), // Simplified check
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        active_sessions,
        timestamp: Utc::now(),
    };

    Json(ApiResponse::success(health_response))
}

#[utoipa::path(
    get,
    path = "/api/health/detailed",
    responses(
        (status = 200, description = "Detailed health check with security metrics", body = ApiResponse<DetailedHealthResponse>),
        (status = 503, description = "Service unavailable", body = ApiResponse<String>)
    ),
    tag = "health",
    security(
        ("Bearer" = [])
    )
)]
pub async fn detailed_health_check(
    State(app_state): State<AppState>,
) -> Json<ApiResponse<DetailedHealthResponse>> {
    // Get basic health first
    let _basic_health_response = health_check(State(app_state.clone())).await;
    // We don't need to extract the inner response, just reconstruct the health data
    
    // For now, we'll reconstruct the health data since the API response fields are private
    // This is a temporary solution - in production, you'd want to refactor ApiResponse to have getters
    let health_response = HealthCheckResponse {
        status: if sqlx::query!("SELECT 1 as status").fetch_one(&app_state.db_pool).await.is_ok() { 
            "healthy".to_string() 
        } else { 
            "unhealthy".to_string() 
        },
        database: if sqlx::query!("SELECT 1 as status").fetch_one(&app_state.db_pool).await.is_ok() { 
            "connected".to_string() 
        } else { 
            "disconnected".to_string() 
        },
        authentication: "operational".to_string(),
        mcp_server: "running".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: 0,
        active_sessions: match sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
               WHERE expires_at > datetime('now', 'subsec')"#
        ).fetch_one(&app_state.db_pool).await {
            Ok(result) => result.count as u64,
            Err(_) => 0,
        },
        timestamp: Utc::now(),
    };

    // Get security metrics (simplified - would need proper security monitor integration)
    let security_metrics = None; // TODO: Implement actual security metrics collection

    // Get system status
    let system_status = get_system_status(&app_state).await;

    let detailed_response = DetailedHealthResponse {
        basic_health: health_response,
        security_metrics,
        system_status,
    };

    Json(ApiResponse::success(detailed_response))
}

#[utoipa::path(
    get,
    path = "/api/health/security",
    responses(
        (status = 200, description = "Security-specific health metrics", body = ApiResponse<SecurityMetrics>),
        (status = 503, description = "Service unavailable", body = ApiResponse<String>)
    ),
    tag = "health",
    security(
        ("Bearer" = [])
    )
)]
pub async fn security_health_check(
    State(app_state): State<AppState>,
) -> Json<ApiResponse<SecurityMetrics>> {
    // Get security metrics from audit logger
    let now = Utc::now();
    let _one_hour_ago = now - chrono::Duration::hours(1);

    // Collect security metrics
    let active_sessions = match sqlx::query!(
        r#"SELECT COUNT(*) as "count!: i64" FROM user_sessions 
           WHERE expires_at > datetime('now', 'subsec')"#
    ).fetch_one(&app_state.db_pool).await {
        Ok(result) => result.count as u64,
        Err(_) => 0,
    };

    // For now, set security metrics to 0 since audit_log table may not exist
    // In production, these would be properly monitored via the audit system
    let failed_auth_attempts = 0u64;
    let security_events = 0u64;

    // Create simplified system health
    let database_healthy = sqlx::query!("SELECT 1 as status").fetch_one(&app_state.db_pool).await.is_ok();
    
    let system_health = crate::security::monitoring::SystemHealth {
        database_status: if database_healthy { HealthStatus::Healthy } else { HealthStatus::Critical },
        authentication_status: if database_healthy { HealthStatus::Healthy } else { HealthStatus::Critical },
        audit_system_status: if database_healthy { HealthStatus::Healthy } else { HealthStatus::Warning },
        overall_status: if database_healthy { HealthStatus::Healthy } else { HealthStatus::Critical },
    };

    let security_metrics = SecurityMetrics {
        timestamp: now,
        active_sessions,
        failed_auth_attempts_last_hour: failed_auth_attempts,
        security_events_last_hour: security_events,
        suspicious_activities: Vec::new(), // Simplified
        system_health,
    };

    Json(ApiResponse::success(security_metrics))
}

/// Get system resource usage information
async fn get_system_status(app_state: &AppState) -> SystemStatus {
    // Get database size
    let database_size_mb = match sqlx::query!(
        r#"SELECT page_count * page_size as "size!: i64" FROM pragma_page_count(), pragma_page_size()"#
    ).fetch_one(&app_state.db_pool).await {
        Ok(result) => Some((result.size / (1024 * 1024)) as u64),
        Err(_) => None,
    };

    // System metrics would typically come from system monitoring tools
    // For now, we'll use placeholder values
    SystemStatus {
        memory_usage_mb: None, // Would use procfs or similar
        cpu_usage_percent: None, // Would use system monitoring
        disk_usage_percent: None, // Would check filesystem usage
        database_size_mb,
    }
}
