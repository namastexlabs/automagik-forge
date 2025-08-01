use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{Json as ResponseJson, Response},
    routing::{get, post},
    Json, Router,
};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::AppState, 
    models::{
        ApiResponse,
        user::{User, CreateUser},
        user_session::{SessionType, UserSession},
    },
};
use crate::auth::{generate_jwt_token, hash_token, JwtConfig, get_current_user, get_user_context};

pub fn auth_router() -> Router<AppState> {
    Router::new()
        .route("/auth/github/device/start", post(device_start))
        .route("/auth/github/device/poll", post(device_poll))
        .route("/auth/github/check", get(github_check_token))
        .route("/auth/me", get(get_current_user_info))
        .route("/auth/logout", post(logout))
        .route("/auth/logout-all", post(logout_all))
}

#[derive(serde::Deserialize, ToSchema)]
struct DeviceStartRequest {}

#[derive(serde::Serialize, TS, ToSchema)]
#[ts(export)]
pub struct DeviceStartResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

#[derive(serde::Deserialize, ToSchema)]
pub struct DevicePollRequest {
    device_code: String,
}

#[derive(serde::Serialize, TS, ToSchema)]
#[ts(export)]
pub struct AuthResponse {
    pub access_token: String,
    pub user: User,
    pub session: UserSession,
}

#[derive(serde::Serialize, TS, ToSchema)]  
#[ts(export)]
pub struct UserInfoResponse {
    pub user: User,
    pub session: Option<UserSession>,
}

/// POST /auth/github/device/start
#[utoipa::path(
    post,
    path = "/auth/github/device/start",
    tag = "auth",
    summary = "Start GitHub OAuth device flow",
    description = "Initiates GitHub OAuth device authorization flow, returning device and user codes",
    responses(
        (status = 200, description = "Device authorization flow started successfully", body = ApiResponse<DeviceStartResponse>),
        (status = 500, description = "Failed to contact GitHub or parse response", body = ApiResponse<String>)
    )
)]
pub async fn device_start() -> ResponseJson<ApiResponse<DeviceStartResponse>> {
    let client_id = std::env::var("GITHUB_CLIENT_ID").unwrap_or_else(|_| "Ov23li2nd1KF5nCPbgoj".to_string());

    let params = [("client_id", client_id.as_str()), ("scope", "user:email,repo")];
    let client = reqwest::Client::new();
    let res = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) => {
            return ResponseJson(ApiResponse::error(&format!(
                "Failed to contact GitHub: {e}"
            )));
        }
    };
    let json: serde_json::Value = match res.json().await {
        Ok(j) => j,
        Err(e) => {
            return ResponseJson(ApiResponse::error(&format!(
                "Failed to parse GitHub response: {e}"
            )));
        }
    };
    if let (
        Some(device_code),
        Some(user_code),
        Some(verification_uri),
        Some(expires_in),
        Some(interval),
    ) = (
        json.get("device_code").and_then(|v| v.as_str()),
        json.get("user_code").and_then(|v| v.as_str()),
        json.get("verification_uri").and_then(|v| v.as_str()),
        json.get("expires_in").and_then(|v| v.as_u64()),
        json.get("interval").and_then(|v| v.as_u64()),
    ) {
        ResponseJson(ApiResponse::success(DeviceStartResponse {
            device_code: device_code.to_string(),
            user_code: user_code.to_string(),
            verification_uri: verification_uri.to_string(),
            expires_in: expires_in.try_into().unwrap_or(600),
            interval: interval.try_into().unwrap_or(5),
        }))
    } else {
        ResponseJson(ApiResponse::error(&format!("GitHub error: {}", json)))
    }
}

