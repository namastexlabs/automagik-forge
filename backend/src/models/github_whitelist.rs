use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct GitHubWhitelist {
    pub id: Uuid,
    pub github_username: String,
    pub github_id: Option<i64>,
    pub invited_by: Option<Uuid>, // User ID who added this entry
    pub is_active: bool,
    pub notes: Option<String>,
    
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateGitHubWhitelist {
    pub github_username: String,
    pub github_id: Option<i64>,
    pub invited_by: Option<Uuid>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
#[allow(dead_code)]
pub struct UpdateGitHubWhitelist {
    pub github_username: Option<String>,
    pub github_id: Option<i64>,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
}

impl GitHubWhitelist {
    /// Find all whitelist entries
    #[allow(dead_code)]
    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            r#"SELECT 
                id as "id!: Uuid", 
                github_username, 
                github_id as "github_id: i64", 
                invited_by as "invited_by: Uuid", 
                is_active as "is_active!: bool", 
                notes, 
                created_at as "created_at!: DateTime<Utc>" 
            FROM github_whitelist 
            ORDER BY created_at DESC"#
        )
        .fetch_all(pool)
        .await
    }

    /// Find all active whitelist entries
    #[allow(dead_code)]
    pub async fn find_all_active(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            r#"SELECT 
                id as "id!: Uuid", 
                github_username, 
                github_id as "github_id: i64", 
                invited_by as "invited_by: Uuid", 
                is_active as "is_active!: bool", 
                notes, 
                created_at as "created_at!: DateTime<Utc>" 
            FROM github_whitelist 
            WHERE is_active = TRUE 
            ORDER BY created_at DESC"#
        )
        .fetch_all(pool)
        .await
    }

    /// Find entry by ID
    #[allow(dead_code)]
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            r#"SELECT 
                id as "id!: Uuid", 
                github_username, 
                github_id as "github_id: i64", 
                invited_by as "invited_by: Uuid", 
                is_active as "is_active!: bool", 
                notes, 
                created_at as "created_at!: DateTime<Utc>" 
            FROM github_whitelist 
            WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Find entry by GitHub username
    #[allow(dead_code)]
    pub async fn find_by_github_username(pool: &SqlitePool, github_username: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            r#"SELECT 
                id as "id!: Uuid", 
                github_username, 
                github_id as "github_id: i64", 
                invited_by as "invited_by: Uuid", 
                is_active as "is_active!: bool", 
                notes, 
                created_at as "created_at!: DateTime<Utc>" 
            FROM github_whitelist 
            WHERE github_username = $1"#,
            github_username
        )
        .fetch_optional(pool)
        .await
    }

    /// Find entry by GitHub ID
    #[allow(dead_code)]
    pub async fn find_by_github_id(pool: &SqlitePool, github_id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            r#"SELECT 
                id as "id!: Uuid", 
                github_username, 
                github_id as "github_id: i64", 
                invited_by as "invited_by: Uuid", 
                is_active as "is_active!: bool", 
                notes, 
                created_at as "created_at!: DateTime<Utc>" 
            FROM github_whitelist 
            WHERE github_id = $1"#,
            github_id
        )
        .fetch_optional(pool)
        .await
    }

    /// Check if GitHub username is whitelisted
    #[allow(dead_code)]
    pub async fn is_username_whitelisted(pool: &SqlitePool, github_username: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT is_active as "is_active!: bool" 
               FROM github_whitelist 
               WHERE github_username = $1 AND is_active = TRUE"#,
            github_username
        )
        .fetch_optional(pool)
        .await?;
        
        Ok(result.is_some())
    }

    /// Check if GitHub ID is whitelisted
    #[allow(dead_code)]
    pub async fn is_github_id_whitelisted(pool: &SqlitePool, github_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT is_active as "is_active!: bool" 
               FROM github_whitelist 
               WHERE github_id = $1 AND is_active = TRUE"#,
            github_id
        )
        .fetch_optional(pool)
        .await?;
        
        Ok(result.is_some())
    }

    /// Create a new whitelist entry
    pub async fn create(
        pool: &SqlitePool,
        data: &CreateGitHubWhitelist,
        whitelist_id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            r#"INSERT INTO github_whitelist (id, github_username, github_id, invited_by, is_active, notes) 
               VALUES ($1, $2, $3, $4, TRUE, $5) 
               RETURNING 
                id as "id!: Uuid", 
                github_username, 
                github_id as "github_id: i64", 
                invited_by as "invited_by: Uuid", 
                is_active as "is_active!: bool", 
                notes, 
                created_at as "created_at!: DateTime<Utc>""#,
            whitelist_id,
            data.github_username,
            data.github_id,
            data.invited_by,
            data.notes
        )
        .fetch_one(pool)
        .await
    }

    /// Update whitelist entry
    #[allow(dead_code)]
    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateGitHubWhitelist,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            r#"UPDATE github_whitelist 
               SET 
                github_username = COALESCE($2, github_username),
                github_id = COALESCE($3, github_id),
                is_active = COALESCE($4, is_active),
                notes = COALESCE($5, notes)
               WHERE id = $1 
               RETURNING 
                id as "id!: Uuid", 
                github_username, 
                github_id as "github_id: i64", 
                invited_by as "invited_by: Uuid", 
                is_active as "is_active!: bool", 
                notes, 
                created_at as "created_at!: DateTime<Utc>""#,
            id,
            data.github_username,
            data.github_id,
            data.is_active,
            data.notes
        )
        .fetch_one(pool)
        .await
    }

    /// Activate whitelist entry
    #[allow(dead_code)]
    pub async fn activate(pool: &SqlitePool, id: Uuid) -> Result<Self, sqlx::Error> {
        let data = UpdateGitHubWhitelist {
            github_username: None,
            github_id: None,
            is_active: Some(true),
            notes: None,
        };
        Self::update(pool, id, &data).await
    }

    /// Deactivate whitelist entry
    #[allow(dead_code)]
    pub async fn deactivate(pool: &SqlitePool, id: Uuid) -> Result<Self, sqlx::Error> {
        let data = UpdateGitHubWhitelist {
            github_username: None,
            github_id: None,
            is_active: Some(false),
            notes: None,
        };
        Self::update(pool, id, &data).await
    }

    /// Delete whitelist entry
    #[allow(dead_code)]
    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM github_whitelist WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Add GitHub username to whitelist
    // Used in bin/migrate_to_multiuser.rs:101 but appears unused to analyzer
    // because it's called from a binary target rather than main library
    #[allow(dead_code)]
    pub async fn add_username(
        pool: &SqlitePool,
        github_username: &str,
        invited_by: Option<Uuid>,
        notes: Option<String>,
    ) -> Result<Self, sqlx::Error> {
        let data = CreateGitHubWhitelist {
            github_username: github_username.to_string(),
            github_id: None,
            invited_by,
            notes,
        };
        Self::create(pool, &data, Uuid::new_v4()).await
    }

    /// Add GitHub ID to whitelist
    #[allow(dead_code)]
    pub async fn add_github_id(
        pool: &SqlitePool,
        github_id: i64,
        github_username: &str,
        invited_by: Option<Uuid>,
        notes: Option<String>,
    ) -> Result<Self, sqlx::Error> {
        let data = CreateGitHubWhitelist {
            github_username: github_username.to_string(),
            github_id: Some(github_id),
            invited_by,
            notes,
        };
        Self::create(pool, &data, Uuid::new_v4()).await
    }

    /// Update GitHub ID for existing username entry
    #[allow(dead_code)]
    pub async fn update_github_id_for_username(
        pool: &SqlitePool,
        github_username: &str,
        github_id: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        // First find the entry
        if let Some(entry) = Self::find_by_github_username(pool, github_username).await? {
            let data = UpdateGitHubWhitelist {
                github_username: None,
                github_id: Some(github_id),
                is_active: None,
                notes: None,
            };
            Ok(Some(Self::update(pool, entry.id, &data).await?))
        } else {
            Ok(None)
        }
    }

    /// Get all entries invited by a specific user
    #[allow(dead_code)]
    pub async fn find_by_inviter(pool: &SqlitePool, inviter_id: Uuid) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            GitHubWhitelist,
            r#"SELECT 
                id as "id!: Uuid", 
                github_username, 
                github_id as "github_id: i64", 
                invited_by as "invited_by: Uuid", 
                is_active as "is_active!: bool", 
                notes, 
                created_at as "created_at!: DateTime<Utc>" 
            FROM github_whitelist 
            WHERE invited_by = $1 
            ORDER BY created_at DESC"#,
            inviter_id
        )
        .fetch_all(pool)
        .await
    }

    /// Count total whitelist entries
    #[allow(dead_code)]
    pub async fn count_total(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM github_whitelist"#
        )
        .fetch_one(pool)
        .await?;
        Ok(result.count)
    }

    /// Count active whitelist entries
    #[allow(dead_code)]
    pub async fn count_active(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT COUNT(*) as "count!: i64" FROM github_whitelist WHERE is_active = TRUE"#
        )
        .fetch_one(pool)
        .await?;
        Ok(result.count)
    }
}