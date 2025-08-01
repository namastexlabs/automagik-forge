use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Json as ResponseJson, Redirect, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    auth::{generate_jwt_token, hash_token, JwtConfig},
    models::{
        user::{CreateUser, User},
        user_session::{SessionType, UserSession},
    },
};
use super::super::app_config::AppConfig;

/// Get GitHub client ID from configuration with hardcoded default
fn get_github_client_id() -> String {
    let config = AppConfig::load().unwrap_or_default();
    
    config.github_client_id
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "Ov23li2nd1KF5nCPbgoj".to_string())
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OAuthDiscoveryResponse {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub response_type: Option<String>,
    #[allow(dead_code)]
    pub scope: Option<String>,
    #[allow(dead_code)]
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    #[allow(dead_code)]
    pub client_id: Option<String>,
    pub code_verifier: Option<String>,
}

#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export)]
pub struct OAuthErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
    pub error_uri: Option<String>,
}

// In-memory storage for OAuth state and authorization codes
// In production, this should be backed by a database or Redis
lazy_static::lazy_static! {
    static ref OAUTH_STATES: std::sync::Mutex<HashMap<String, OAuthState>> = 
        std::sync::Mutex::new(HashMap::new());
    static ref AUTH_CODES: std::sync::Mutex<HashMap<String, AuthCode>> = 
        std::sync::Mutex::new(HashMap::new());
}

#[derive(Debug, Clone)]
struct OAuthState {
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: Option<String>,
    #[allow(dead_code)]
    pub code_challenge_method: Option<String>,
    #[allow(dead_code)]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
struct AuthCode {
    #[allow(dead_code)]
    pub code: String,
    pub user_id: Uuid,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: Option<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub fn oauth_router() -> Router<AppState> {
    Router::new()
        .route("/.well-known/oauth-authorization-server", get(oauth_discovery))
        .route("/oauth/authorize", get(oauth_authorize))
        .route("/oauth/callback", get(oauth_callback))
        .route("/oauth/token", get(oauth_token).post(oauth_token))
}

/// GET /.well-known/oauth-authorization-server
#[utoipa::path(
    get,
    path = "/.well-known/oauth-authorization-server",
    tag = "oauth",
    summary = "OAuth 2.1 Authorization Server Discovery",
    description = "Returns OAuth 2.1 discovery metadata for MCP clients",
    responses(
        (status = 200, description = "OAuth discovery metadata", body = OAuthDiscoveryResponse)
    )
)]
pub async fn oauth_discovery() -> ResponseJson<OAuthDiscoveryResponse> {
    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());
    
    let discovery = OAuthDiscoveryResponse {
        issuer: base_url.clone(),
        authorization_endpoint: format!("{}/oauth/authorize", base_url),
        token_endpoint: format!("{}/oauth/token", base_url),
        response_types_supported: vec!["code".to_string()],
        grant_types_supported: vec!["authorization_code".to_string()],
        code_challenge_methods_supported: vec!["S256".to_string()],
    };

    ResponseJson(discovery)
}

/// GET /oauth/authorize
#[utoipa::path(
    get,
    path = "/oauth/authorize",
    tag = "oauth",
    summary = "OAuth 2.1 Authorization Endpoint",
    description = "Redirects to GitHub OAuth for MCP client authorization",
    responses(
        (status = 302, description = "Redirect to GitHub OAuth"),
        (status = 400, description = "Invalid request parameters", body = OAuthErrorResponse)
    )
)]
pub async fn oauth_authorize(
    Query(params): Query<AuthorizeQuery>,
) -> Result<Redirect, Response> {
    // Validate required parameters
    let client_id = params.client_id.as_deref().unwrap_or("mcp-client");
    let redirect_uri = match params.redirect_uri {
        Some(uri) => uri,
        None => {
            return Err(oauth_error_response(
                "invalid_request",
                Some("Missing redirect_uri parameter"),
            ));
        }
    };

    let response_type = params.response_type.as_deref().unwrap_or("code");
    if response_type != "code" {
        return Err(oauth_error_response(
            "unsupported_response_type",
            Some("Only 'code' response type is supported"),
        ));
    }

    // Generate state for CSRF protection
    let oauth_state = Uuid::new_v4().to_string();
    
    // Store OAuth state
    {
        let mut states = OAUTH_STATES.lock().unwrap();
        states.insert(
            oauth_state.clone(),
            OAuthState {
                client_id: client_id.to_string(),
                redirect_uri: redirect_uri.clone(),
                code_challenge: params.code_challenge,
                code_challenge_method: params.code_challenge_method,
                created_at: chrono::Utc::now(),
            },
        );
    }

    // GitHub OAuth configuration
    let github_client_id = get_github_client_id();
    let base_url = std::env::var("BASE_URL")
        .unwrap_or_else(|_| "http://localhost:3001".to_string());
    let callback_uri = format!("{}/oauth/callback", base_url);

    // Build GitHub OAuth URL
    let github_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user:email,repo&state={}",
        urlencoding::encode(&github_client_id),
        urlencoding::encode(&callback_uri),
        urlencoding::encode(&oauth_state)
    );

    Ok(Redirect::temporary(&github_url))
}

