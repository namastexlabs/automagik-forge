# Multiuser Automagik-Forge Implementation Plan

## Overview
Transform automagik-forge from single-user to multiuser collaborative kanban where each server instance represents a team. All users authenticate via GitHub OAuth (both web UI and MCP connections) and collaborate on shared projects with their individual GitHub identities for commits.

## Core Philosophy
- **GitHub OAuth Everywhere**: Web UI and MCP connections require GitHub authentication
- **One Server = One Team**: No complex team management, just authorized users collaborating
- **GitHub Identity**: Each user commits with their own GitHub account
- **Shared Projects**: All authenticated users can access all projects on the server
- **Individual Attribution**: Track who did what for accountability
- **Whitelist Security**: Only approved GitHub accounts can access the server

---

## Current State Analysis

### ✅ Strengths
- Strong Rust/Axum backend with type safety
- Existing GitHub OAuth flow (`/auth/github/*`)
- Real-time SSE streams for live updates
- Solid git workflow with worktrees per task attempt
- Task execution system ready for multi-user

### ❌ Critical Issues
- **No user context**: All data globally accessible (security risk)
- **Local config storage**: GitHub tokens in filesystem instead of per-user
- **Unscoped queries**: `Project::find_all()` returns ALL projects without user context
- **Single auth token**: Only one GitHub token stored globally

---

## Database Schema Changes

### New Tables
```sql
-- Core user table with whitelist support
CREATE TABLE users (
    id BLOB PRIMARY KEY,
    github_id INTEGER UNIQUE NOT NULL,
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    display_name TEXT,
    avatar_url TEXT,
    github_token TEXT, -- Encrypted OAuth token for commits
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    is_whitelisted BOOLEAN NOT NULL DEFAULT TRUE, -- Set false to revoke access
    last_login_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- User sessions for web and MCP auth
CREATE TABLE user_sessions (
    id BLOB PRIMARY KEY,
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT UNIQUE NOT NULL,
    session_type TEXT NOT NULL DEFAULT 'web' CHECK (session_type IN ('web', 'mcp')),
    client_info TEXT, -- Store MCP client info for audit
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- GitHub account whitelist (can be pre-populated)
CREATE TABLE github_whitelist (
    id BLOB PRIMARY KEY,
    github_username TEXT UNIQUE NOT NULL,
    github_id INTEGER UNIQUE,
    invited_by BLOB REFERENCES users(id),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- User preferences (migrated from config.json)
CREATE TABLE user_preferences (
    id BLOB PRIMARY KEY,
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    theme TEXT NOT NULL DEFAULT 'system',
    sound_alerts BOOLEAN NOT NULL DEFAULT TRUE,
    sound_file TEXT NOT NULL DEFAULT 'abstract-sound4',
    push_notifications BOOLEAN NOT NULL DEFAULT TRUE,
    editor_type TEXT NOT NULL DEFAULT 'vscode',
    editor_custom_command TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE(user_id)
);
```

### Modified Tables
```sql
-- Add user tracking to existing tables
ALTER TABLE projects ADD COLUMN created_by BLOB REFERENCES users(id);
ALTER TABLE tasks ADD COLUMN created_by BLOB REFERENCES users(id);
ALTER TABLE tasks ADD COLUMN assigned_to BLOB REFERENCES users(id);
ALTER TABLE task_attempts ADD COLUMN created_by BLOB REFERENCES users(id);

-- Indexes for performance
CREATE INDEX idx_projects_created_by ON projects(created_by);
CREATE INDEX idx_tasks_created_by ON tasks(created_by);
CREATE INDEX idx_tasks_assigned_to ON tasks(assigned_to);
CREATE INDEX idx_task_attempts_created_by ON task_attempts(created_by);
```

---

## MCP OAuth Integration

### Simple SSE OAuth with rmcp
Using rmcp's built-in OAuth support for automatic browser opening and token management:

```rust
// Add auth feature to Cargo.toml
// rmcp = { version = "0.1", features = ["auth", "transport-sse-server"] }

use rmcp::auth::{OAuthStore, AccessToken};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct AuthenticatedTaskServer {
    pub pool: SqlitePool,
    pub oauth_store: Arc<RwLock<OAuthStore>>,
    tool_router: ToolRouter<TaskServer>,
}

impl AuthenticatedTaskServer {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            oauth_store: Arc::new(RwLock::new(OAuthStore::new())),
            tool_router: Self::tool_router(),
        }
    }
}
```

