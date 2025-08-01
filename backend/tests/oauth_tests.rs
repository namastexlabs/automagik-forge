use automagik_forge::{
    auth::{generate_jwt_token, hash_token, JwtConfig},
    models::{
        user::{User, CreateUser},
        user_session::{UserSession, SessionType, CreateUserSession},
    },
    routes::oauth::{
        oauth_discovery, oauth_authorize, oauth_callback, oauth_token,
        AuthorizeQuery, CallbackQuery, TokenRequest, TokenResponse, OAuthErrorResponse,
    },
    app_state::AppState,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;
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
async fn create_test_user(pool: &SqlitePool, github_id: i64) -> User {
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id,
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        display_name: Some("Test User".to_string()),
        avatar_url: Some("https://github.com/testuser.png".to_string()),
        github_token: Some("encrypted_token".to_string()),
        is_admin: Some(false),
    };
    
    User::create(pool, &create_data, user_id).await.unwrap()
}

/// Helper function to add user to whitelist
async fn add_to_whitelist(pool: &SqlitePool, github_id: i64, username: &str) {
    sqlx::query!(
        "INSERT INTO github_whitelist (id, github_username, github_id, is_active) VALUES (?, ?, ?, ?)",
        Uuid::new_v4().to_string(),
        username,
        github_id,
        true
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_oauth_discovery_endpoint() {
    let response = oauth_discovery().await;
    let discovery = response.0;
    
    // Should contain required OAuth 2.1 discovery fields
    assert!(!discovery.issuer.is_empty());
    assert!(discovery.authorization_endpoint.contains("/oauth/authorize"));
    assert!(discovery.token_endpoint.contains("/oauth/token"));
    assert_eq!(discovery.response_types_supported, vec!["code"]);
    assert_eq!(discovery.grant_types_supported, vec!["authorization_code"]);
    assert_eq!(discovery.code_challenge_methods_supported, vec!["S256"]);
}

#[tokio::test]
async fn test_oauth_authorize_valid_request() {
    let params = AuthorizeQuery {
        client_id: Some("test-client".to_string()),
        redirect_uri: Some("http://localhost:8080/callback".to_string()),
        response_type: Some("code".to_string()),
        scope: Some("user:email,repo".to_string()),
        state: Some("test-state".to_string()),
        code_challenge: Some("test-challenge".to_string()),
        code_challenge_method: Some("S256".to_string()),
    };
    
    let result = oauth_authorize(Query(params)).await;
    assert!(result.is_ok());
    
    let redirect = result.unwrap();
    let location = redirect.location().unwrap();
    
    // Should redirect to GitHub OAuth
    assert!(location.starts_with("https://github.com/login/oauth/authorize"));
    assert!(location.contains("client_id="));
    assert!(location.contains("redirect_uri="));
    assert!(location.contains("scope=user%3Aemail%2Crepo"));
    assert!(location.contains("state="));
}

#[tokio::test]
async fn test_oauth_authorize_missing_redirect_uri() {
    let params = AuthorizeQuery {
        client_id: Some("test-client".to_string()),
        redirect_uri: None, // Missing required parameter
        response_type: Some("code".to_string()),
        scope: Some("user:email,repo".to_string()),
        state: Some("test-state".to_string()),
        code_challenge: Some("test-challenge".to_string()),
        code_challenge_method: Some("S256".to_string()),
    };
    
    let result = oauth_authorize(Query(params)).await;
    assert!(result.is_err());
    
    // Should return error response
    let error_response = result.unwrap_err();
    assert_eq!(error_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_oauth_authorize_invalid_response_type() {
    let params = AuthorizeQuery {
        client_id: Some("test-client".to_string()),
        redirect_uri: Some("http://localhost:8080/callback".to_string()),
        response_type: Some("token".to_string()), // Invalid response type
        scope: Some("user:email,repo".to_string()),
        state: Some("test-state".to_string()),
        code_challenge: Some("test-challenge".to_string()),
        code_challenge_method: Some("S256".to_string()),
    };
    
    let result = oauth_authorize(Query(params)).await;
    assert!(result.is_err());
    
    let error_response = result.unwrap_err();
    assert_eq!(error_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_oauth_callback_github_error() {
    let pool = setup_test_db().await;
    let app_state = AppState { db_pool: pool };
    
    let params = CallbackQuery {
        code: None,
        state: Some("test-state".to_string()),
        error: Some("access_denied".to_string()),
        error_description: Some("The user denied the request".to_string()),
    };
    
    let result = oauth_callback(State(app_state), Query(params)).await;
    assert!(result.is_err());
    
    let error_response = result.unwrap_err();
    assert_eq!(error_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_oauth_callback_missing_code() {
    let pool = setup_test_db().await;
    let app_state = AppState { db_pool: pool };
    
    let params = CallbackQuery {
        code: None, // Missing authorization code
        state: Some("test-state".to_string()),
        error: None,
        error_description: None,
    };
    
    let result = oauth_callback(State(app_state), Query(params)).await;
    assert!(result.is_err());
    
    let error_response = result.unwrap_err();
    assert_eq!(error_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_oauth_callback_missing_state() {
    let pool = setup_test_db().await;
    let app_state = AppState { db_pool: pool };
    
    let params = CallbackQuery {
        code: Some("github-auth-code".to_string()),
        state: None, // Missing state parameter
        error: None,
        error_description: None,
    };
    
    let result = oauth_callback(State(app_state), Query(params)).await;
    assert!(result.is_err());
    
    let error_response = result.unwrap_err();
    assert_eq!(error_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_oauth_token_invalid_grant_type() {
    let pool = setup_test_db().await;
    let app_state = AppState { db_pool: pool };
    
    let request = TokenRequest {
        grant_type: "password".to_string(), // Invalid grant type
        code: Some("test-code".to_string()),
        redirect_uri: Some("http://localhost:8080/callback".to_string()),
        client_id: Some("test-client".to_string()),
        code_verifier: Some("test-verifier".to_string()),
    };
    
    let result = oauth_token(State(app_state), Json(request)).await;
    assert!(result.is_err());
    
    let error_response = result.unwrap_err();
    assert_eq!(error_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_oauth_token_missing_code() {
    let pool = setup_test_db().await;
    let app_state = AppState { db_pool: pool };
    
    let request = TokenRequest {
        grant_type: "authorization_code".to_string(),
        code: None, // Missing authorization code
        redirect_uri: Some("http://localhost:8080/callback".to_string()),
        client_id: Some("test-client".to_string()),
        code_verifier: Some("test-verifier".to_string()),
    };
    
    let result = oauth_token(State(app_state), Json(request)).await;
    assert!(result.is_err());
    
    let error_response = result.unwrap_err();
    assert_eq!(error_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_oauth_token_invalid_authorization_code() {
    let pool = setup_test_db().await;
    let app_state = AppState { db_pool: pool };
    
    let request = TokenRequest {
        grant_type: "authorization_code".to_string(),
        code: Some("invalid-auth-code".to_string()), // Non-existent code
        redirect_uri: Some("http://localhost:8080/callback".to_string()),
        client_id: Some("test-client".to_string()),
        code_verifier: Some("test-verifier".to_string()),
    };
    
    let result = oauth_token(State(app_state), Json(request)).await;
    assert!(result.is_err());
    
    let error_response = result.unwrap_err();
    assert_eq!(error_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_jwt_token_mcp_session_creation() {
    let pool = setup_test_db().await;
    
    // Create test user
    let user = create_test_user(&pool, 123456).await;
    
    // Generate JWT token for MCP session
    let jwt_config = JwtConfig::default();
    let session_id = Uuid::new_v4();
    let jwt_token = generate_jwt_token(user.id, session_id, SessionType::Mcp, &jwt_config).unwrap();
    let token_hash = hash_token(&jwt_token);
    
    // Create MCP session
    let expires_at = Utc::now() + Duration::days(UserSession::MCP_SESSION_DURATION_DAYS);
    let session_data = CreateUserSession {
        user_id: user.id,
        token_hash: token_hash.clone(),
        session_type: SessionType::Mcp,
        client_info: Some("MCP Client Test".to_string()),
        expires_at,
    };
    
    let session = UserSession::create(&pool, &session_data, session_id).await.unwrap();
    
    // Verify session properties
    assert_eq!(session.id, session_id);
    assert_eq!(session.user_id, user.id);
    assert_eq!(session.token_hash, token_hash);
    assert_eq!(session.session_type, SessionType::Mcp);
    assert_eq!(session.client_info, Some("MCP Client Test".to_string()));
    assert!(session.expires_at > Utc::now());
    
    // Verify token can be validated
    let claims = automagik_forge::auth::validate_jwt_token(&jwt_token, &jwt_config).unwrap();
    assert_eq!(claims.sub, user.id.to_string());
    assert_eq!(claims.session_id, session_id.to_string());
    assert_eq!(claims.session_type, SessionType::Mcp);
}

#[tokio::test]
async fn test_session_expiration_handling() {
    let pool = setup_test_db().await;
    
    // Create test user
    let user = create_test_user(&pool, 123456).await;
    
    // Create expired session
    let jwt_config = JwtConfig::default();
    let session_id = Uuid::new_v4();
    let jwt_token = generate_jwt_token(user.id, session_id, SessionType::Web, &jwt_config).unwrap();
    let token_hash = hash_token(&jwt_token);
    
    // Set expiration to past time
    let expired_at = Utc::now() - Duration::hours(1);
    let session_data = CreateUserSession {
        user_id: user.id,
        token_hash: token_hash.clone(),
        session_type: SessionType::Web,
        client_info: Some("Expired Test Session".to_string()),
        expires_at: expired_at,
    };
    
    let session = UserSession::create(&pool, &session_data, session_id).await.unwrap();
    assert!(session.is_expired());
    assert!(!session.is_valid());
    assert!(session.time_remaining().is_none());
    
    // Finding valid session should return None
    let valid_session = UserSession::find_valid_by_token_hash(&pool, &token_hash).await.unwrap();
    assert!(valid_session.is_none());
    
    // Finding any session should still return the expired one
    let any_session = UserSession::find_by_token_hash(&pool, &token_hash).await.unwrap();
    assert!(any_session.is_some());
}

#[tokio::test]
async fn test_session_cleanup() {
    let pool = setup_test_db().await;
    
    // Create test user
    let user = create_test_user(&pool, 123456).await;
    
    // Create multiple sessions with different expiration times
    let mut session_data = vec![];
    
    // Valid session
    let valid_expires_at = Utc::now() + Duration::hours(1);
    session_data.push((valid_expires_at, "valid"));
    
    // Expired session 1
    let expired1_at = Utc::now() - Duration::hours(1);
    session_data.push((expired1_at, "expired1"));
    
    // Expired session 2
    let expired2_at = Utc::now() - Duration::days(1);
    session_data.push((expired2_at, "expired2"));
    
    // Create sessions
    for (expires_at, client_info) in session_data {
        let session_id = Uuid::new_v4();
        let jwt_token = generate_jwt_token(user.id, session_id, SessionType::Web, &JwtConfig::default()).unwrap();
        let token_hash = hash_token(&jwt_token);
        
        let create_data = CreateUserSession {
            user_id: user.id,
            token_hash,
            session_type: SessionType::Web,
            client_info: Some(client_info.to_string()),
            expires_at,
        };
        
        UserSession::create(&pool, &create_data, session_id).await.unwrap();
    }
    
    // Verify all sessions exist
    let all_sessions = UserSession::find_by_user_id(&pool, user.id).await.unwrap();
    assert_eq!(all_sessions.len(), 3);
    
    // Clean up expired sessions
    let cleaned_count = UserSession::cleanup_expired(&pool).await.unwrap();
    assert_eq!(cleaned_count, 2); // Should clean up 2 expired sessions
    
    // Verify only valid session remains
    let remaining_sessions = UserSession::find_by_user_id(&pool, user.id).await.unwrap();
    assert_eq!(remaining_sessions.len(), 1);
    assert_eq!(remaining_sessions[0].client_info, Some("valid".to_string()));
    
    // Verify active sessions query
    let active_sessions = UserSession::find_all_active(&pool).await.unwrap();
    assert_eq!(active_sessions.len(), 1);
}

#[tokio::test]
async fn test_session_type_specific_operations() {
    let pool = setup_test_db().await;
    
    // Create test user
    let user = create_test_user(&pool, 123456).await;
    
    // Create web session
    let web_session = UserSession::create_with_defaults(
        &pool,
        user.id,
        "web_token_hash".to_string(),
        SessionType::Web,
        Some("Web Client".to_string()),
    ).await.unwrap();
    
    // Create MCP session
    let mcp_session = UserSession::create_with_defaults(
        &pool,
        user.id,
        "mcp_token_hash".to_string(),
        SessionType::Mcp,
        Some("MCP Client".to_string()),
    ).await.unwrap();
    
    // Verify session types
    assert_eq!(web_session.session_type, SessionType::Web);
    assert_eq!(mcp_session.session_type, SessionType::Mcp);
    
    // MCP session should have longer expiration
    assert!(mcp_session.expires_at > web_session.expires_at);
    
    // Count active sessions by type
    let web_count = UserSession::count_active_by_user_and_type(
        &pool, user.id, SessionType::Web
    ).await.unwrap();
    assert_eq!(web_count, 1);
    
    let mcp_count = UserSession::count_active_by_user_and_type(
        &pool, user.id, SessionType::Mcp
    ).await.unwrap();
    assert_eq!(mcp_count, 1);
}

#[tokio::test]
async fn test_whitelist_environment_variable_override() {
    let pool = setup_test_db().await;
    
    // Test user not in database whitelist
    let github_id = 999999;
    let username = "env_whitelisted_user";
    
    // Should not be whitelisted initially
    let is_whitelisted = User::is_github_id_whitelisted(&pool, github_id).await.unwrap();
    assert!(!is_whitelisted);
    
    // Environment variable whitelist would be checked in the actual OAuth flow
    // Here we're testing the database lookup behavior when user doesn't exist
    let is_username_whitelisted = User::is_github_username_whitelisted(&pool, username).await.unwrap();
    assert!(!is_username_whitelisted);
}

#[tokio::test]
async fn test_user_creation_during_oauth_flow() {
    let pool = setup_test_db().await;
    
    // Simulate GitHub OAuth response data
    let github_id = 123456;
    let username = "oauth_test_user";
    let email = "oauth_test@example.com";
    let display_name = "OAuth Test User";
    let avatar_url = "https://github.com/oauth_test_user.png";
    let github_token = "github_oauth_token_123";
    
    // Add user to whitelist first
    add_to_whitelist(&pool, github_id, username).await;
    
    // Verify user doesn't exist initially
    let existing_user = User::find_by_github_id(&pool, github_id).await.unwrap();
    assert!(existing_user.is_none());
    
    // Create user as would happen in OAuth callback
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id,
        username: username.to_string(),
        email: email.to_string(),
        display_name: Some(display_name.to_string()),
        avatar_url: Some(avatar_url.to_string()),
        github_token: Some(github_token.to_string()),
        is_admin: Some(false),
    };
    
    let created_user = User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Verify user was created correctly
    assert_eq!(created_user.github_id, github_id);
    assert_eq!(created_user.username, username);
    assert_eq!(created_user.email, email);
    assert_eq!(created_user.display_name, Some(display_name.to_string()));
    assert_eq!(created_user.avatar_url, Some(avatar_url.to_string()));
    assert_eq!(created_user.github_token, Some(github_token.to_string()));
    assert!(!created_user.is_admin);
    assert!(created_user.is_whitelisted); // Default value
    
    // Verify user can be found by GitHub ID
    let found_user = User::find_by_github_id(&pool, github_id).await.unwrap();
    assert!(found_user.is_some());
    assert_eq!(found_user.unwrap().id, created_user.id);
}

#[tokio::test]
async fn test_user_update_during_oauth_flow() {
    let pool = setup_test_db().await;
    
    let github_id = 123456;
    
    // Create initial user
    let user = create_test_user(&pool, github_id).await;
    let original_username = user.username.clone();
    let original_email = user.email.clone();
    
    // Simulate updated data from GitHub OAuth
    let update_data = automagik_forge::models::user::UpdateUser {
        username: Some("updated_username".to_string()),
        email: Some("updated@example.com".to_string()),
        display_name: Some("Updated Display Name".to_string()),
        avatar_url: Some("https://github.com/updated.png".to_string()),
        github_token: Some("new_github_token".to_string()),
        is_admin: None, // Should not change
        is_whitelisted: None, // Should not change
    };
    
    let updated_user = User::update(&pool, user.id, &update_data).await.unwrap();
    
    // Verify updates
    assert_eq!(updated_user.id, user.id);
    assert_eq!(updated_user.github_id, github_id); // Should not change
    assert_eq!(updated_user.username, "updated_username");
    assert_eq!(updated_user.email, "updated@example.com");
    assert_eq!(updated_user.display_name, Some("Updated Display Name".to_string()));
    assert_eq!(updated_user.avatar_url, Some("https://github.com/updated.png".to_string()));
    assert_eq!(updated_user.github_token, Some("new_github_token".to_string()));
    assert_eq!(updated_user.is_admin, user.is_admin); // Should not change
    assert_eq!(updated_user.is_whitelisted, user.is_whitelisted); // Should not change
    assert!(updated_user.updated_at > user.updated_at);
}

#[tokio::test]
async fn test_multiple_session_types_per_user() {
    let pool = setup_test_db().await;
    
    // Create test user
    let user = create_test_user(&pool, 123456).await;
    
    // Create multiple sessions of different types
    let web_session1 = UserSession::create_with_defaults(
        &pool,
        user.id,
        "web_token_1".to_string(),
        SessionType::Web,
        Some("Web Browser 1".to_string()),
    ).await.unwrap();
    
    let web_session2 = UserSession::create_with_defaults(
        &pool,
        user.id,
        "web_token_2".to_string(),
        SessionType::Web,
        Some("Web Browser 2".to_string()),
    ).await.unwrap();
    
    let mcp_session1 = UserSession::create_with_defaults(
        &pool,
        user.id,
        "mcp_token_1".to_string(),
        SessionType::Mcp,
        Some("MCP Client 1".to_string()),
    ).await.unwrap();
    
    let mcp_session2 = UserSession::create_with_defaults(
        &pool,
        user.id,
        "mcp_token_2".to_string(),
        SessionType::Mcp,
        Some("MCP Client 2".to_string()),
    ).await.unwrap();
    
    // Verify session counts
    let web_count = UserSession::count_active_by_user_and_type(
        &pool, user.id, SessionType::Web
    ).await.unwrap();
    assert_eq!(web_count, 2);
    
    let mcp_count = UserSession::count_active_by_user_and_type(
        &pool, user.id, SessionType::Mcp
    ).await.unwrap();
    assert_eq!(mcp_count, 2);
    
    // Verify all sessions exist
    let all_user_sessions = UserSession::find_by_user_id(&pool, user.id).await.unwrap();
    assert_eq!(all_user_sessions.len(), 4);
    
    // Delete all sessions for user
    let deleted_count = UserSession::delete_all_for_user(&pool, user.id).await.unwrap();
    assert_eq!(deleted_count, 4);
    
    // Verify no sessions remain
    let remaining_sessions = UserSession::find_by_user_id(&pool, user.id).await.unwrap();
    assert_eq!(remaining_sessions.len(), 0);
}