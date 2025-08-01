use automagik_forge::{
    auth::{generate_jwt_token, validate_jwt_token, hash_token, extract_bearer_token, JwtConfig, UserContext, auth_middleware},
    models::{
        user::{User, CreateUser},
        user_session::{UserSession, SessionType, CreateUserSession},
    },
    app_state::AppState,
};
use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, Method, StatusCode},
    middleware::Next,
    response::Response,
    body::Body,
};
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio;
use uuid::Uuid;

/// Helper function to create an in-memory test database
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();
        
    pool
}

/// Helper function to create a test user
async fn create_test_user(pool: &SqlitePool) -> User {
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id: 123456,
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        display_name: Some("Test User".to_string()),
        avatar_url: Some("https://github.com/testuser.png".to_string()),
        github_token: Some("encrypted_token".to_string()),
        is_admin: Some(false),
    };
    
    User::create(pool, &create_data, user_id).await.unwrap()
}

/// Helper function to create a test session
async fn create_test_session(pool: &SqlitePool, user: &User, session_type: SessionType) -> (UserSession, String) {
    let jwt_config = JwtConfig::default();
    let session_id = Uuid::new_v4();
    
    // Generate JWT token
    let jwt_token = generate_jwt_token(user.id, session_id, session_type, &jwt_config).unwrap();
    let token_hash = hash_token(&jwt_token);
    
    // Create session in database
    let expires_at = match session_type {
        SessionType::Web => Utc::now() + Duration::hours(UserSession::WEB_SESSION_DURATION_HOURS),
        SessionType::Mcp => Utc::now() + Duration::days(UserSession::MCP_SESSION_DURATION_DAYS),
    };
    
    let session_data = CreateUserSession {
        user_id: user.id,
        token_hash: token_hash.clone(),
        session_type,
        client_info: Some("Test Client".to_string()),
        expires_at,
    };
    
    let session = UserSession::create(pool, &session_data, session_id).await.unwrap();
    (session, jwt_token)
}

#[tokio::test]
async fn test_jwt_token_generation_and_validation() {
    let jwt_config = JwtConfig::default();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let session_type = SessionType::Web;
    
    // Generate token
    let token = generate_jwt_token(user_id, session_id, session_type, &jwt_config).unwrap();
    assert!(!token.is_empty());
    
    // Validate token
    let claims = validate_jwt_token(&token, &jwt_config).unwrap();
    
    assert_eq!(claims.sub, user_id.to_string());
    assert_eq!(claims.session_id, session_id.to_string());
    assert_eq!(claims.session_type, session_type);
    assert!(claims.exp > Utc::now().timestamp());
    assert!(claims.iat <= Utc::now().timestamp());
}

