use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Type, Serialize, Deserialize, PartialEq, TS, ToSchema)]
#[sqlx(type_name = "session_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SessionType {
    Web,
    Mcp,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub session_type: SessionType,
    pub client_info: Option<String>,
    
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub expires_at: DateTime<Utc>,
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateUserSession {
    pub user_id: Uuid,
    pub token_hash: String,
    pub session_type: SessionType,
    pub client_info: Option<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct UpdateUserSession {
    pub expires_at: Option<DateTime<Utc>>,
    pub client_info: Option<String>,
}

impl UserSession {
    /// Default session duration for web sessions (24 hours)
    pub const WEB_SESSION_DURATION_HOURS: i64 = 24;
    
    /// Default session duration for MCP sessions (30 days)
    pub const MCP_SESSION_DURATION_DAYS: i64 = 30;

    /// Create a new session with default expiration based on session type
    pub async fn create_with_defaults(
        pool: &SqlitePool,
        user_id: Uuid,
        token_hash: String,
        session_type: SessionType,
        client_info: Option<String>,
    ) -> Result<Self, sqlx::Error> {
        let expires_at = match session_type {
            SessionType::Web => Utc::now() + Duration::hours(Self::WEB_SESSION_DURATION_HOURS),
            SessionType::Mcp => Utc::now() + Duration::days(Self::MCP_SESSION_DURATION_DAYS),
        };

        let data = CreateUserSession {
            user_id,
            token_hash,
            session_type,
            client_info,
            expires_at,
        };

        Self::create(pool, &data, Uuid::new_v4()).await
    }

    /// Find all sessions for a user
    pub async fn find_by_user_id(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            UserSession,
            r#"SELECT 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                token_hash, 
                session_type as "session_type!: SessionType", 
                client_info, 
                expires_at as "expires_at!: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>" 
            FROM user_sessions 
            WHERE user_id = $1 
            ORDER BY created_at DESC"#,
            user_id
        )
        .fetch_all(pool)
        .await
    }

    /// Find session by token hash
    pub async fn find_by_token_hash(pool: &SqlitePool, token_hash: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            UserSession,
            r#"SELECT 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                token_hash, 
                session_type as "session_type!: SessionType", 
                client_info, 
                expires_at as "expires_at!: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>" 
            FROM user_sessions 
            WHERE token_hash = $1"#,
            token_hash
        )
        .fetch_optional(pool)
        .await
    }

    /// Find valid (non-expired) session by token hash
    pub async fn find_valid_by_token_hash(pool: &SqlitePool, token_hash: &str) -> Result<Option<Self>, sqlx::Error> {
        let now = Utc::now();
        sqlx::query_as!(
            UserSession,
            r#"SELECT 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                token_hash, 
                session_type as "session_type!: SessionType", 
                client_info, 
                expires_at as "expires_at!: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>" 
            FROM user_sessions 
            WHERE token_hash = $1 AND expires_at > $2"#,
            token_hash,
            now
        )
        .fetch_optional(pool)
        .await
    }

    /// Create a new session
    pub async fn create(
        pool: &SqlitePool,
        data: &CreateUserSession,
        session_id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        let session_type_str = match data.session_type {
            SessionType::Web => "web",
            SessionType::Mcp => "mcp",
        };
        sqlx::query_as!(
            UserSession,
            r#"INSERT INTO user_sessions (id, user_id, token_hash, session_type, client_info, expires_at) 
               VALUES ($1, $2, $3, $4, $5, $6) 
               RETURNING 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                token_hash, 
                session_type as "session_type!: SessionType", 
                client_info, 
                expires_at as "expires_at!: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>""#,
            session_id,
            data.user_id,
            data.token_hash,
            session_type_str,
            data.client_info,
            data.expires_at
        )
        .fetch_one(pool)
        .await
    }

    /// Update session
    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateUserSession,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            UserSession,
            r#"UPDATE user_sessions 
               SET 
                expires_at = COALESCE($2, expires_at),
                client_info = COALESCE($3, client_info)
               WHERE id = $1 
               RETURNING 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                token_hash, 
                session_type as "session_type!: SessionType", 
                client_info, 
                expires_at as "expires_at!: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>""#,
            id,
            data.expires_at,
            data.client_info
        )
        .fetch_one(pool)
        .await
    }

    /// Extend session expiration
    pub async fn extend_expiration(
        pool: &SqlitePool,
        id: Uuid,
        session_type: SessionType,
    ) -> Result<Self, sqlx::Error> {
        let new_expires_at = match session_type {
            SessionType::Web => Utc::now() + Duration::hours(Self::WEB_SESSION_DURATION_HOURS),
            SessionType::Mcp => Utc::now() + Duration::days(Self::MCP_SESSION_DURATION_DAYS),
        };

        let data = UpdateUserSession {
            expires_at: Some(new_expires_at),
            client_info: None,
        };

        Self::update(pool, id, &data).await
    }

    /// Delete session
    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM user_sessions WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete session by token hash
    pub async fn delete_by_token_hash(pool: &SqlitePool, token_hash: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM user_sessions WHERE token_hash = $1", token_hash)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete all sessions for user
    pub async fn delete_all_for_user(pool: &SqlitePool, user_id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM user_sessions WHERE user_id = $1", user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let now = Utc::now();
        let result = sqlx::query!("DELETE FROM user_sessions WHERE expires_at <= $1", now)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }

    /// Check if session is valid (not expired)
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    /// Get remaining session time
    pub fn time_remaining(&self) -> Option<Duration> {
        let now = Utc::now();
        if self.expires_at > now {
            Some(self.expires_at - now)
        } else {
            None
        }
    }

    /// Count active sessions for user by type
    pub async fn count_active_by_user_and_type(
        pool: &SqlitePool,
        user_id: Uuid,
        session_type: SessionType,
    ) -> Result<i64, sqlx::Error> {
        let now = Utc::now();
        let session_type_str = match session_type {
            SessionType::Web => "web",
            SessionType::Mcp => "mcp",
        };
        let result = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" 
               FROM user_sessions 
               WHERE user_id = $1 AND session_type = $2 AND expires_at > $3"#,
            user_id,
            session_type_str,
            now
        )
        .fetch_one(pool)
        .await?;
        Ok(result.count)
    }

    /// Get all active sessions (not expired)
    pub async fn find_all_active(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        let now = Utc::now();
        sqlx::query_as!(
            UserSession,
            r#"SELECT 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                token_hash, 
                session_type as "session_type!: SessionType", 
                client_info, 
                expires_at as "expires_at!: DateTime<Utc>", 
                created_at as "created_at!: DateTime<Utc>" 
            FROM user_sessions 
            WHERE expires_at > $1 
            ORDER BY created_at DESC"#,
            now
        )
        .fetch_all(pool)
        .await
    }
}