/// POST /auth/github/device/poll
#[utoipa::path(
    post,
    path = "/auth/github/device/poll",
    tag = "auth",
    summary = "Poll GitHub OAuth device flow",
    description = "Polls GitHub for OAuth device flow completion, validates whitelist, and creates JWT session",
    request_body = DevicePollRequest,
    responses(
        (status = 200, description = "GitHub login successful with JWT token", body = ApiResponse<AuthResponse>),
        (status = 400, description = "OAuth error or invalid device code", body = ApiResponse<String>),
        (status = 403, description = "User not whitelisted", body = ApiResponse<String>)
    )
)]
pub async fn device_poll(
    State(app_state): State<AppState>,
    Json(payload): Json<DevicePollRequest>,
) -> ResponseJson<ApiResponse<AuthResponse>> {
    let client_id = std::env::var("GITHUB_CLIENT_ID").unwrap_or_else(|_| "Ov23li2nd1KF5nCPbgoj".to_string());

    let params = [
        ("client_id", client_id.as_str()),
        ("device_code", payload.device_code.as_str()),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
    ];
    let client = reqwest::Client::new();
    
    // Get access token from GitHub
    let res = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) => {
            return ResponseJson(ApiResponse::error(&format!(
                "Failed to contact GitHub: {e}"
            )));
        }
    };
    let json: serde_json::Value = match res.json().await {
        Ok(j) => j,
        Err(e) => {
            return ResponseJson(ApiResponse::error(&format!(
                "Failed to parse GitHub response: {e}"
            )));
        }
    };
    
    if let Some(error) = json.get("error").and_then(|v| v.as_str()) {
        // Not authorized yet, or other error
        return ResponseJson(ApiResponse::error(error));
    }
    
    let access_token = json.get("access_token").and_then(|v| v.as_str());
    let Some(access_token) = access_token else {
        return ResponseJson(ApiResponse::error("No access token yet"));
    };

    // Fetch user info from GitHub
    let user_res = client
        .get("https://api.github.com/user")
        .bearer_auth(access_token)
        .header("User-Agent", "automagik-forge-app")
        .send()
        .await;
    let user_json: serde_json::Value = match user_res {
        Ok(res) => match res.json().await {
            Ok(json) => json,
            Err(e) => {
                return ResponseJson(ApiResponse::error(&format!(
                    "Failed to parse GitHub user response: {e}"
                )));
            }
        },
        Err(e) => {
            return ResponseJson(ApiResponse::error(&format!(
                "Failed to fetch user info: {e}"
            )));
        }
    };

    let github_id = user_json.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let username = user_json.get("login").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let display_name = user_json.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let avatar_url = user_json.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Fetch user emails
    let emails_res = client
        .get("https://api.github.com/user/emails")
        .bearer_auth(access_token)
        .header("User-Agent", "automagik-forge-app")
        .send()
        .await;
    let emails_json: serde_json::Value = match emails_res {
        Ok(res) => match res.json().await {
            Ok(json) => json,
            Err(e) => {
                return ResponseJson(ApiResponse::error(&format!(
                    "Failed to parse GitHub emails response: {e}"
                )));
            }
        },
        Err(e) => {
            return ResponseJson(ApiResponse::error(&format!(
                "Failed to fetch user emails: {e}"
            )));
        }
    };
    
    let primary_email = emails_json
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|email| {
                    email
                        .get("primary")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                })
                .and_then(|email| email.get("email").and_then(|v| v.as_str()))
        })
        .unwrap_or("")
        .to_string();

    // Check if user is whitelisted
    let is_whitelisted = match User::is_github_id_whitelisted(&app_state.db_pool, github_id).await {
        Ok(whitelisted) => whitelisted,
        Err(e) => {
            tracing::error!("Failed to check whitelist status: {}", e);
            return ResponseJson(ApiResponse::error("Failed to validate user access"));
        }
    };

    if !is_whitelisted {
        tracing::warn!("User {} (ID: {}) is not whitelisted", username, github_id);
        return ResponseJson(ApiResponse::error("User not authorized to access this application"));
    }

    // Create or update user in database
    let user = match User::find_by_github_id(&app_state.db_pool, github_id).await {
        Ok(Some(existing_user)) => {
            // Update existing user with latest GitHub info
            let update_data = crate::models::user::UpdateUser {
                username: Some(username.clone()),
                email: Some(primary_email.clone()),
                display_name: display_name.clone(),
                avatar_url: avatar_url.clone(),
                github_token: Some(access_token.to_string()),
                is_admin: None, // Don't change admin status
                is_whitelisted: None, // Don't change whitelist status
            };
            match User::update(&app_state.db_pool, existing_user.id, &update_data).await {
                Ok(updated_user) => updated_user,
                Err(e) => {
                    tracing::error!("Failed to update user: {}", e);
                    return ResponseJson(ApiResponse::error("Failed to update user information"));
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
                github_token: Some(access_token.to_string()),
                is_admin: Some(false), // New users are not admin by default
            };
            match User::create(&app_state.db_pool, &create_data, Uuid::new_v4()).await {
                Ok(new_user) => new_user,
                Err(e) => {
                    tracing::error!("Failed to create user: {}", e);
                    return ResponseJson(ApiResponse::error("Failed to create user account"));
                }
            }
        }
        Err(e) => {
            tracing::error!("Database error during user lookup: {}", e);
            return ResponseJson(ApiResponse::error("Database error"));
        }
    };

    // Generate JWT token
    let jwt_config = JwtConfig::default();
    let session_id = Uuid::new_v4();
    
    let jwt_token = match generate_jwt_token(user.id, session_id, SessionType::Web, &jwt_config) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to generate JWT token: {}", e);
            return ResponseJson(ApiResponse::error("Failed to generate session token"));
        }
    };

    // Hash token for database storage
    let token_hash = hash_token(&jwt_token);

    // Create session in database
    let session = match UserSession::create_with_defaults(
        &app_state.db_pool,
        user.id,
        token_hash,
        SessionType::Web,
        Some("Web Browser".to_string()),
    ).await {
        Ok(session) => session,
        Err(e) => {
            tracing::error!("Failed to create user session: {}", e);
            return ResponseJson(ApiResponse::error("Failed to create session"));
        }
    };

    // Update config for backwards compatibility (migration path)
    {
        let mut config = app_state.get_config().write().await;
        config.github.username = Some(username.clone());
        config.github.primary_email = Some(primary_email.clone());
        config.github.token = Some(access_token.to_string());
        config.github_login_acknowledged = true;
 
        let config_path = crate::utils::config_path();
        if let Err(e) = config.save(&config_path) {
            tracing::warn!("Failed to save config (non-critical): {}", e);
        }
    }

    // Update Sentry scope
    app_state.update_sentry_scope().await;
    
    // Track analytics
    let mut props = serde_json::Map::new();
    props.insert("username".to_string(), serde_json::Value::String(username.clone()));
    props.insert("email".to_string(), serde_json::Value::String(primary_email.clone()));
    let props = serde_json::Value::Object(props);
    app_state.track_analytics_event("$identify", Some(props)).await;

    let auth_response = AuthResponse {
        access_token: jwt_token,
        user,
        session,
    };

    ResponseJson(ApiResponse::success(auth_response))
}