### OAuth SSE Server Implementation 
```rust
// OAuth validation middleware for SSE connections
async fn validate_oauth_token(
    State(oauth_store): State<Arc<RwLock<OAuthStore>>>,
    headers: HeaderMap,
    mut req: Request<Body>,
    next: Next<Body>,
) -> Response {
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let store = oauth_store.read().await;
                if let Some(access_token) = store.get_access_token(token) {
                    // Get user from GitHub token stored in access_token
                    if let Ok(user) = get_user_by_github_token(&access_token.github_token).await {
                        if user.is_whitelisted {
                            req.extensions_mut().insert(user);
                            return next.run(req).await;
                        }
                    }
                }
            }
        }
    }
    
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("WWW-Authenticate", "Bearer")
        .body(Body::from("OAuth token required"))
        .unwrap()
}

// OAuth authorization endpoint that redirects to GitHub
pub async fn oauth_authorize(
    State(app_state): State<AppState>,
    Query(params): Query<OAuthAuthorizeRequest>,
) -> Response {
    // Store OAuth client info for callback
    let state = format!("mcp_{}", Uuid::new_v4());
    app_state.oauth_store.write().await.store_oauth_session(
        &state, 
        params.client_id, 
        params.redirect_uri,
        params.code_challenge
    );
    
    // Redirect to GitHub OAuth
    let github_auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope=user:email&state={}",
        env::var("GITHUB_CLIENT_ID").unwrap(),
        state
    );
    
    Redirect::temporary(&github_auth_url).into_response()
}

// GitHub OAuth callback that issues MCP tokens
pub async fn oauth_callback(
    State(app_state): State<AppState>,
    Query(params): Query<GitHubCallbackRequest>,
) -> Response {
    // Exchange GitHub code for access token
    let github_user = exchange_github_code(&params.code).await?;
    
    // Check whitelist
    if !GitHubWhitelist::is_allowed(&app_state.db, &github_user.login).await? {
        return error_response("GitHub account not authorized");
    }
    
    // Create/update user
    let user = User::create_or_update_from_github(&app_state.db, &github_user).await?;
    
    // Issue MCP access token
    let mcp_token = Uuid::new_v4().to_string();
    app_state.oauth_store.write().await.create_access_token(
        &mcp_token,
        user.id,
        user.github_token.clone(),
        vec!["mcp".to_string(), "tasks:read".to_string(), "tasks:write".to_string()]
    );
    
    // Redirect back to MCP client with authorization code
    let oauth_session = app_state.oauth_store.read().await.get_session(&params.state)?;
    let callback_url = format!("{}?code={}&state={}", 
        oauth_session.redirect_uri, 
        mcp_token, 
        params.state
    );
    
    Redirect::temporary(&callback_url).into_response()
}
```

### Client-Side Automatic Browser Opening
External MCP clients using rmcp will automatically handle browser opening:

```rust
// Client configuration (what users will set up)
let mut oauth_state = OAuthState::new("http://192.168.112.154:8889", None).await?;
oauth_state.start_authorization(&["mcp", "tasks:read", "tasks:write"], "http://localhost:8080/callback").await?;

// rmcp automatically opens browser with authorization URL
let auth_url = oauth_state.get_authorization_url().await?;
// Browser opens automatically, user authenticates via GitHub

// Client runs local callback server to receive auth code
let auth_code = receive_callback_code().await?; // rmcp handles this
let credentials = oauth_state.handle_callback(&auth_code).await?;

// Create authenticated SSE transport
let transport = create_authorized_transport(
    "http://192.168.112.154:8889/sse",
    oauth_state,
    Some(retry_config)
).await?;

let client = client_service.serve(transport).await?;
```

### Simple MCP Authentication Flow
1. **Client connects to SSE**: `"automagik-forge": { "type": "sse", "url": "http://192.168.112.154:8889/sse" }`
2. **OAuth required**: Server returns 401 with WWW-Authenticate header  
3. **Browser opens automatically**: rmcp opens GitHub OAuth in user's browser
4. **GitHub authentication**: User logs in with GitHub (whitelist validated)
5. **Token issued**: Server issues MCP access token tied to GitHub user
6. **SSE connection authenticated**: All subsequent requests include Bearer token

---

## Authentication System

### Dual Authentication (Web + MCP)
- **Web UI**: Traditional GitHub OAuth with JWT sessions
- **MCP Connections**: OAuth 2.1 flow with GitHub identity verification  
- **Unified User Context**: Both authentication methods create same user records

