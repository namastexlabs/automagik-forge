use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use axum::{
    extract::State,
    http::{HeaderMap, Request, Response, StatusCode},
    middleware::Next,
    body::Body,
};
use sqlx::SqlitePool;
use chrono;

use crate::{
    auth::{validate_jwt_token, hash_token, JwtConfig},
    models::{
        user::User,
        user_session::{SessionType, UserSession},
    },
    mcp::task_server::McpToken,
};

/// OAuth validation middleware for MCP SSE connections
/// This middleware validates Bearer tokens and injects user context into requests
#[allow(dead_code)] // OAuth middleware for future MCP SSE authentication
pub async fn validate_oauth_token_middleware(
    State(token_store): State<Arc<RwLock<HashMap<String, McpToken>>>>,
    State(db_pool): State<SqlitePool>,
    headers: HeaderMap,
    mut req: Request<Body>,
    next: Next,
) -> Response<Body> {
    // Extract Authorization header
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                // First try MCP token store
                {
                    let store = token_store.read().await;
                    if let Some(mcp_token) = store.get(token) {
                        if mcp_token.expires_at > chrono::Utc::now() {
                            if let Ok(Some(user)) = User::find_by_id(&db_pool, mcp_token.user_id).await {
                                if let Ok(Some(session)) = UserSession::find_by_token_hash(&db_pool, &hash_token(token)).await {
                                    if user.is_whitelisted && session.session_type == SessionType::Mcp {
                                        // Inject user context into request
                                        req.extensions_mut().insert(user);
                                        req.extensions_mut().insert(session);
                                        return next.run(req).await;
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Fallback to JWT validation for existing OAuth endpoints compatibility
                let jwt_config = JwtConfig::default();
                if let Ok(claims) = validate_jwt_token(token, &jwt_config) {
                    if let Ok(Some(session)) = UserSession::find_by_token_hash(&db_pool, &hash_token(token)).await {
                        if let Ok(claims_uuid) = claims.sub.parse::<uuid::Uuid>() {
                            if let Ok(Some(user)) = User::find_by_id(&db_pool, claims_uuid).await {
                                if user.is_whitelisted && session.session_type == SessionType::Mcp {
                                    // Inject user context into request
                                    req.extensions_mut().insert(user);
                                    req.extensions_mut().insert(session);
                                    return next.run(req).await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Return OAuth challenge response for unauthenticated requests
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("WWW-Authenticate", "Bearer realm=\"MCP\", error=\"invalid_token\"")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"error":"invalid_token","error_description":"Valid OAuth Bearer token required for MCP access"}"#))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Server Error"))
                .unwrap()
        })
}

/// Enhanced OAuth middleware that also handles automatic browser opening workflow
/// This is used by the SSE server to provide OAuth 2.1 authentication for external clients
#[allow(dead_code)] // OAuth SSE middleware for future MCP authentication
pub async fn oauth_sse_authentication_middleware(
    token_store: Arc<RwLock<HashMap<String, McpToken>>>,
    db_pool: SqlitePool,
    headers: HeaderMap,
    mut req: Request<Body>,
    next: Next,
) -> Response<Body> {
    // Check for existing valid authentication first
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                // Validate token using the same logic as validate_oauth_token_middleware
                {
                    let store = token_store.read().await;
                    if let Some(mcp_token) = store.get(token) {
                        if mcp_token.expires_at > chrono::Utc::now() {
                            if let Ok(Some(user)) = User::find_by_id(&db_pool, mcp_token.user_id).await {
                                if let Ok(Some(session)) = UserSession::find_by_token_hash(&db_pool, &hash_token(token)).await {
                                    if user.is_whitelisted && session.session_type == SessionType::Mcp {
                                        req.extensions_mut().insert(user);
                                        req.extensions_mut().insert(session);
                                        return next.run(req).await;
                                    }
                                }
                            }
                        }
                    }
                }
                
                // JWT fallback
                let jwt_config = JwtConfig::default();
                if let Ok(claims) = validate_jwt_token(token, &jwt_config) {
                    if let Ok(Some(session)) = UserSession::find_by_token_hash(&db_pool, &hash_token(token)).await {
                        if let Ok(claims_uuid) = claims.sub.parse::<uuid::Uuid>() {
                            if let Ok(Some(user)) = User::find_by_id(&db_pool, claims_uuid).await {
                                if user.is_whitelisted && session.session_type == SessionType::Mcp {
                                    req.extensions_mut().insert(user);
                                    req.extensions_mut().insert(session);
                                    return next.run(req).await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // For unauthenticated requests, return OAuth 2.1 challenge with discovery information
    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());
    let oauth_challenge = format!(
        r#"Bearer realm="MCP", authorization_uri="{}/oauth/authorize", token_uri="{}/oauth/token", error="insufficient_scope""#,
        base_url, base_url
    );
    
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("WWW-Authenticate", &oauth_challenge)
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"error":"insufficient_scope","error_description":"OAuth 2.1 authentication required","authorization_endpoint":"{}/.well-known/oauth-authorization-server"}}"#,
            base_url
        )))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Server Error"))
                .unwrap()
        })
}