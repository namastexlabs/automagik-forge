use axum::{
    extract::Request,
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
};
use std::time::Duration;

/// Security headers middleware that adds various security-related HTTP headers
pub async fn security_headers_middleware(
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    // Strict-Transport-Security (HSTS) - Force HTTPS
    // Only add in production or when HTTPS is detected
    let is_https = std::env::var("HTTPS").unwrap_or_default() == "true" ||
                   std::env::var("NODE_ENV").unwrap_or_default() == "production";
    if is_https {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
        );
    }

    // X-Frame-Options - Prevent clickjacking
    headers.insert(
        "x-frame-options",
        HeaderValue::from_static("DENY"),
    );

    // X-Content-Type-Options - Prevent MIME type sniffing
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );

    // Referrer-Policy - Control referrer information
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // X-XSS-Protection - Enable XSS filtering (legacy, but still good to have)
    headers.insert(
        "x-xss-protection",
        HeaderValue::from_static("1; mode=block"),
    );

    // Content-Security-Policy - Comprehensive CSP for security
    let csp = create_content_security_policy();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_str(&csp).unwrap_or_else(|_| HeaderValue::from_static("default-src 'self'")),
    );

    // Permissions-Policy - Control browser features
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), interest-cohort=()"),
    );

    // Cross-Origin-Embedder-Policy - Prevent loading cross-origin resources
    headers.insert(
        "cross-origin-embedder-policy",
        HeaderValue::from_static("require-corp"),
    );

    // Cross-Origin-Opener-Policy - Isolate browsing context
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );

    // Cross-Origin-Resource-Policy - Control cross-origin resource sharing
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );

    // Server header - Remove or obfuscate server information
    headers.insert(
        header::SERVER,
        HeaderValue::from_static("automagik-forge"),
    );

    response
}

/// Create Content Security Policy based on environment and configuration
fn create_content_security_policy() -> String {
    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());
    let is_development = cfg!(debug_assertions) || 
                        std::env::var("NODE_ENV").unwrap_or_default() == "development";

    let mut csp_directives = vec![
        "default-src 'self'".to_string(),
        format!("connect-src 'self' {} https://api.github.com https://github.com wss: ws:", base_url),
        "font-src 'self' data: https://fonts.gstatic.com".to_string(),
        "img-src 'self' data: https: blob:".to_string(),
        "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com".to_string(),
        "frame-ancestors 'none'".to_string(),
        "base-uri 'self'".to_string(),
        "form-action 'self' https://github.com".to_string(),
        "manifest-src 'self'".to_string(),
        "media-src 'self' blob:".to_string(),
        "object-src 'none'".to_string(),
        "worker-src 'self' blob:".to_string(),
    ];

    // In development, allow 'unsafe-eval' for better DX with hot reloading
    if is_development {
        csp_directives.push("script-src 'self' 'unsafe-eval'".to_string());
    } else {
        csp_directives.push("script-src 'self'".to_string());
        // Add upgrade-insecure-requests in production
        csp_directives.push("upgrade-insecure-requests".to_string());
    }

    csp_directives.join("; ")
}

/// CORS configuration for production security
pub fn create_secure_cors_layer() -> tower_http::cors::CorsLayer {
    use tower_http::cors::{CorsLayer, Any};
    use axum::http::{Method, HeaderValue};

    let allowed_origins = get_allowed_origins();
    let is_development = cfg!(debug_assertions) || 
                        std::env::var("NODE_ENV").unwrap_or_default() == "development";

    let mut cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            "x-requested-with".parse::<header::HeaderName>().unwrap(),
        ])
        .allow_credentials(true)
        .max_age(Duration::from_secs(86400)); // 24 hours

    // Configure origins based on environment
    if is_development {
        // In development, be more permissive but still secure
        cors = cors.allow_origin(Any);
        tracing::warn!("CORS configured for development - allowing all origins");
    } else {
        // In production, only allow specific origins
        for origin in allowed_origins {
            if let Ok(header_value) = HeaderValue::from_str(&origin) {
                cors = cors.allow_origin(header_value);
            }
        }
    }

    cors
}

/// Get allowed CORS origins from environment configuration
fn get_allowed_origins() -> Vec<String> {
    let mut origins = Vec::new();

    // Add base URL if configured
    if let Ok(base_url) = std::env::var("BASE_URL") {
        origins.push(base_url);
    }

    // Add additional allowed origins from environment
    if let Ok(cors_origins) = std::env::var("CORS_ORIGINS") {
        for origin in cors_origins.split(',') {
            let trimmed = origin.trim();
            if !trimmed.is_empty() {
                origins.push(trimmed.to_string());
            }
        }
    }

    // Default development origins if none configured
    if origins.is_empty() {
        origins.extend_from_slice(&[
            "http://localhost:3000".to_string(),
            "http://127.0.0.1:3000".to_string(),
            "http://localhost:3001".to_string(),
            "http://127.0.0.1:3001".to_string(),
        ]);
    }

    origins
}

