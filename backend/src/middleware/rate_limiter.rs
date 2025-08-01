use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use tokio::sync::RwLock;
use uuid::Uuid;
use tracing::{debug, warn};

use crate::{
    app_state::AppState,
    auth::{get_current_user, get_user_context},
    models::user_session::SessionType,
};

/// Rate limit configuration for different endpoint types
#[derive(Debug, Clone)]
pub struct RateLimit {
    pub requests: u32,
    pub window: Duration,
    pub burst: u32,
}

impl RateLimit {
    /// Standard web API rate limit (60 requests per minute, burst of 10)
    pub fn web_api() -> Self {
        Self {
            requests: 60,
            window: Duration::from_secs(60),
            burst: 10,
        }
    }

    /// Authentication endpoints (more restrictive - 10 requests per minute)
    pub fn auth_endpoints() -> Self {
        Self {
            requests: 10,
            window: Duration::from_secs(60),
            burst: 3,
        }
    }

    /// MCP tool calls (higher limit - 120 requests per minute)
    pub fn mcp_tools() -> Self {
        Self {
            requests: 120,
            window: Duration::from_secs(60),
            burst: 20,
        }
    }

    /// Admin endpoints (very restrictive - 30 requests per hour)
    pub fn admin_endpoints() -> Self {
        Self {
            requests: 30,
            window: Duration::from_secs(3600),
            burst: 5,
        }
    }

    /// IP-based rate limiting for unauthenticated endpoints
    pub fn unauthenticated_ip() -> Self {
        Self {
            requests: 30,
            window: Duration::from_secs(60),
            burst: 5,
        }
    }
}

/// Rate limit entry tracking requests for a specific key
#[derive(Debug, Clone)]
struct RateLimitEntry {
    count: u32,
    window_start: Instant,
    burst_count: u32,
    burst_window_start: Instant,
}

impl RateLimitEntry {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            count: 0,
            window_start: now,
            burst_count: 0,
            burst_window_start: now,
        }
    }

    /// Check if request is allowed under rate limit
    fn is_allowed(&mut self, limit: &RateLimit) -> bool {
        let now = Instant::now();

        // Reset main window if expired
        if now.duration_since(self.window_start) >= limit.window {
            self.count = 0;
            self.window_start = now;
        }

        // Reset burst window every 10 seconds
        let burst_window = Duration::from_secs(10);
        if now.duration_since(self.burst_window_start) >= burst_window {
            self.burst_count = 0;
            self.burst_window_start = now;
        }

        // Check burst limit first
        if self.burst_count >= limit.burst {
            debug!("Rate limit exceeded: burst limit reached");
            return false;
        }

        // Check main rate limit
        if self.count >= limit.requests {
            debug!("Rate limit exceeded: request limit reached");
            return false;
        }

        // Allow request and increment counters
        self.count += 1;
        self.burst_count += 1;
        true
    }

    /// Get remaining requests in current window
    fn remaining(&self, limit: &RateLimit) -> u32 {
        let now = Instant::now();
        
        // If window has expired, return full limit
        if now.duration_since(self.window_start) >= limit.window {
            return limit.requests;
        }

        limit.requests.saturating_sub(self.count)
    }

    /// Get seconds until window reset
    fn reset_in_seconds(&self, limit: &RateLimit) -> u64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.window_start);
        
        if elapsed >= limit.window {
            0
        } else {
            (limit.window - elapsed).as_secs()
        }
    }
}

/// In-memory rate limiter (for production, consider Redis-based implementation)
#[derive(Debug)]
pub struct RateLimiter {
    entries: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        let rate_limiter = Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start cleanup task to remove expired entries
        let entries_clone = rate_limiter.entries.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Cleanup every 5 minutes
            loop {
                interval.tick().await;
                let mut entries = entries_clone.write().await;
                let now = Instant::now();
                
                // Remove entries older than 1 hour
                entries.retain(|_key, entry| {
                    now.duration_since(entry.window_start) < Duration::from_secs(3600)
                });
                
                debug!("Rate limiter cleanup: {} entries retained", entries.len());
            }
        });

        rate_limiter
    }

    /// Check if request is allowed and get rate limit info
    pub async fn check_rate_limit(&self, key: &str, limit: &RateLimit) -> (bool, RateLimitInfo) {
        let mut entries = self.entries.write().await;
        let entry = entries.entry(key.to_string()).or_insert_with(RateLimitEntry::new);
        
        let allowed = entry.is_allowed(limit);
        let info = RateLimitInfo {
            limit: limit.requests,
            remaining: entry.remaining(limit),
            reset_in_seconds: entry.reset_in_seconds(limit),
        };

        (allowed, info)
    }
}

/// Rate limit information to include in response headers
#[derive(Debug)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_in_seconds: u64,
}

/// Get rate limit key for a user
fn get_user_rate_limit_key(user_id: Uuid, endpoint_type: &str) -> String {
    format!("user:{}:{}", user_id, endpoint_type)
}