/// GET /auth/github/check
#[utoipa::path(
    get,
    path = "/auth/github/check",
    tag = "auth",
    summary = "Check GitHub token validity",
    description = "Validates the stored GitHub access token by making a test API call (requires authentication)",
    responses(
        (status = 200, description = "GitHub token is valid"),
        (status = 401, description = "Not authenticated"),
        (status = 400, description = "GitHub token is invalid or missing")
    )
)]
pub async fn github_check_token(
    State(_app_state): State<AppState>,
    req: Request,
) -> ResponseJson<ApiResponse<()>> {
    // Extract user from authenticated request
    let Some(user) = get_current_user(&req) else {
        return ResponseJson(ApiResponse::error("Not authenticated"));
    };

    // Check if user has a GitHub token
    let Some(token) = &user.github_token else {
        return ResponseJson(ApiResponse::error("github_token_invalid"));
    };

    // Test the GitHub token
    let client = reqwest::Client::new();
    let res = client
        .get("https://api.github.com/user")
        .bearer_auth(token)
        .header("User-Agent", "automagik-forge-app")
        .send()
        .await;
    
    match res {
        Ok(r) if r.status().is_success() => ResponseJson(ApiResponse::success(())),
        _ => ResponseJson(ApiResponse::error("github_token_invalid")),
    }
}

/// GET /auth/me
#[utoipa::path(
    get,
    path = "/auth/me",
    tag = "auth",
    summary = "Get current user information",
    description = "Returns the currently authenticated user and session information",
    responses(
        (status = 200, description = "Current user information", body = ApiResponse<UserInfoResponse>),
        (status = 401, description = "Not authenticated", body = ApiResponse<String>)
    )
)]
pub async fn get_current_user_info(req: Request) -> ResponseJson<ApiResponse<UserInfoResponse>> {
    let Some(user_context) = get_user_context(&req) else {
        return ResponseJson(ApiResponse::error("Not authenticated"));
    };

    let user_info = UserInfoResponse {
        user: user_context.user.clone(),
        session: Some(user_context.session.clone()),
    };

    ResponseJson(ApiResponse::success(user_info))
}

/// POST /auth/logout
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    summary = "Logout current session",
    description = "Invalidates the current user session",
    responses(
        (status = 200, description = "Successfully logged out", body = ApiResponse<String>),
        (status = 401, description = "Not authenticated", body = ApiResponse<String>)
    )
)]
pub async fn logout(
    State(app_state): State<AppState>,
    req: Request,
) -> ResponseJson<ApiResponse<String>> {
    let Some(user_context) = get_user_context(&req) else {
        return ResponseJson(ApiResponse::error("Not authenticated"));
    };

    // Delete the current session
    match UserSession::delete(&app_state.db_pool, user_context.session.id).await {
        Ok(_) => ResponseJson(ApiResponse::success("Successfully logged out".to_string())),
        Err(e) => {
            tracing::error!("Failed to delete session during logout: {}", e);
            ResponseJson(ApiResponse::error("Failed to logout"))
        }
    }
}

/// POST /auth/logout-all
#[utoipa::path(
    post,
    path = "/auth/logout-all",
    tag = "auth",
    summary = "Logout all sessions",
    description = "Invalidates all user sessions for the current user",
    responses(
        (status = 200, description = "Successfully logged out all sessions", body = ApiResponse<String>),
        (status = 401, description = "Not authenticated", body = ApiResponse<String>)
    )
)]
pub async fn logout_all(
    State(app_state): State<AppState>,
    req: Request,
) -> ResponseJson<ApiResponse<String>> {
    let Some(user_context) = get_user_context(&req) else {
        return ResponseJson(ApiResponse::error("Not authenticated"));
    };

    // Delete all sessions for the user
    match UserSession::delete_all_for_user(&app_state.db_pool, user_context.user.id).await {
        Ok(deleted_count) => ResponseJson(ApiResponse::success(format!(
            "Successfully logged out {} session(s)",
            deleted_count
        ))),
        Err(e) => {
            tracing::error!("Failed to delete all sessions during logout: {}", e);
            ResponseJson(ApiResponse::error("Failed to logout all sessions"))
        }
    }
}

/// Middleware to set Sentry user context for every request
pub async fn sentry_user_context_middleware(
    State(app_state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    app_state.update_sentry_scope().await;
    next.run(req).await
}