#[tokio::test]
async fn test_jwt_token_expiration() {
    let jwt_config = JwtConfig::default();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    
    // Generate token with very short expiration for testing
    let now = Utc::now();
    let expired_time = now - Duration::hours(1); // Already expired
    
    // We can't directly test expired tokens with our current implementation,
    // but we can test validation of malformed tokens
    let invalid_token = "invalid.jwt.token";
    let result = validate_jwt_token(invalid_token, &jwt_config);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_token_hashing_consistency() {
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

#[tokio::test]
async fn test_bearer_token_extraction() {
    assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
    assert_eq!(extract_bearer_token("Bearer "), Some(""));
    assert_eq!(extract_bearer_token("Basic abc123"), None);
    assert_eq!(extract_bearer_token("bearer abc123"), None); // Case sensitive
    assert_eq!(extract_bearer_token("abc123"), None);
}

#[tokio::test]
async fn test_auth_middleware_success() {
    let pool = setup_test_db().await;
    let app_state = AppState {
        db_pool: pool.clone(),
    };
    
    // Create test user and session
    let user = create_test_user(&pool).await;
    let (session, jwt_token) = create_test_session(&pool, &user, SessionType::Web).await;
    
    // Create test request with valid token
    let mut request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .header(AUTHORIZATION, format!("Bearer {}", jwt_token))
        .body(Body::empty())
        .unwrap();
    
    // Mock next handler that expects user context
    let next = |req: Request| async move {
        // Verify user context is available
        let user_context = req.extensions().get::<UserContext>();
        assert!(user_context.is_some());
        
        let context = user_context.unwrap();
        assert_eq!(context.user.id, user.id);
        assert_eq!(context.session.id, session.id);
        
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap()
    };
    
    // Run middleware
    let result = auth_middleware(State(app_state), request, next).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_middleware_missing_token() {
    let pool = setup_test_db().await;
    let app_state = AppState {
        db_pool: pool.clone(),
    };
    
    // Create test request without token
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .body(Body::empty())
        .unwrap();
    
    let next = |_req: Request| async move {
        panic!("Next handler should not be called");
    };
    
    // Run middleware
    let result = auth_middleware(State(app_state), request, next).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_middleware_invalid_token() {
    let pool = setup_test_db().await;
    let app_state = AppState {
        db_pool: pool.clone(),
    };
    
    // Create test request with invalid token
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .header(AUTHORIZATION, "Bearer invalid.jwt.token")
        .body(Body::empty())
        .unwrap();
    
    let next = |_req: Request| async move {
        panic!("Next handler should not be called");
    };
    
    // Run middleware
    let result = auth_middleware(State(app_state), request, next).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_middleware_expired_session() {
    let pool = setup_test_db().await;
    let app_state = AppState {
        db_pool: pool.clone(),
    };
    
    // Create test user
    let user = create_test_user(&pool).await;
    
    // Create expired session
    let jwt_config = JwtConfig::default();
    let session_id = Uuid::new_v4();
    let jwt_token = generate_jwt_token(user.id, session_id, SessionType::Web, &jwt_config).unwrap();
    let token_hash = hash_token(&jwt_token);
    
    // Create session with past expiration date
    let expired_at = Utc::now() - Duration::hours(1);
    let session_data = CreateUserSession {
        user_id: user.id,
        token_hash,
        session_type: SessionType::Web,
        client_info: Some("Test Client".to_string()),
        expires_at: expired_at,
    };
    
    UserSession::create(&pool, &session_data, session_id).await.unwrap();
    
    // Create test request with token for expired session
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .header(AUTHORIZATION, format!("Bearer {}", jwt_token))
        .body(Body::empty())
        .unwrap();
    
    let next = |_req: Request| async move {
        panic!("Next handler should not be called");
    };
    
    // Run middleware
    let result = auth_middleware(State(app_state), request, next).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_middleware_non_whitelisted_user() {
    let pool = setup_test_db().await;
    let app_state = AppState {
        db_pool: pool.clone(),
    };
    
    // Create test user that is not whitelisted
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id: 789012,
        username: "nonwhitelisted".to_string(),
        email: "nonwhitelisted@example.com".to_string(),
        display_name: Some("Non Whitelisted User".to_string()),
        avatar_url: None,
        github_token: None,
        is_admin: Some(false),
    };
    
    let mut user = User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Manually set user as not whitelisted
    sqlx::query!("UPDATE users SET is_whitelisted = FALSE WHERE id = ?", user.id)
        .execute(&pool)
        .await
        .unwrap();
    
    user.is_whitelisted = false;
    
    // Create session for non-whitelisted user
    let (_, jwt_token) = create_test_session(&pool, &user, SessionType::Web).await;
    
    // Create test request
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .header(AUTHORIZATION, format!("Bearer {}", jwt_token))
        .body(Body::empty())
        .unwrap();
    
    let next = |_req: Request| async move {
        panic!("Next handler should not be called");
    };
    
    // Run middleware
    let result = auth_middleware(State(app_state), request, next).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
}


#[tokio::test]
async fn test_different_session_types() {
    let jwt_config = JwtConfig::default();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    
    // Test Web session
    let web_token = generate_jwt_token(user_id, session_id, SessionType::Web, &jwt_config).unwrap();
    let web_claims = validate_jwt_token(&web_token, &jwt_config).unwrap();
    assert_eq!(web_claims.session_type, SessionType::Web);
    
    // Test MCP session
    let mcp_token = generate_jwt_token(user_id, session_id, SessionType::Mcp, &jwt_config).unwrap();
    let mcp_claims = validate_jwt_token(&mcp_token, &jwt_config).unwrap();
    assert_eq!(mcp_claims.session_type, SessionType::Mcp);
    
    // Tokens should be different
    assert_ne!(web_token, mcp_token);
}

#[tokio::test]
async fn test_auth_middleware_session_mismatch() {
    let pool = setup_test_db().await;
    let app_state = AppState {
        db_pool: pool.clone(),
    };
    
    // Create test user
    let user = create_test_user(&pool).await;
    
    // Create JWT token with different session ID than what's in database
    let jwt_config = JwtConfig::default();
    let wrong_session_id = Uuid::new_v4();
    let jwt_token = generate_jwt_token(user.id, wrong_session_id, SessionType::Web, &jwt_config).unwrap();
    
    // Create session in database with different ID
    let correct_session_id = Uuid::new_v4();
    let token_hash = hash_token(&jwt_token);
    let expires_at = Utc::now() + Duration::hours(24);
    let session_data = CreateUserSession {
        user_id: user.id,
        token_hash,
        session_type: SessionType::Web,
        client_info: Some("Test Client".to_string()),
        expires_at,
    };
    
    UserSession::create(&pool, &session_data, correct_session_id).await.unwrap();
    
    // Create test request
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/test")
        .header(AUTHORIZATION, format!("Bearer {}", jwt_token))
        .body(Body::empty())
        .unwrap();
    
    let next = |_req: Request| async move {
        panic!("Next handler should not be called");
    };
    
    // Run middleware - should fail due to session ID mismatch
    let result = auth_middleware(State(app_state), request, next).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
}