### JWT-Based Sessions
- Replace global config with per-user JWT tokens
- Store session tokens in `user_sessions` table with expiration
- Middleware extracts user context from Authorization header

### GitHub OAuth with Whitelist Validation
```rust
// Updated auth flow with whitelist check
pub async fn device_poll(
    State(app_state): State<AppState>,
    Json(payload): Json<DevicePollRequest>,
) -> ResponseJson<ApiResponse<AuthResponse>> {
    // ... GitHub OAuth flow ...
    
    // Check whitelist first
    if !GitHubWhitelist::is_allowed(&app_state.db, &username).await? {
        return ResponseJson(ApiResponse::error("GitHub account not authorized for this server"));
    }
    
    // Create or update user (auto-whitelisted if they got here)
    let user = User::create_or_update_from_github(
        &app_state.db, 
        github_id, 
        username, 
        email, 
        access_token
    ).await?;
    
    // Create session token (web type)
    let session_token = UserSession::create(&app_state.db, user.id, SessionType::Web).await?;
    
    ResponseJson(ApiResponse::success(AuthResponse {
        token: session_token,
        user: user.into_public(),
    }))
}

// MCP OAuth authorization endpoint
pub async fn oauth_authorize(
    State(app_state): State<AppState>,
    Query(params): Query<OAuthAuthorizeRequest>,
) -> Response {
    // Validate MCP client request
    let client_id = params.client_id;
    let scopes = params.scope.split(' ').collect::<Vec<_>>();
    
    // Redirect to GitHub OAuth with state parameter
    let github_auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope=user:email&state=mcp_{}",
        env::var("GITHUB_CLIENT_ID").unwrap(),
        params.state
    );
    
    Redirect::temporary(&github_auth_url).into_response()
}

// Handle GitHub callback for MCP auth
pub async fn oauth_callback_mcp(
    State(app_state): State<AppState>,
    Query(params): Query<GitHubCallbackRequest>,
) -> Response {
    // Exchange code for GitHub access token
    let github_user = get_github_user_info(&params.code).await?;
    
    // Check whitelist
    if !GitHubWhitelist::is_allowed(&app_state.db, &github_user.login).await? {
        return error_response("GitHub account not authorized for this server");
    }
    
    // Create user and MCP session
    let user = User::create_or_update_from_github(
        &app_state.db,
        github_user.id,
        github_user.login,
        github_user.email,
        github_access_token,
    ).await?;
    
    let mcp_token = UserSession::create(&app_state.db, user.id, SessionType::Mcp).await?;
    
    // Return authorization code to MCP client
    let callback_url = format!("{}?code={}&state={}", 
        params.redirect_uri, 
        mcp_token, 
        params.state
    );
    
    Redirect::temporary(&callback_url).into_response()
}
```

### User Context Middleware
```rust
pub async fn auth_middleware(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    mut req: Request,
    next: Next,
) -> Response {
    if let Some(token) = extract_bearer_token(&headers) {
        if let Ok(user) = UserSession::get_user_by_token(&app_state.db, &token).await {
            // Validate user is still whitelisted and active
            if user.is_whitelisted {
                req.extensions_mut().insert(user);
            } else {
                return Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Body::from("Account access revoked"))
                    .unwrap();
            }
        }
    }
    next.run(req).await
}

// MCP-specific middleware for tool calls
pub async fn mcp_auth_middleware(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    mut req: Request,
    next: Next,
) -> Response {
    if let Some(token) = extract_bearer_token(&headers) {
        if let Ok(session) = UserSession::get_by_token(&app_state.db, &token).await {
            if session.session_type == SessionType::Mcp && session.user.is_whitelisted {
                req.extensions_mut().insert(session.user);
                return next.run(req).await;
            }
        }
    }
    
    // Return OAuth error for MCP clients
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("WWW-Authenticate", "Bearer realm=\"MCP\", error=\"invalid_token\"")
        .body(Body::from("MCP authentication required"))
        .unwrap()
}
```

---

## Data Access Patterns

### User-Scoped Queries
All data access functions need user context, but projects remain shared:

```rust
impl Project {
    // Projects are shared but track who created them
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<ProjectWithCreator>, sqlx::Error> {
        sqlx::query_as!(
            ProjectWithCreator,
            r#"SELECT 
                p.id, p.name, p.git_repo_path, p.setup_script, p.dev_script, p.cleanup_script,
                p.created_at, p.updated_at, p.created_by,
                u.username as creator_username, u.display_name as creator_display_name
               FROM projects p
               LEFT JOIN users u ON p.created_by = u.id
               ORDER BY p.created_at DESC"#
        )
        .fetch_all(pool)
        .await
    }
}

impl Task {
    // Tasks can be assigned to specific users
    pub async fn find_by_project_id_with_users(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<TaskWithUsers>, sqlx::Error> {
        sqlx::query_as!(
            TaskWithUsers,
            r#"SELECT 
                t.*, 
                creator.username as creator_username,
                assignee.username as assignee_username
               FROM tasks t
               LEFT JOIN users creator ON t.created_by = creator.id
               LEFT JOIN users assignee ON t.assigned_to = assignee.id
               WHERE t.project_id = $1
               ORDER BY t.created_at DESC"#,
            project_id
        )
        .fetch_all(pool)
        .await
    }
}
```

---

## Git Integration

### Per-User Commits
Each task attempt uses the assigned user's GitHub token and identity:

```rust
impl TaskAttempt {
    pub async fn create_with_user_context(
        pool: &SqlitePool,
        task_id: Uuid,
        user: &User,
        worktree_path: String,
    ) -> Result<Self, sqlx::Error> {
        // Configure git with user's identity
        let repo = Repository::open(&worktree_path)?;
        let config = repo.config()?;
        config.set_str("user.name", &user.display_name.unwrap_or(user.username.clone()))?;
        config.set_str("user.email", &user.email)?;
        
        // Store user's GitHub token for pushes
        // (encrypted in database)
        
        Self::create(pool, task_id, user.id, worktree_path).await
    }
}
```

### Commit Attribution
```rust
// When creating commits, use user's GitHub identity
pub async fn commit_changes(
    attempt: &TaskAttempt, 
    user: &User, 
    message: &str
) -> Result<Oid, git2::Error> {
    let repo = Repository::open(&attempt.worktree_path)?;
    let signature = Signature::now(&user.display_name.unwrap_or(user.username.clone()), &user.email)?;
    
    // Create commit with user's signature
    let tree = repo.index()?.write_tree()?;
    let tree = repo.find_tree(tree)?;
    let parent = repo.head()?.peel_to_commit()?;
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&parent],
    )
}
```

---

## API Changes

### Authentication Required
All API endpoints except `/auth/*` and `/health` require authentication:

```rust
// Apply auth middleware to all protected routes
let protected_routes = Router::new()
    .merge(projects_router())
    .merge(tasks_router())
    .merge(task_attempts_router())
    .layer(from_fn_with_state(app_state.clone(), auth_middleware));

let app = Router::new()
    .nest("/api", protected_routes)
    .merge(auth_router()) // No auth required
    .route("/api/health", get(health_check)) // No auth required
```

### User Context in Handlers
```rust
pub async fn create_task(
    State(app_state): State<AppState>,
    Extension(user): Extension<User>, // Injected by auth middleware
    Json(payload): Json<CreateTask>,
) -> ResponseJson<ApiResponse<Task>> {
    let task = Task::create(&app_state.db, &payload, user.id).await?;
    ResponseJson(ApiResponse::success(task))
}
```

---

## Real-time Collaboration

### User-Aware SSE Streams
```rust
pub async fn project_stream(
    State(app_state): State<AppState>,
    Extension(user): Extension<User>,
    Path(project_id): Path<Uuid>,
) -> Sse<impl Stream<Item = Event>> {
    // Subscribe user to project-specific events
    let stream = app_state.subscribe_to_project_events(project_id, user.id).await;
    
    Sse::new(stream.map(|event| {
        Event::default()
            .event(&event.event_type)
            .data(&serde_json::to_string(&event).unwrap())
    }))
}
```

### Event Broadcasting
```rust
// When task is updated, broadcast to all users
pub async fn broadcast_task_update(
    app_state: &AppState,
    task: &Task,
    user: &User,
) {
    let event = TaskUpdateEvent {
        task: task.clone(),
        updated_by: user.clone().into_public(),
        timestamp: Utc::now(),
    };
    
    app_state.broadcast_to_project(task.project_id, event).await;
}
```

---

## Migration Strategy

### Phase 1: Database Setup
1. Create new user tables
2. Add user columns to existing tables  
3. Create first admin user from existing GitHub config
4. Migrate existing projects/tasks to admin user

### Phase 2: Authentication
1. Replace config-based auth with JWT sessions
2. Update GitHub OAuth to create user accounts
3. Add auth middleware to protected routes
4. Migrate user preferences from config.json