/// Middleware to add security context to response headers for debugging
pub async fn security_context_middleware(
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    
    // Add security context headers (only in development)
    if cfg!(debug_assertions) {
        let headers = response.headers_mut();
        headers.insert(
            "x-security-context",
            HeaderValue::from_static("automagik-forge-security-enabled"),
        );
    }

    response
}


/// Security monitoring middleware that logs suspicious requests
pub async fn security_monitoring_middleware(
    req: Request,
    next: Next,
) -> Response {
    let uri = req.uri().clone();
    let method = req.method().clone();
    let headers = req.headers().clone();
    
    // Check for suspicious patterns
    let path = uri.path();
    let is_suspicious = is_suspicious_request(path, &headers);
    
    if is_suspicious {
        tracing::warn!(
            method = %method,
            path = %path,
            user_agent = ?headers.get("user-agent"),
            x_forwarded_for = ?headers.get("x-forwarded-for"),
            "Suspicious request detected"
        );
    }
    
    let response = next.run(req).await;
    
    // Log failed requests that might indicate attacks
    if response.status().is_client_error() || response.status().is_server_error() {
        tracing::info!(
            status = %response.status(),
            method = %method,
            path = %path,
            "Request completed with error status"
        );
    }
    
    response
}

/// Check if a request exhibits suspicious patterns
fn is_suspicious_request(path: &str, headers: &axum::http::HeaderMap) -> bool {
    // Check for common attack patterns in path
    let suspicious_paths = [
        "/.env",
        "/admin",
        "/wp-admin",
        "/phpMyAdmin",
        "/config",
        "/backup",
        "/.git",
        "/vendor",
        "/.well-known/security.txt",
        "/robots.txt",
        "/sitemap.xml",
    ];
    
    let path_lower = path.to_lowercase();
    
    // Check for directory traversal attempts
    if path_lower.contains("../") || path_lower.contains("..\\") {
        return true;
    }
    
    // Check for SQL injection patterns
    if path_lower.contains("union") || path_lower.contains("select") || 
       path_lower.contains("drop") || path_lower.contains("insert") {
        return true;
    }
    
    // Check for XSS patterns
    if path_lower.contains("<script") || path_lower.contains("javascript:") {
        return true;
    }
    
    // Check for suspicious file extensions
    let suspicious_extensions = [".php", ".asp", ".jsp", ".cgi"];
    for ext in &suspicious_extensions {
        if path_lower.ends_with(ext) {
            return true;
        }
    }
    
    // Check for suspicious paths
    for suspicious_path in &suspicious_paths {
        if path_lower.starts_with(suspicious_path) {
            return true;
        }
    }
    
    // Check for suspicious user agents
    if let Some(user_agent) = headers.get("user-agent") {
        if let Ok(ua_str) = user_agent.to_str() {
            let ua_lower = ua_str.to_lowercase();
            let suspicious_uas = ["sqlmap", "nikto", "dirb", "gobuster", "masscan", "nmap"];
            for sus_ua in &suspicious_uas {
                if ua_lower.contains(sus_ua) {
                    return true;
                }
            }
        }
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_create_content_security_policy() {
        let csp = create_content_security_policy();
        
        // Check that basic directives are present
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("object-src 'none'"));
        
        // In tests, it should include script-src with 'unsafe-eval' for development
        assert!(csp.contains("script-src"));
    }

    #[test]
    fn test_get_allowed_origins() {
        let origins = get_allowed_origins();
        
        // Should have at least default localhost origins
        assert!(!origins.is_empty());
        assert!(origins.iter().any(|o| o.contains("localhost")));
    }

    #[test]
    fn test_is_suspicious_request() {
        let headers = HeaderMap::new();
        
        // Test suspicious paths
        assert!(is_suspicious_request("/.env", &headers));
        assert!(is_suspicious_request("/admin", &headers));
        assert!(is_suspicious_request("/test/../etc/passwd", &headers));
        assert!(is_suspicious_request("/index.php", &headers));
        
        // Test normal paths
        assert!(!is_suspicious_request("/", &headers));
        assert!(!is_suspicious_request("/api/health", &headers));
        assert!(!is_suspicious_request("/static/js/app.js", &headers));
    }

    #[test]
    fn test_suspicious_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_static("sqlmap/1.0"));
        
        assert!(is_suspicious_request("/", &headers));
        
        headers.clear();
        headers.insert("user-agent", HeaderValue::from_static("Mozilla/5.0 (Normal Browser)"));
        assert!(!is_suspicious_request("/", &headers));
    }

    #[test]
    fn test_sql_injection_detection() {
        let headers = HeaderMap::new();
        
        assert!(is_suspicious_request("/api/users?id=1 UNION SELECT * FROM users", &headers));
        assert!(is_suspicious_request("/search?q=test' OR 1=1--", &headers));
        assert!(!is_suspicious_request("/api/users?name=john", &headers));
    }
}