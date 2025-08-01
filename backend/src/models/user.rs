use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct User {
    pub id: Uuid,
    pub github_id: i64,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub github_token: Option<String>, // Encrypted OAuth token
    pub is_admin: bool,
    pub is_whitelisted: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
#[allow(dead_code)]
pub struct CreateUser {
    pub github_id: i64,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub github_token: Option<String>,
    pub is_admin: Option<bool>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
#[allow(dead_code)]
pub struct UpdateUser {
    pub username: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub github_token: Option<String>,
    pub is_admin: Option<bool>,
    pub is_whitelisted: Option<bool>,
}

impl User {
    /// Find all users
    #[allow(dead_code)]
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT 
                id as "id!: Uuid", 
                github_id as "github_id!: i64", 
                username, 
                email, 
                display_name, 
                avatar_url, 
                github_token, 
                is_admin as "is_admin!: bool", 
                is_whitelisted as "is_whitelisted!: bool", 
                last_login_at as "last_login_at: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>" 
            FROM users 
            ORDER BY created_at DESC"#
        )
        .fetch_all(pool)
        .await
    }

    /// Find user by ID
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT 
                id as "id!: Uuid", 
                github_id as "github_id!: i64", 
                username, 
                email, 
                display_name, 
                avatar_url, 
                github_token, 
                is_admin as "is_admin!: bool", 
                is_whitelisted as "is_whitelisted!: bool", 
                last_login_at as "last_login_at: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>" 
            FROM users 
            WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Find user by GitHub ID
    pub async fn find_by_github_id(pool: &SqlitePool, github_id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT 
                id as "id!: Uuid", 
                github_id as "github_id!: i64", 
                username, 
                email, 
                display_name, 
                avatar_url, 
                github_token, 
                is_admin as "is_admin!: bool", 
                is_whitelisted as "is_whitelisted!: bool", 
                last_login_at as "last_login_at: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>" 
            FROM users 
            WHERE github_id = $1"#,
            github_id
        )
        .fetch_optional(pool)
        .await
    }

    /// Find user by username
    #[allow(dead_code)]
    pub async fn find_by_username(pool: &SqlitePool, username: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT 
                id as "id!: Uuid", 
                github_id as "github_id!: i64", 
                username, 
                email, 
                display_name, 
                avatar_url, 
                github_token, 
                is_admin as "is_admin!: bool", 
                is_whitelisted as "is_whitelisted!: bool", 
                last_login_at as "last_login_at: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>" 
            FROM users 
            WHERE username = $1"#,
            username
        )
        .fetch_optional(pool)
        .await
    }

    /// Create a new user
    pub async fn create(
        pool: &SqlitePool,
        data: &CreateUser,
        user_id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        let is_admin = data.is_admin.unwrap_or(false);
        sqlx::query_as!(
            User,
            r#"INSERT INTO users (id, github_id, username, email, display_name, avatar_url, github_token, is_admin, is_whitelisted) 
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE) 
               RETURNING 
                id as "id!: Uuid", 
                github_id as "github_id!: i64", 
                username, 
                email, 
                display_name, 
                avatar_url, 
                github_token, 
                is_admin as "is_admin!: bool", 
                is_whitelisted as "is_whitelisted!: bool", 
                last_login_at as "last_login_at: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>""#,
            user_id,
            data.github_id,
            data.username,
            data.email,
            data.display_name,
            data.avatar_url,
            data.github_token,
            is_admin
        )
        .fetch_one(pool)
        .await
    }

    /// Update user information
    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateUser,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"UPDATE users 
               SET 
                username = COALESCE($2, username),
                email = COALESCE($3, email),
                display_name = COALESCE($4, display_name),
                avatar_url = COALESCE($5, avatar_url),
                github_token = COALESCE($6, github_token),
                is_admin = COALESCE($7, is_admin),
                is_whitelisted = COALESCE($8, is_whitelisted),
                updated_at = datetime('now', 'subsec')
               WHERE id = $1 
               RETURNING 
                id as "id!: Uuid", 
                github_id as "github_id!: i64", 
                username, 
                email, 
                display_name, 
                avatar_url, 
                github_token, 
                is_admin as "is_admin!: bool", 
                is_whitelisted as "is_whitelisted!: bool", 
                last_login_at as "last_login_at: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            data.username,
            data.email,
            data.display_name,
            data.avatar_url,
            data.github_token,
            data.is_admin,
            data.is_whitelisted
        )
        .fetch_one(pool)
        .await
    }

    /// Update last login timestamp
    pub async fn update_last_login(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE users SET last_login_at = datetime('now', 'subsec') WHERE id = $1",
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Delete user
    #[allow(dead_code)]
    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM users WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Check if user exists
    #[allow(dead_code)]
    pub async fn exists(pool: &SqlitePool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM users WHERE id = $1"#,
            id
        )
        .fetch_one(pool)
        .await?;
        Ok(result.count > 0)
    }

    /// Check if user is whitelisted by GitHub ID
    pub async fn is_github_id_whitelisted(pool: &SqlitePool, github_id: i64) -> Result<bool, sqlx::Error> {
        // Check if user exists and is whitelisted
        let user_result = sqlx::query!(
            r#"SELECT is_whitelisted as "is_whitelisted!: bool" FROM users WHERE github_id = $1"#,
            github_id
        )
        .fetch_optional(pool)
        .await?;

        if let Some(user) = user_result {
            return Ok(user.is_whitelisted);
        }

        // If user doesn't exist, check the whitelist table
        let whitelist_result = sqlx::query!(
            r#"SELECT is_active as "is_active!: bool" FROM github_whitelist WHERE github_id = $1"#,
            github_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(whitelist_result.map_or(false, |w| w.is_active))
    }

    /// Check if username is whitelisted
    #[allow(dead_code)]
    pub async fn is_github_username_whitelisted(pool: &SqlitePool, username: &str) -> Result<bool, sqlx::Error> {
        // Check if user exists and is whitelisted
        let user_result = sqlx::query!(
            r#"SELECT is_whitelisted as "is_whitelisted!: bool" FROM users WHERE username = $1"#,
            username
        )
        .fetch_optional(pool)
        .await?;

        if let Some(user) = user_result {
            return Ok(user.is_whitelisted);
        }

        // If user doesn't exist, check the whitelist table
        let whitelist_result = sqlx::query!(
            r#"SELECT is_active as "is_active!: bool" FROM github_whitelist WHERE github_username = $1"#,
            username
        )
        .fetch_optional(pool)
        .await?;

        Ok(whitelist_result.map_or(false, |w| w.is_active))
    }

    /// Get all admin users
    #[allow(dead_code)]
    pub async fn find_admins(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT 
                id as "id!: Uuid", 
                github_id as "github_id!: i64", 
                username, 
                email, 
                display_name, 
                avatar_url, 
                github_token, 
                is_admin as "is_admin!: bool", 
                is_whitelisted as "is_whitelisted!: bool", 
                last_login_at as "last_login_at: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>" 
            FROM users 
            WHERE is_admin = TRUE 
            ORDER BY created_at ASC"#
        )
        .fetch_all(pool)
        .await
    }

    /// Get the first admin user (for fallback admin assignment)
    #[allow(dead_code)]
    pub async fn get_first_admin(pool: &SqlitePool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT 
                id as "id!: Uuid", 
                github_id as "github_id!: i64", 
                username, 
                email, 
                display_name, 
                avatar_url, 
                github_token, 
                is_admin as "is_admin!: bool", 
                is_whitelisted as "is_whitelisted!: bool", 
                last_login_at as "last_login_at: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>" 
            FROM users 
            WHERE is_admin = TRUE 
            ORDER BY created_at ASC 
            LIMIT 1"#
        )
        .fetch_optional(pool)
        .await
    }
}