### Phase 3: Collaboration Features
1. Add user assignment to tasks
2. Implement real-time updates with user context
3. Add user avatars and activity tracking
4. Enable per-user git commits

### Data Migration Script
```sql
-- Migrate existing data to first user (admin)
INSERT INTO users (id, github_id, username, email, display_name, is_admin)
SELECT 
    randomblob(16), -- Generate UUID
    0, -- Placeholder GitHub ID
    'admin',
    'admin@example.com',
    'Administrator',
    TRUE;

-- Assign existing projects to admin
UPDATE projects SET created_by = (SELECT id FROM users WHERE username = 'admin');

-- Assign existing tasks to admin  
UPDATE tasks SET created_by = (SELECT id FROM users WHERE username = 'admin');
```

---

---

## Whitelist Management

### GitHub Account Whitelist
```rust
// Whitelist management model
impl GitHubWhitelist {
    pub async fn add_user(
        pool: &SqlitePool,
        github_username: &str,
        invited_by: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            "INSERT INTO github_whitelist (id, github_username, invited_by, notes) 
             VALUES ($1, $2, $3, $4) RETURNING *",
            Uuid::new_v4(),
            github_username,
            invited_by,
            notes
        )
        .fetch_one(pool)
        .await
    }
    
    pub async fn is_allowed(
        pool: &SqlitePool,
        github_username: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            "SELECT COUNT(*) as count FROM github_whitelist 
             WHERE github_username = $1 AND is_active = TRUE",
            github_username
        )
        .fetch_one(pool)
        .await?;
        
        Ok(result.count > 0)
    }
    
    pub async fn revoke_access(
        pool: &SqlitePool,
        github_username: &str,
    ) -> Result<(), sqlx::Error> {
        // Deactivate whitelist entry
        sqlx::query!(
            "UPDATE github_whitelist SET is_active = FALSE WHERE github_username = $1",
            github_username
        )
        .execute(pool)
        .await?;
        
        // Deactivate user account
        sqlx::query!(
            "UPDATE users SET is_whitelisted = FALSE WHERE username = $1",
            github_username
        )
        .execute(pool)
        .await?;
        
        // Expire all sessions for this user
        sqlx::query!(
            "UPDATE user_sessions SET expires_at = datetime('now') 
             WHERE user_id IN (SELECT id FROM users WHERE username = $1)",
            github_username
        )
        .execute(pool)
        .await?;
        
        Ok(())
    }
}
```

### Whitelist Administration
```rust
// Admin routes for whitelist management
pub fn admin_router() -> Router<AppState> {
    Router::new()
        .route("/admin/whitelist", get(list_whitelist).post(add_to_whitelist))
        .route("/admin/whitelist/:username", delete(revoke_user_access))
        .route("/admin/users", get(list_users))
        .layer(admin_required_middleware)
}

pub async fn add_to_whitelist(
    State(app_state): State<AppState>,
    Extension(admin_user): Extension<User>,
    Json(request): Json<AddToWhitelistRequest>,
) -> ResponseJson<ApiResponse<GitHubWhitelist>> {
    let whitelist_entry = GitHubWhitelist::add_user(
        &app_state.db,
        &request.github_username,
        Some(admin_user.id),
        request.notes.as_deref(),
    ).await?;
    
    ResponseJson(ApiResponse::success(whitelist_entry))
}
```

---

## Implementation Checklist

### Backend Changes
- [ ] Create user tables migration with whitelist support
- [ ] Implement User, UserSession, and GitHubWhitelist models
- [ ] Add MCP OAuth 2.1 endpoints (/.well-known/oauth-authorization-server, etc.)
- [ ] Update GitHub OAuth flow for whitelist validation
- [ ] Add dual authentication middleware (web + MCP)
- [ ] Update MCP TaskServer with user context injection
- [ ] Update all data models with user attribution
- [ ] Migrate configuration from file to database
- [ ] Add user-aware git operations with individual GitHub tokens
- [ ] Update SSE streams with user context and real-time collaboration
- [ ] Add admin routes for whitelist management

### MCP SSE OAuth Integration  
- [ ] Enable `auth` feature in rmcp dependency: `features = ["auth", "transport-sse-server"]`
- [ ] Add OAuth middleware to SSE server for token validation
- [ ] Implement GitHub OAuth endpoints for MCP authorization flow
- [ ] Add user context injection to all MCP tool calls
- [ ] Update MCP TaskServer with OAuth store integration
- [ ] Test automatic browser opening and SSE authentication flow

