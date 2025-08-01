use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    models::{
        user::User,
        user_session::{SessionType, UserSession},
    },
};
use super::app_config::AppConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,      // User ID
    pub session_id: String, // Session ID
    pub exp: i64,         // Expiration timestamp
    pub iat: i64,         // Issued at timestamp
    pub session_type: SessionType,
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub algorithm: Algorithm,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self::from_config()
    }
}

impl JwtConfig {
    /// Create JWT config from application configuration
    pub fn from_config() -> Self {
        let config = AppConfig::load().unwrap_or_else(|e| {
            tracing::error!("Failed to load configuration: {}", e);
            AppConfig::default()
        });
        
        Self {
            secret: config.jwt_secret,
            algorithm: Algorithm::HS256,
        }
    }

    pub fn encoding_key(&self) -> EncodingKey {
        EncodingKey::from_secret(self.secret.as_ref())
    }

    pub fn decoding_key(&self) -> DecodingKey {
        DecodingKey::from_secret(self.secret.as_ref())
    }

    pub fn validation(&self) -> Validation {
        let mut validation = Validation::new(self.algorithm);
        validation.validate_exp = true;
        validation
    }
}

/// Generate JWT token for a user session
pub fn generate_jwt_token(
    user_id: Uuid,
    session_id: Uuid,
    session_type: SessionType,
    jwt_config: &JwtConfig,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let expires_at = match session_type {
        SessionType::Web => now + Duration::hours(UserSession::WEB_SESSION_DURATION_HOURS),
        SessionType::Mcp => now + Duration::days(UserSession::MCP_SESSION_DURATION_DAYS),
    };

    let claims = Claims {
        sub: user_id.to_string(),
        session_id: session_id.to_string(),
        exp: expires_at.timestamp(),
        iat: now.timestamp(),
        session_type,
    };

    let header = Header::new(jwt_config.algorithm);
    encode(&header, &claims, &jwt_config.encoding_key())
}

/// Validate JWT token and return claims
pub fn validate_jwt_token(
    token: &str,
    jwt_config: &JwtConfig,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &jwt_config.decoding_key(), &jwt_config.validation())
        .map(|token_data| token_data.claims)
}

/// Hash token for storage (SHA256)
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Extract bearer token from Authorization header
pub fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    if auth_header.starts_with("Bearer ") {
        Some(&auth_header[7..])
    } else {
        None
    }
}

/// User context for authenticated requests
#[derive(Debug, Clone)]
pub struct UserContext {
    pub user: User,
    pub session: UserSession,
}

/// Authentication middleware
pub async fn auth_middleware(
    State(app_state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get JWT config from environment or default
    let jwt_config = JwtConfig::default();

    // Extract token from Authorization header
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|auth_header| extract_bearer_token(auth_header))
        .ok_or_else(|| {
            tracing::debug!("No valid Authorization header found");
            StatusCode::UNAUTHORIZED
        })?;

    // Validate JWT token
    let claims = validate_jwt_token(token, &jwt_config).map_err(|e| {
        tracing::debug!("JWT validation failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    // Parse user ID from claims
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let session_id = Uuid::parse_str(&claims.session_id).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Hash token for database lookup
    let token_hash = hash_token(token);

    // Verify session exists and is valid
    let session = UserSession::find_valid_by_token_hash(&app_state.db_pool, &token_hash)
        .await
        .map_err(|e| {
            tracing::error!("Database error during session lookup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Verify session belongs to the user in the JWT
    if session.user_id != user_id || session.id != session_id {
        tracing::warn!("Session mismatch: JWT claims don't match database session");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Get user from database
    let user = User::find_by_id(&app_state.db_pool, user_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error during user lookup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check if user is still whitelisted
    if !user.is_whitelisted {
        tracing::warn!("User {} is no longer whitelisted", user.username);
        return Err(StatusCode::FORBIDDEN);
    }

    // Update last login time
    if let Err(e) = User::update_last_login(&app_state.db_pool, user_id).await {
        tracing::warn!("Failed to update last login time: {}", e);
    }

    // Extend session expiration on activity
    if let Err(e) = UserSession::extend_session(&app_state.db_pool, session.id, session.session_type.clone()).await {
        tracing::warn!("Failed to extend session: {}", e);
    }

    // Create user context
    let user_context = UserContext { user, session };

    // Insert user context into request extensions
    req.extensions_mut().insert(user_context);

    Ok(next.run(req).await)
}

/// Optional authentication middleware (doesn't fail on missing auth)
#[allow(dead_code)] // Planned for future API endpoints that don't require auth
pub async fn optional_auth_middleware(
    State(app_state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    // Try to authenticate but don't fail if no auth provided
    if let Some(auth_header) = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
    {
        if let Some(token) = extract_bearer_token(auth_header) {
            let jwt_config = JwtConfig::default();
            
            if let Ok(claims) = validate_jwt_token(token, &jwt_config) {
                if let (Ok(user_id), Ok(session_id)) = (
                    Uuid::parse_str(&claims.sub),
                    Uuid::parse_str(&claims.session_id),
                ) {
                    let token_hash = hash_token(token);
                    
                    if let Ok(Some(session)) = UserSession::find_valid_by_token_hash(&app_state.db_pool, &token_hash).await {
                        if session.user_id == user_id && session.id == session_id {
                            if let Ok(Some(user)) = User::find_by_id(&app_state.db_pool, user_id).await {
                                if user.is_whitelisted {
                                    // Successfully authenticated - update last login
                                    if let Err(e) = User::update_last_login(&app_state.db_pool, user_id).await {
                                        tracing::warn!("Failed to update last login time: {}", e);
                                    }
                                    let user_context = UserContext { user, session };
                                    req.extensions_mut().insert(user_context);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    next.run(req).await
}

/// Extract user context from request (for use in handlers)
pub fn get_user_context(req: &Request) -> Option<&UserContext> {
    req.extensions().get::<UserContext>()
}

/// Extract user from request (convenience function)
pub fn get_current_user(req: &Request) -> Option<&User> {
    get_user_context(req).map(|ctx| &ctx.user)
}

/// Extract session from request (convenience function)
#[allow(dead_code)] // Utility function for handlers that need session info
pub fn get_current_session(req: &Request) -> Option<&UserSession> {
    get_user_context(req).map(|ctx| &ctx.session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_token() {
        let token = "test-token-123";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);
        
        // Same token should produce same hash
        assert_eq!(hash1, hash2);
        
        // Different tokens should produce different hashes
        let different_hash = hash_token("different-token");
        assert_ne!(hash1, different_hash);
        
        // Hash should be consistent length (SHA256 = 64 hex chars)
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_extract_bearer_token() {
        assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("Bearer "), Some(""));
        assert_eq!(extract_bearer_token("Basic abc123"), None);
        assert_eq!(extract_bearer_token("bearer abc123"), None); // Case sensitive
        assert_eq!(extract_bearer_token("abc123"), None);
    }

    #[test]
    fn test_jwt_token_generation_and_validation() {
        let jwt_config = JwtConfig::default();
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let session_type = SessionType::Web;
        
        // Generate token
        let token = generate_jwt_token(user_id, session_id, session_type.clone(), &jwt_config).unwrap();
        
        // Validate token
        let claims = validate_jwt_token(&token, &jwt_config).unwrap();
        
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.session_id, session_id.to_string());
        assert_eq!(claims.session_type, session_type);
        assert!(claims.exp > Utc::now().timestamp());
        assert!(claims.iat <= Utc::now().timestamp());
    }
}