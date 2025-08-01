PRAGMA foreign_keys = ON;

-- Core user table with whitelist support and GitHub integration
CREATE TABLE users (
    id BLOB PRIMARY KEY,
    github_id INTEGER UNIQUE NOT NULL,
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    display_name TEXT,
    avatar_url TEXT,
    github_token TEXT, -- Encrypted OAuth token for commits
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    is_whitelisted BOOLEAN NOT NULL DEFAULT TRUE,
    last_login_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- Index for GitHub ID lookups
CREATE INDEX idx_users_github_id ON users(github_id);

-- Index for username lookups  
CREATE INDEX idx_users_username ON users(username);

-- User sessions for web and MCP authentication
CREATE TABLE user_sessions (
    id BLOB PRIMARY KEY,
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT UNIQUE NOT NULL,
    session_type TEXT NOT NULL DEFAULT 'web' CHECK (session_type IN ('web', 'mcp')),
    client_info TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- Index for session lookups by token hash
CREATE INDEX idx_user_sessions_token_hash ON user_sessions(token_hash);

-- Index for session cleanup by expiration
CREATE INDEX idx_user_sessions_expires_at ON user_sessions(expires_at);

-- Index for user sessions lookup
CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id);

-- GitHub account whitelist for access control
CREATE TABLE github_whitelist (
    id BLOB PRIMARY KEY,
    github_username TEXT UNIQUE NOT NULL,
    github_id INTEGER UNIQUE,
    invited_by BLOB REFERENCES users(id),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- Index for GitHub username lookups
CREATE INDEX idx_github_whitelist_username ON github_whitelist(github_username);

-- Index for GitHub ID lookups
CREATE INDEX idx_github_whitelist_github_id ON github_whitelist(github_id);

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
    disclaimer_acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    onboarding_acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    github_login_acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    telemetry_acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    analytics_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    default_pr_base TEXT NOT NULL DEFAULT 'main',
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE(user_id)
);

-- Index for user preferences lookup
CREATE INDEX idx_user_preferences_user_id ON user_preferences(user_id);