/// GET /oauth/callback
#[utoipa::path(
    get,
    path = "/oauth/callback",
    tag = "oauth",
    summary = "OAuth 2.1 Callback Endpoint",
    description = "Handles GitHub OAuth callback and issues authorization code",
    responses(
        (status = 302, description = "Redirect back to client with authorization code"),
        (status = 400, description = "OAuth error", body = OAuthErrorResponse)
    )
)]
pub async fn oauth_callback(
    State(app_state): State<AppState>,
    Query(params): Query<CallbackQuery>,
) -> Result<Redirect, Response> {
    // Handle GitHub OAuth errors
    if let Some(error) = params.error {
        let error_description = params.error_description
            .unwrap_or_else(|| "GitHub OAuth failed".to_string());
        return Err(oauth_error_response(&error, Some(&error_description)));
    }

    let github_code = match params.code {
        Some(code) => code,
        None => {
            return Err(oauth_error_response(
                "invalid_request",
                Some("Missing authorization code from GitHub"),
            ));
        }
    };

    let state = match params.state {
        Some(state) => state,
        None => {
            return Err(oauth_error_response(
                "invalid_request",
                Some("Missing state parameter"),
            ));
        }
    };

    // Retrieve and validate OAuth state
    let oauth_state = {
        let mut states = OAUTH_STATES.lock().unwrap();
        match states.remove(&state) {
            Some(state) => state,
            None => {
                return Err(oauth_error_response(
                    "invalid_request",
                    Some("Invalid or expired state parameter"),
                ));
            }
        }
    };

    // Exchange GitHub code for access token
    let github_client_id = get_github_client_id();
    let github_client_secret = std::env::var("GITHUB_CLIENT_SECRET")
        .unwrap_or_else(|_| "".to_string());

    let client = reqwest::Client::new();
    let token_params = [
        ("client_id", github_client_id.as_str()),
        ("client_secret", github_client_secret.as_str()),
        ("code", github_code.as_str()),
    ];

    let token_response = match client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&token_params)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Failed to contact GitHub OAuth server"),
            ));
        }
    };

    let token_json: serde_json::Value = match token_response.json().await {
        Ok(json) => json,
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Failed to parse GitHub token response"),
            ));
        }
    };

    if let Some(error) = token_json.get("error") {
        let error_description = token_json
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("GitHub token exchange failed");
        return Err(oauth_error_response(
            error.as_str().unwrap_or("server_error"),
            Some(error_description),
        ));
    }

    let github_access_token = match token_json.get("access_token").and_then(|v| v.as_str()) {
        Some(token) => token,
        None => {
            return Err(oauth_error_response(
                "server_error",
                Some("No access token received from GitHub"),
            ));
        }
    };

    // Fetch user info from GitHub
    let user_response = match client
        .get("https://api.github.com/user")
        .bearer_auth(github_access_token)
        .header("User-Agent", "automagik-forge-mcp")
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Failed to fetch user info from GitHub"),
            ));
        }
    };

    let user_json: serde_json::Value = match user_response.json().await {
        Ok(json) => json,
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Failed to parse GitHub user response"),
            ));
        }
    };

    let github_id = user_json.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let username = user_json.get("login").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let display_name = user_json.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let avatar_url = user_json.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Fetch user emails
    let emails_response = match client
        .get("https://api.github.com/user/emails")
        .bearer_auth(github_access_token)
        .header("User-Agent", "automagik-forge-mcp")
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Failed to fetch user emails from GitHub"),
            ));
        }
    };

    let emails_json: serde_json::Value = match emails_response.json().await {
        Ok(json) => json,
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Failed to parse GitHub emails response"),
            ));
        }
    };

    let primary_email = emails_json
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|email| {
                    email.get("primary").and_then(|v| v.as_bool()).unwrap_or(false)
                })
                .and_then(|email| email.get("email").and_then(|v| v.as_str()))
        })
        .unwrap_or("")
        .to_string();

    // Check if whitelist is disabled via environment variable
    let whitelist_disabled = std::env::var("DISABLE_GITHUB_WHITELIST")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    if !whitelist_disabled {
        // Check whitelist: first try environment variable, then database
        let mut is_whitelisted = false;
        
        // Check environment variable whitelist first
        if let Ok(env_whitelist) = std::env::var("GITHUB_WHITELIST") {
            let whitelisted_users: Vec<&str> = env_whitelist
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            
            if whitelisted_users.contains(&username.as_str()) {
                is_whitelisted = true;
                tracing::info!("User {} (ID: {}) allowed via environment whitelist", username, github_id);
            }
        }
        
        // If not in environment whitelist, check database
        if !is_whitelisted {
            match User::is_github_id_whitelisted(&app_state.db_pool, github_id).await {
                Ok(whitelisted) => {
                    is_whitelisted = whitelisted;
                    if is_whitelisted {
                        tracing::info!("User {} (ID: {}) allowed via database whitelist", username, github_id);
                    }
                }
                Err(_) => {
                    return Err(oauth_error_response(
                        "server_error",
                        Some("Failed to validate user access"),
                    ));
                }
            }
        }

        if !is_whitelisted {
            tracing::warn!("User {} (ID: {}) is not in whitelist", username, github_id);
            return Err(oauth_error_response(
                "access_denied",
                Some("User not authorized to access this application"),
            ));
        }
    } else {
        tracing::info!("GitHub whitelist is disabled, allowing user {} (ID: {})", username, github_id);
    }

    // Create or update user in database
    let user = match User::find_by_github_id(&app_state.db_pool, github_id).await {
        Ok(Some(existing_user)) => {
            // Update existing user
            let update_data = crate::models::user::UpdateUser {
                username: Some(username.clone()),
                email: Some(primary_email.clone()),
                display_name: display_name.clone(),
                avatar_url: avatar_url.clone(),
                github_token: Some(github_access_token.to_string()),
                is_admin: None,
                is_whitelisted: None,
            };
            match User::update(&app_state.db_pool, existing_user.id, &update_data).await {
                Ok(updated_user) => updated_user,
                Err(_) => {
                    return Err(oauth_error_response(
                        "server_error",
                        Some("Failed to update user information"),
                    ));
                }
            }
        }
        Ok(None) => {
            // Create new user
            let create_data = CreateUser {
                github_id,
                username: username.clone(),
                email: primary_email.clone(),
                display_name,
                avatar_url,
                github_token: Some(github_access_token.to_string()),
                is_admin: Some(false),
            };
            match User::create(&app_state.db_pool, &create_data, Uuid::new_v4()).await {
                Ok(new_user) => new_user,
                Err(_) => {
                    return Err(oauth_error_response(
                        "server_error",
                        Some("Failed to create user account"),
                    ));
                }
            }
        }
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Database error during user lookup"),
            ));
        }
    };

    // Generate authorization code
    let auth_code = Uuid::new_v4().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(10); // 10 minute expiry

    // Store authorization code
    {
        let mut codes = AUTH_CODES.lock().unwrap();
        codes.insert(
            auth_code.clone(),
            AuthCode {
                code: auth_code.clone(),
                user_id: user.id,
                client_id: oauth_state.client_id,
                redirect_uri: oauth_state.redirect_uri.clone(),
                code_challenge: oauth_state.code_challenge,
                expires_at,
            },
        );
    }

    // Build redirect URL with authorization code
    let separator = if oauth_state.redirect_uri.contains('?') { "&" } else { "?" };
    let redirect_url = format!(
        "{}{}code={}",
        oauth_state.redirect_uri,
        separator,
        urlencoding::encode(&auth_code)
    );

    Ok(Redirect::temporary(&redirect_url))
}