/// Get rate limit key for an IP address
fn get_ip_rate_limit_key(ip: &str, endpoint_type: &str) -> String {
    format!("ip:{}:{}", ip, endpoint_type)
}

/// Extract client IP address from request headers
fn get_client_ip(headers: &HeaderMap) -> String {
    // Try various headers in order of preference
    let ip_headers = [
        "x-forwarded-for",
        "x-real-ip",
        "cf-connecting-ip", // Cloudflare
        "x-client-ip",
    ];

    for header_name in &ip_headers {
        if let Some(header_value) = headers.get(header_name) {
            if let Ok(ip_str) = header_value.to_str() {
                // X-Forwarded-For can contain multiple IPs, take the first one
                let ip = ip_str.split(',').next().unwrap_or(ip_str).trim();
                if !ip.is_empty() && ip != "unknown" {
                    return ip.to_string();
                }
            }
        }
    }

    // Fallback to a default (this shouldn't happen in production with proper proxy setup)
    "unknown".to_string()
}

/// Add rate limit headers to response
fn add_rate_limit_headers(response: &mut Response, info: &RateLimitInfo) {
    if let Ok(mut response) = response.headers_mut() {
        response.insert("X-RateLimit-Limit", info.limit.to_string().parse().unwrap());
        response.insert("X-RateLimit-Remaining", info.remaining.to_string().parse().unwrap());
        response.insert("X-RateLimit-Reset", info.reset_in_seconds.to_string().parse().unwrap());
    }
}

/// Generic rate limiting middleware
pub async fn rate_limit_middleware(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let rate_limiter = &app_state.rate_limiter;
    let limit = RateLimit::web_api();
    let endpoint_type = "web_api";

    let (key, user_authenticated) = if let Some(user) = get_current_user(&req) {
        // User-based rate limiting
        (get_user_rate_limit_key(user.id, endpoint_type), true)
    } else {
        // IP-based rate limiting
        let ip = get_client_ip(&headers);
        (get_ip_rate_limit_key(&ip, endpoint_type), false)
    };

    let (allowed, info) = rate_limiter.check_rate_limit(&key, &limit).await;

    if !allowed {
        warn!(
            "Rate limit exceeded for {}: {} (limit: {}, remaining: {})",
            if user_authenticated { "user" } else { "IP" },
            key,
            info.limit,
            info.remaining
        );

        let mut response = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Content-Type", "application/json")
            .body(format!(
                r#"{{"error":"rate_limit_exceeded","message":"Rate limit exceeded. Try again in {} seconds.","retry_after":{}}}"#,
                info.reset_in_seconds, info.reset_in_seconds
            ).into())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        add_rate_limit_headers(&mut response, &info);
        return Ok(response);
    }

    // Continue to next middleware/handler
    let mut response = next.run(req).await;
    add_rate_limit_headers(&mut response, &info);
    Ok(response)
}

/// Authentication endpoints rate limiting middleware
pub async fn auth_rate_limit_middleware(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let rate_limiter = &app_state.rate_limiter;
    let limit = RateLimit::auth_endpoints();
    let endpoint_type = "auth";

    // For auth endpoints, always use IP-based limiting since user may not be authenticated yet
    let ip = get_client_ip(&headers);
    let key = get_ip_rate_limit_key(&ip, endpoint_type);

    let (allowed, info) = rate_limiter.check_rate_limit(&key, &limit).await;

    if !allowed {
        warn!("Auth rate limit exceeded for IP: {} (limit: {}, remaining: {})", ip, info.limit, info.remaining);

        let mut response = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Content-Type", "application/json")
            .body(format!(
                r#"{{"error":"auth_rate_limit_exceeded","message":"Too many authentication requests. Try again in {} seconds.","retry_after":{}}}"#,
                info.reset_in_seconds, info.reset_in_seconds
            ).into())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        add_rate_limit_headers(&mut response, &info);
        return Ok(response);
    }

    let mut response = next.run(req).await;
    add_rate_limit_headers(&mut response, &info);
    Ok(response)
}

/// MCP-specific rate limiting middleware
pub async fn mcp_rate_limit_middleware(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let rate_limiter = &app_state.rate_limiter;
    let limit = RateLimit::mcp_tools();
    let endpoint_type = "mcp";

    let (key, user_authenticated) = if let Some(user_context) = get_user_context(&req) {
        // For MCP, only allow rate limiting for MCP sessions
        if user_context.session.session_type == SessionType::Mcp {
            (get_user_rate_limit_key(user_context.user.id, endpoint_type), true)
        } else {
            // Fallback to IP limiting for non-MCP sessions
            let ip = get_client_ip(&headers);
            (get_ip_rate_limit_key(&ip, endpoint_type), false)
        }
    } else {
        // IP-based rate limiting for unauthenticated requests
        let ip = get_client_ip(&headers);
        (get_ip_rate_limit_key(&ip, endpoint_type), false)
    };

    let (allowed, info) = rate_limiter.check_rate_limit(&key, &limit).await;

    if !allowed {
        warn!(
            "MCP rate limit exceeded for {}: {} (limit: {}, remaining: {})",
            if user_authenticated { "user" } else { "IP" },
            key,
            info.limit,
            info.remaining
        );

        let mut response = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Content-Type", "application/json")
            .body(format!(
                r#"{{"error":"mcp_rate_limit_exceeded","message":"MCP rate limit exceeded. Try again in {} seconds.","retry_after":{}}}"#,
                info.reset_in_seconds, info.reset_in_seconds
            ).into())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        add_rate_limit_headers(&mut response, &info);
        return Ok(response);
    }

    let mut response = next.run(req).await;
    add_rate_limit_headers(&mut response, &info);
    Ok(response)
}