### Frontend Changes
- [ ] Add login/logout UI with GitHub OAuth
- [ ] Show current user in header with avatar
- [ ] Display task assignments and creators throughout UI
- [ ] Add user avatars and activity indicators
- [ ] Update settings page for per-user preferences
- [ ] Add user selection dropdown for task assignment
- [ ] Add admin panel for whitelist management
- [ ] Show real-time presence indicators for active users

### DevOps & Security
- [ ] Environment variables for JWT secrets and OAuth settings
- [ ] GitHub OAuth app configuration with proper callback URLs
- [ ] Database backup strategy for user data
- [ ] Rate limiting per user for both web and MCP
- [ ] Audit logging for admin actions and whitelist changes
- [ ] Session management and token rotation

---

## Security Considerations

### Data Protection
- Encrypt GitHub tokens in database
- Use secure JWT signing keys  
- Implement proper CORS for multiuser access
- Add rate limiting per user

### Access Control
- All users can see all projects (team server model)
- Only task creators/assignees can modify tasks
- Admin users can manage all data
- Audit log for sensitive operations

### Privacy
- User emails only visible to admins
- GitHub tokens never exposed in API responses
- Optional user profile visibility settings

---

## Deployment Model

### Server = Team
Each automagik-forge server represents one team:
- Deploy separate instances for different teams
- No cross-server communication needed
- Simple scaling model
- Clear data boundaries

### Configuration
```env
# JWT settings for web sessions
JWT_SECRET=your-secret-key
JWT_EXPIRES_IN=7d

# OAuth settings for MCP
OAUTH_CLIENT_SECRET=your-oauth-secret
OAUTH_ACCESS_TOKEN_EXPIRES_IN=1h
OAUTH_REFRESH_TOKEN_EXPIRES_IN=30d

# GitHub OAuth (per deployment)
GITHUB_CLIENT_ID=your-client-id
GITHUB_CLIENT_SECRET=your-client-secret

# Server settings
SERVER_BASE_URL=https://your-server.com
MCP_SSE_PORT=8889

# Admin settings
ADMIN_REGISTRATION_ENABLED=true
INITIAL_ADMIN_GITHUB_USERNAME=your-github-username

# Whitelist settings (optional pre-populated list)
GITHUB_WHITELIST=user1,user2,user3
```

---

## MCP Client Configuration

### For External MCP Clients
Users connecting to your MCP server just need simple SSE configuration - rmcp handles OAuth automatically:

```json
{
  "automagik-forge": {
    "type": "sse",
    "url": "http://192.168.112.154:8889/sse"
  }
}
```

That's it! When the client first connects:
1. Server responds with 401 Unauthorized + WWW-Authenticate header
2. rmcp automatically detects OAuth requirement 
3. rmcp opens user's browser to GitHub OAuth
4. User authenticates and gets redirected back
5. rmcp stores token and reconnects with authentication
6. All subsequent requests are automatically authenticated

### External User Workflow
1. **Simple Setup**: User adds SSE URL to MCP client config
2. **Auto OAuth**: Browser opens automatically for GitHub authentication
3. **Whitelist Check**: Server validates GitHub account is whitelisted
4. **Seamless Access**: User can immediately use MCP tools with their GitHub identity
5. **Commit Attribution**: All git operations automatically use user's real GitHub account

---

## Benefits of This Approach

### Security & Compliance
- **Controlled Access**: Only whitelisted GitHub accounts can connect
- **Individual Attribution**: Every action tied to real GitHub identity  
- **Audit Trail**: Complete history of who did what, when
- **Token Management**: Encrypted GitHub tokens, session management
- **Revocable Access**: Instant user deactivation capabilities

### Developer Experience
- **No Extra Setup**: Uses existing GitHub accounts for authentication
- **Git Integration**: Commits automatically use correct identity
- **MCP Compatibility**: Standard OAuth 2.1 works with any MCP client
- **Real-time Collaboration**: Live kanban updates across team members
- **Simple Administration**: Whitelist management through admin UI

### Operational Simplicity
- **One Server = One Team**: Clear deployment model
- **No Complex Permissions**: Shared projects with individual attribution
- **Standard OAuth**: No custom authentication schemes
- **Familiar Tools**: GitHub OAuth flow everyone knows

This enhanced approach transforms automagik-forge into a secure, collaborative platform where GitHub authentication provides both access control and commit attribution, while MCP OAuth ensures external clients authenticate properly through the same GitHub identity system.