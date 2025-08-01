use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{
    task::Task,
    task_attempt::TaskAttempt,
    project::Project,
    user::User,
};

/// Public user information for collaboration events (without sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct PublicUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<User> for PublicUser {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
        }
    }
}

impl From<&User> for PublicUser {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            avatar_url: user.avatar_url.clone(),
        }
    }
}

/// User presence status
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub enum PresenceStatus {
    Online,
    Away,
    Offline,
}

/// User presence information
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct UserPresence {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub last_seen: DateTime<Utc>,
    pub status: PresenceStatus,
    pub current_project: Option<Uuid>,
}

/// Base collaboration event structure
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CollaborationEvent {
    pub event_type: String,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub user_info: PublicUser,
    #[ts(type = "any")]
    pub data: serde_json::Value,
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
    pub event_id: Uuid,
}

impl CollaborationEvent {
    pub fn new(
        event_type: String,
        project_id: Uuid,
        user: &User,
        data: serde_json::Value,
    ) -> Self {
        Self {
            event_type,
            project_id,
            user_id: user.id,
            user_info: user.into(),
            data,
            timestamp: Utc::now(),
            event_id: Uuid::new_v4(),
        }
    }
}

/// Task creation event
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct TaskCreatedEvent {
    pub task: Task,
    pub created_by: PublicUser,
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
}

/// Task update event
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct TaskUpdatedEvent {
    pub task: Task,
    pub updated_by: PublicUser,
    pub changes: Vec<String>, // List of changed fields
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
}

/// Task assignment event
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct TaskAssignedEvent {
    pub task: Task,
    pub assigned_by: PublicUser,
    pub assigned_to: Option<PublicUser>,
    pub previous_assignee: Option<PublicUser>,
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
}

/// Task attempt event
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct TaskAttemptEvent {
    pub task_attempt: TaskAttempt,
    pub task: Task,
    pub created_by: PublicUser,
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
}

/// User presence event
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct UserPresenceEvent {
    pub user_presence: UserPresence,
    pub event_type: String, // "join", "leave", "update"
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
}

/// Project update event
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct ProjectUpdateEvent {
    pub project: Project,
    pub updated_by: PublicUser,
    pub changes: Vec<String>, // List of changed fields
    #[ts(type = "Date")]
    #[schema(value_type = String, format = DateTime)]
    pub timestamp: DateTime<Utc>,
}

/// Connection information for SSE management
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConnectionInfo {
    #[allow(dead_code)]
    pub user_id: Uuid,
    #[allow(dead_code)]
    pub project_id: Uuid,
    #[allow(dead_code)]
    pub connection_id: Uuid,
    #[allow(dead_code)]
    pub connected_at: DateTime<Utc>,
    #[allow(dead_code)]
    pub last_seen: DateTime<Utc>,
}

/// Event subscription for filtering
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EventSubscription {
    #[allow(dead_code)]
    pub user_id: Uuid,
    #[allow(dead_code)]
    pub project_id: Option<Uuid>, // None means all projects
    #[allow(dead_code)]
    pub event_types: Option<Vec<String>>, // None means all event types
}