use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

// Re-export types from config for backward compatibility
pub use crate::models::config::{EditorType, SoundConstants, SoundFile, ThemeMode};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct UserPreferences {
    pub id: Uuid,
    pub user_id: Uuid,
    pub theme: ThemeMode,
    pub sound_alerts: bool,
    pub sound_file: SoundFile,
    pub push_notifications: bool,
    pub editor_type: EditorType,
    pub editor_custom_command: Option<String>,
    pub disclaimer_acknowledged: bool,
    pub onboarding_acknowledged: bool,
    pub github_login_acknowledged: bool,
    pub telemetry_acknowledged: bool,
    pub analytics_enabled: bool,
    pub default_pr_base: String,
    
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateUserPreferences {
    pub user_id: Uuid,
    pub theme: Option<ThemeMode>,
    pub sound_alerts: Option<bool>,
    pub sound_file: Option<SoundFile>,
    pub push_notifications: Option<bool>,
    pub editor_type: Option<EditorType>,
    pub editor_custom_command: Option<String>,
    pub disclaimer_acknowledged: Option<bool>,
    pub onboarding_acknowledged: Option<bool>,
    pub github_login_acknowledged: Option<bool>,
    pub telemetry_acknowledged: Option<bool>,
    pub analytics_enabled: Option<bool>,
    pub default_pr_base: Option<String>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct UpdateUserPreferences {
    pub theme: Option<ThemeMode>,
    pub sound_alerts: Option<bool>,
    pub sound_file: Option<SoundFile>,
    pub push_notifications: Option<bool>,
    pub editor_type: Option<EditorType>,
    pub editor_custom_command: Option<String>,
    pub disclaimer_acknowledged: Option<bool>,
    pub onboarding_acknowledged: Option<bool>,
    pub github_login_acknowledged: Option<bool>,
    pub telemetry_acknowledged: Option<bool>,
    pub analytics_enabled: Option<bool>,
    pub default_pr_base: Option<String>,
}

impl Default for CreateUserPreferences {
    fn default() -> Self {
        Self {
            user_id: Uuid::new_v4(), // This will be overridden
            theme: Some(ThemeMode::System),
            sound_alerts: Some(true),
            sound_file: Some(SoundFile::AbstractSound4),
            push_notifications: Some(true),
            editor_type: Some(EditorType::VSCode),
            editor_custom_command: None,
            disclaimer_acknowledged: Some(false),
            onboarding_acknowledged: Some(false),
            github_login_acknowledged: Some(false),
            telemetry_acknowledged: Some(false),
            analytics_enabled: Some(true),
            default_pr_base: Some("main".to_string()),
        }
    }
}

impl UserPreferences {
    /// Find preferences by user ID
    pub async fn find_by_user_id(pool: &SqlitePool, user_id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            UserPreferences,
            r#"SELECT 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                theme as "theme!: ThemeMode", 
                sound_alerts as "sound_alerts!: bool", 
                sound_file as "sound_file!: SoundFile", 
                push_notifications as "push_notifications!: bool", 
                editor_type as "editor_type!: EditorType", 
                editor_custom_command, 
                disclaimer_acknowledged as "disclaimer_acknowledged!: bool", 
                onboarding_acknowledged as "onboarding_acknowledged!: bool", 
                github_login_acknowledged as "github_login_acknowledged!: bool", 
                telemetry_acknowledged as "telemetry_acknowledged!: bool", 
                analytics_enabled as "analytics_enabled!: bool", 
                default_pr_base, 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>" 
            FROM user_preferences 
            WHERE user_id = $1"#,
            user_id
        )
        .fetch_optional(pool)
        .await
    }

    /// Get or create preferences for user (with defaults)
    pub async fn get_or_create_for_user(pool: &SqlitePool, user_id: Uuid) -> Result<Self, sqlx::Error> {
        if let Some(preferences) = Self::find_by_user_id(pool, user_id).await? {
            Ok(preferences)
        } else {
            // Create default preferences for user
            let mut defaults = CreateUserPreferences::default();
            defaults.user_id = user_id;
            Self::create(pool, &defaults, Uuid::new_v4()).await
        }
    }

    /// Create new user preferences
    pub async fn create(
        pool: &SqlitePool,
        data: &CreateUserPreferences,
        preferences_id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        let theme = data.theme.clone().unwrap_or(ThemeMode::System);
        let sound_alerts = data.sound_alerts.unwrap_or(true);
        let sound_file = data.sound_file.clone().unwrap_or(SoundFile::AbstractSound4);
        let push_notifications = data.push_notifications.unwrap_or(true);
        let editor_type = data.editor_type.clone().unwrap_or(EditorType::VSCode);
        let disclaimer_acknowledged = data.disclaimer_acknowledged.unwrap_or(false);
        let onboarding_acknowledged = data.onboarding_acknowledged.unwrap_or(false);
        let github_login_acknowledged = data.github_login_acknowledged.unwrap_or(false);
        let telemetry_acknowledged = data.telemetry_acknowledged.unwrap_or(false);
        let analytics_enabled = data.analytics_enabled.unwrap_or(true);
        let default_pr_base = data.default_pr_base.clone().unwrap_or_else(|| "main".to_string());

        sqlx::query_as!(
            UserPreferences,
            r#"INSERT INTO user_preferences (
                id, user_id, theme, sound_alerts, sound_file, push_notifications, 
                editor_type, editor_custom_command, disclaimer_acknowledged, 
                onboarding_acknowledged, github_login_acknowledged, telemetry_acknowledged, 
                analytics_enabled, default_pr_base
            ) 
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) 
            RETURNING 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                theme as "theme!: ThemeMode", 
                sound_alerts as "sound_alerts!: bool", 
                sound_file as "sound_file!: SoundFile", 
                push_notifications as "push_notifications!: bool", 
                editor_type as "editor_type!: EditorType", 
                editor_custom_command, 
                disclaimer_acknowledged as "disclaimer_acknowledged!: bool", 
                onboarding_acknowledged as "onboarding_acknowledged!: bool", 
                github_login_acknowledged as "github_login_acknowledged!: bool", 
                telemetry_acknowledged as "telemetry_acknowledged!: bool", 
                analytics_enabled as "analytics_enabled!: bool", 
                default_pr_base, 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>""#,
            preferences_id,
            data.user_id,
            theme as ThemeMode,
            sound_alerts,
            sound_file as SoundFile,
            push_notifications,
            editor_type as EditorType,
            data.editor_custom_command,
            disclaimer_acknowledged,
            onboarding_acknowledged,
            github_login_acknowledged,
            telemetry_acknowledged,
            analytics_enabled,
            default_pr_base
        )
        .fetch_one(pool)
        .await
    }

    /// Update user preferences
    pub async fn update(
        pool: &SqlitePool,
        user_id: Uuid,
        data: &UpdateUserPreferences,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            UserPreferences,
            r#"UPDATE user_preferences 
               SET 
                theme = COALESCE($2, theme),
                sound_alerts = COALESCE($3, sound_alerts),
                sound_file = COALESCE($4, sound_file),
                push_notifications = COALESCE($5, push_notifications),
                editor_type = COALESCE($6, editor_type),
                editor_custom_command = COALESCE($7, editor_custom_command),
                disclaimer_acknowledged = COALESCE($8, disclaimer_acknowledged),
                onboarding_acknowledged = COALESCE($9, onboarding_acknowledged),
                github_login_acknowledged = COALESCE($10, github_login_acknowledged),
                telemetry_acknowledged = COALESCE($11, telemetry_acknowledged),
                analytics_enabled = COALESCE($12, analytics_enabled),
                default_pr_base = COALESCE($13, default_pr_base),
                updated_at = datetime('now', 'subsec')
               WHERE user_id = $1 
               RETURNING 
                id as "id!: Uuid", 
                user_id as "user_id!: Uuid", 
                theme as "theme!: ThemeMode", 
                sound_alerts as "sound_alerts!: bool", 
                sound_file as "sound_file!: SoundFile", 
                push_notifications as "push_notifications!: bool", 
                editor_type as "editor_type!: EditorType", 
                editor_custom_command, 
                disclaimer_acknowledged as "disclaimer_acknowledged!: bool", 
                onboarding_acknowledged as "onboarding_acknowledged!: bool", 
                github_login_acknowledged as "github_login_acknowledged!: bool", 
                telemetry_acknowledged as "telemetry_acknowledged!: bool", 
                analytics_enabled as "analytics_enabled!: bool", 
                default_pr_base, 
                created_at as "created_at!: DateTime<Utc>", 
                updated_at as "updated_at!: DateTime<Utc>""#,
            user_id,
            data.theme.as_ref().map(|t| t.clone() as ThemeMode),
            data.sound_alerts,
            data.sound_file.as_ref().map(|s| s.clone() as SoundFile),
            data.push_notifications,
            data.editor_type.as_ref().map(|e| e.clone() as EditorType),
            data.editor_custom_command,
            data.disclaimer_acknowledged,
            data.onboarding_acknowledged,
            data.github_login_acknowledged,
            data.telemetry_acknowledged,
            data.analytics_enabled,
            data.default_pr_base
        )
        .fetch_one(pool)
        .await
    }

    /// Delete user preferences
    pub async fn delete(pool: &SqlitePool, user_id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM user_preferences WHERE user_id = $1", user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Update specific acknowledgment flags
    pub async fn acknowledge_disclaimer(pool: &SqlitePool, user_id: Uuid) -> Result<Self, sqlx::Error> {
        let data = UpdateUserPreferences {
            disclaimer_acknowledged: Some(true),
            ..Default::default()
        };
        Self::update(pool, user_id, &data).await
    }

    pub async fn acknowledge_onboarding(pool: &SqlitePool, user_id: Uuid) -> Result<Self, sqlx::Error> {
        let data = UpdateUserPreferences {
            onboarding_acknowledged: Some(true),
            ..Default::default()
        };
        Self::update(pool, user_id, &data).await
    }

    pub async fn acknowledge_github_login(pool: &SqlitePool, user_id: Uuid) -> Result<Self, sqlx::Error> {
        let data = UpdateUserPreferences {
            github_login_acknowledged: Some(true),
            ..Default::default()
        };
        Self::update(pool, user_id, &data).await
    }

    pub async fn acknowledge_telemetry(pool: &SqlitePool, user_id: Uuid) -> Result<Self, sqlx::Error> {
        let data = UpdateUserPreferences {
            telemetry_acknowledged: Some(true),
            ..Default::default()
        };
        Self::update(pool, user_id, &data).await
    }

    /// Convert to legacy Config format for backward compatibility
    pub fn to_legacy_config(&self, github_pat: Option<String>, github_token: Option<String>, github_username: Option<String>, primary_email: Option<String>) -> crate::models::config::Config {
        use crate::models::config::{Config, EditorConfig, GitHubConfig};
        use crate::executor::ExecutorConfig;

        Config {
            theme: self.theme.clone(),
            executor: ExecutorConfig::Claude, // Default, will be handled by executor config separately
            disclaimer_acknowledged: self.disclaimer_acknowledged,
            onboarding_acknowledged: self.onboarding_acknowledged,
            github_login_acknowledged: self.github_login_acknowledged,
            telemetry_acknowledged: self.telemetry_acknowledged,
            sound_alerts: self.sound_alerts,
            sound_file: self.sound_file.clone(),
            push_notifications: self.push_notifications,
            editor: EditorConfig {
                editor_type: self.editor_type.clone(),
                custom_command: self.editor_custom_command.clone(),
            },
            github: GitHubConfig {
                pat: github_pat,
                token: github_token,
                username: github_username,
                primary_email,
                default_pr_base: Some(self.default_pr_base.clone()),
            },
            analytics_enabled: Some(self.analytics_enabled),
        }
    }

    /// Create from legacy Config format
    pub fn from_legacy_config(user_id: Uuid, config: &crate::models::config::Config) -> CreateUserPreferences {
        CreateUserPreferences {
            user_id,
            theme: Some(config.theme.clone()),
            sound_alerts: Some(config.sound_alerts),
            sound_file: Some(config.sound_file.clone()),
            push_notifications: Some(config.push_notifications),
            editor_type: Some(config.editor.editor_type.clone()),
            editor_custom_command: config.editor.custom_command.clone(),
            disclaimer_acknowledged: Some(config.disclaimer_acknowledged),
            onboarding_acknowledged: Some(config.onboarding_acknowledged),
            github_login_acknowledged: Some(config.github_login_acknowledged),
            telemetry_acknowledged: Some(config.telemetry_acknowledged),
            analytics_enabled: Some(config.analytics_enabled.unwrap_or(true)),
            default_pr_base: config.github.default_pr_base.clone(),
        }
    }

    /// Get editor command based on preferences
    pub fn get_editor_command(&self) -> Vec<String> {
        match &self.editor_type {
            EditorType::VSCode => vec!["code".to_string()],
            EditorType::Cursor => vec!["cursor".to_string()],
            EditorType::Windsurf => vec!["windsurf".to_string()],
            EditorType::IntelliJ => vec!["idea".to_string()],
            EditorType::Zed => vec!["zed".to_string()],
            EditorType::Custom => {
                if let Some(custom) = &self.editor_custom_command {
                    custom.split_whitespace().map(|s| s.to_string()).collect()
                } else {
                    vec!["code".to_string()] // fallback to VSCode
                }
            }
        }
    }
}

impl Default for UpdateUserPreferences {
    fn default() -> Self {
        Self {
            theme: None,
            sound_alerts: None,
            sound_file: None,
            push_notifications: None,
            editor_type: None,
            editor_custom_command: None,
            disclaimer_acknowledged: None,
            onboarding_acknowledged: None,
            github_login_acknowledged: None,
            telemetry_acknowledged: None,
            analytics_enabled: None,
            default_pr_base: None,
        }
    }
}