/// Admin endpoints rate limiting middleware
pub async fn admin_rate_limit_middleware(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let rate_limiter = &app_state.rate_limiter;
    let limit = RateLimit::admin_endpoints();
    let endpoint_type = "admin";

    let key = if let Some(user) = get_current_user(&req) {
        if !user.is_admin {
            // Non-admin users shouldn't reach admin endpoints
            return Err(StatusCode::FORBIDDEN);
        }
        get_user_rate_limit_key(user.id, endpoint_type)
    } else {
        // Should not happen due to auth middleware, but fallback to IP limiting
        let ip = get_client_ip(&headers);
        get_ip_rate_limit_key(&ip, endpoint_type)
    };

    let (allowed, info) = rate_limiter.check_rate_limit(&key, &limit).await;

    if !allowed {
        warn!("Admin rate limit exceeded for key: {} (limit: {}, remaining: {})", key, info.limit, info.remaining);

        let mut response = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Content-Type", "application/json")
            .body(format!(
                r#"{{"error":"admin_rate_limit_exceeded","message":"Admin rate limit exceeded. Try again in {} seconds.","retry_after":{}}}"#,
                info.reset_in_seconds, info.reset_in_seconds
            ).into())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        add_rate_limit_headers(&mut response, &info);
        return Ok(response);
    }

    let mut response = next.run(req).await;
    add_rate_limit_headers(&mut response, &info);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_rate_limiter_basic_functionality() {
        let rate_limiter = RateLimiter::new();
        let limit = RateLimit {
            requests: 2,
            window: Duration::from_secs(10),
            burst: 1,
        };

        let key = "test_key";

        // First request should be allowed
        let (allowed, info) = rate_limiter.check_rate_limit(key, &limit).await;
        assert!(allowed);
        assert_eq!(info.remaining, 1);

        // Second request should be allowed but hit burst limit
        let (allowed, info) = rate_limiter.check_rate_limit(key, &limit).await;
        assert!(allowed);
        assert_eq!(info.remaining, 0);

        // Third request should be blocked due to burst limit
        let (allowed, _info) = rate_limiter.check_rate_limit(key, &limit).await;
        assert!(!allowed);
    }

    #[tokio::test]
    async fn test_rate_limiter_window_reset() {
        let rate_limiter = RateLimiter::new();
        let limit = RateLimit {
            requests: 1,
            window: Duration::from_millis(100), // Very short window for testing
            burst: 5, // High burst to avoid burst limiting
        };

        let key = "test_key_reset";

        // First request should be allowed
        let (allowed, _info) = rate_limiter.check_rate_limit(key, &limit).await;
        assert!(allowed);

        // Second request should be blocked
        let (allowed, _info) = rate_limiter.check_rate_limit(key, &limit).await;
        assert!(!allowed);

        // Wait for window to reset
        sleep(Duration::from_millis(150)).await;

        // Third request should be allowed after window reset
        let (allowed, info) = rate_limiter.check_rate_limit(key, &limit).await;
        assert!(allowed);
        assert_eq!(info.remaining, 0);
    }

    #[test]
    fn test_rate_limit_configurations() {
        let web_limit = RateLimit::web_api();
        assert_eq!(web_limit.requests, 60);
        assert_eq!(web_limit.window, Duration::from_secs(60));

        let auth_limit = RateLimit::auth_endpoints();
        assert_eq!(auth_limit.requests, 10);
        assert_eq!(auth_limit.window, Duration::from_secs(60));

        let mcp_limit = RateLimit::mcp_tools();
        assert_eq!(mcp_limit.requests, 120);
        assert_eq!(mcp_limit.window, Duration::from_secs(60));

        let admin_limit = RateLimit::admin_endpoints();
        assert_eq!(admin_limit.requests, 30);
        assert_eq!(admin_limit.window, Duration::from_secs(3600));
    }

    #[test]
    fn test_rate_limit_key_generation() {
        let user_id = Uuid::new_v4();
        let user_key = get_user_rate_limit_key(user_id, "web_api");
        assert_eq!(user_key, format!("user:{}:web_api", user_id));

        let ip_key = get_ip_rate_limit_key("192.168.1.1", "auth");
        assert_eq!(ip_key, "ip:192.168.1.1:auth");
    }
}