/// GET|POST /oauth/token
#[utoipa::path(
    post,
    path = "/oauth/token",
    tag = "oauth",
    summary = "OAuth 2.1 Token Endpoint",
    description = "Exchanges authorization code for access token",
    request_body = TokenRequest,
    responses(
        (status = 200, description = "Access token issued", body = TokenResponse),
        (status = 400, description = "Invalid token request", body = OAuthErrorResponse)
    )
)]
pub async fn oauth_token(
    State(app_state): State<AppState>,
    Json(request): Json<TokenRequest>,
) -> Result<ResponseJson<TokenResponse>, Response> {
    // Validate grant type
    if request.grant_type != "authorization_code" {
        return Err(oauth_error_response(
            "unsupported_grant_type",
            Some("Only 'authorization_code' grant type is supported"),
        ));
    }

    let code = match request.code {
        Some(code) => code,
        None => {
            return Err(oauth_error_response(
                "invalid_request",
                Some("Missing authorization code"),
            ));
        }
    };

    // Retrieve and validate authorization code
    let auth_code = {
        let mut codes = AUTH_CODES.lock().unwrap();
        match codes.remove(&code) {
            Some(code) => code,
            None => {
                return Err(oauth_error_response(
                    "invalid_grant",
                    Some("Invalid or expired authorization code"),
                ));
            }
        }
    };

    // Validate expiry
    if chrono::Utc::now() > auth_code.expires_at {
        return Err(oauth_error_response(
            "invalid_grant",
            Some("Authorization code has expired"),
        ));
    }

    // Validate redirect URI if provided
    if let Some(redirect_uri) = request.redirect_uri {
        if redirect_uri != auth_code.redirect_uri {
            return Err(oauth_error_response(
                "invalid_grant",
                Some("Redirect URI mismatch"),
            ));
        }
    }

    // Validate PKCE if code challenge was provided
    if let Some(challenge) = auth_code.code_challenge {
        let verifier = match request.code_verifier {
            Some(verifier) => verifier,
            None => {
                return Err(oauth_error_response(
                    "invalid_request",
                    Some("Missing code verifier for PKCE"),
                ));
            }
        };

        // Verify PKCE challenge (S256 method)
        use sha2::{Digest, Sha256};
        use base64::{Engine as _, engine::general_purpose};
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let computed_challenge = general_purpose::STANDARD.encode(hasher.finalize())
            .trim_end_matches('=')
            .replace('+', "-")
            .replace('/', "_");

        if computed_challenge != challenge {
            return Err(oauth_error_response(
                "invalid_grant",
                Some("Invalid PKCE code verifier"),
            ));
        }
    }

    // Generate session ID first
    let session_id = Uuid::new_v4();
    
    // Generate JWT token for MCP session
    let jwt_config = JwtConfig::default();

    let jwt_token = match generate_jwt_token(
        auth_code.user_id,
        session_id,
        SessionType::Mcp,
        &jwt_config,
    ) {
        Ok(token) => token,
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Failed to generate access token"),
            ));
        }
    };

    // Hash token for database storage
    let token_hash = hash_token(&jwt_token);

    // Create MCP session in database with the same session_id used in JWT
    let expires_at = chrono::Utc::now() + chrono::Duration::days(UserSession::MCP_SESSION_DURATION_DAYS);
    let session_data = crate::models::user_session::CreateUserSession {
        user_id: auth_code.user_id,
        token_hash,
        session_type: SessionType::Mcp,
        client_info: Some(format!("MCP Client: {}", auth_code.client_id)),
        expires_at,
    };

    match UserSession::create(&app_state.db_pool, &session_data, session_id).await {
        Ok(_) => {}
        Err(_) => {
            return Err(oauth_error_response(
                "server_error",
                Some("Failed to create session"),
            ));
        }
    }

    // TODO: Also store token in rmcp OAuth store for direct MCP server integration
    // This would require access to the AuthenticatedTaskServer's OAuth store
    // For now, the JWT token will be validated by both JWT validation and OAuth store lookup

    let token_response = TokenResponse {
        access_token: jwt_token,
        token_type: "Bearer".to_string(),
        expires_in: UserSession::MCP_SESSION_DURATION_DAYS * 24 * 60 * 60,
        scope: Some("mcp tasks:read tasks:write".to_string()),
    };

    Ok(ResponseJson(token_response))
}

fn oauth_error_response(error: &str, description: Option<&str>) -> Response {
    let error_response = OAuthErrorResponse {
        error: error.to_string(),
        error_description: description.map(|s| s.to_string()),
        error_uri: None,
    };

    let json_response = match serde_json::to_string(&error_response) {
        Ok(json) => json,
        Err(_) => r#"{"error":"server_error","error_description":"Failed to serialize error"}"#.to_string(),
    };

    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header("Content-Type", "application/json")
        .body(json_response.into())
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Server Error".into())
                .unwrap()
        })
}