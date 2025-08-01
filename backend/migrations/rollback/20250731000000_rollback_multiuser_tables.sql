PRAGMA foreign_keys = ON;

-- Remove user preferences table and indexes
DROP INDEX IF EXISTS idx_user_preferences_user_id;
DROP TABLE IF EXISTS user_preferences;

-- Remove GitHub whitelist table and indexes
DROP INDEX IF EXISTS idx_github_whitelist_github_id;
DROP INDEX IF EXISTS idx_github_whitelist_username;
DROP TABLE IF EXISTS github_whitelist;

-- Remove user sessions table and indexes
DROP INDEX IF EXISTS idx_user_sessions_user_id;
DROP INDEX IF EXISTS idx_user_sessions_expires_at;
DROP INDEX IF EXISTS idx_user_sessions_token_hash;
DROP TABLE IF EXISTS user_sessions;

-- Remove users table and indexes
DROP INDEX IF EXISTS idx_users_username;
DROP INDEX IF EXISTS idx_users_github_id;
DROP TABLE IF EXISTS users;