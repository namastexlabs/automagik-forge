use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use chrono::Utc;
use futures_util::{Stream, StreamExt};
use tokio_stream::wrappers::BroadcastStream;

use crate::models::{
    collaboration::{
        CollaborationEvent, ConnectionInfo, UserPresence, 
        PresenceStatus, PublicUser
    },
    user::User,
};

/// Collaboration service for managing real-time events and user presence
#[derive(Debug, Clone)]
pub struct CollaborationService {
    /// Global event broadcaster
    event_sender: broadcast::Sender<CollaborationEvent>,
    
    /// Active connections by connection ID
    connections: Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
    
    /// User presence tracking
    user_presence: Arc<RwLock<HashMap<Uuid, UserPresence>>>,
    
    /// Project subscriptions (project_id -> Vec<user_id>)
    project_subscriptions: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
}

impl CollaborationService {
    /// Create a new collaboration service
    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(1000); // Buffer up to 1000 events
        
        Self {
            event_sender,
            connections: Arc::new(RwLock::new(HashMap::new())),
            user_presence: Arc::new(RwLock::new(HashMap::new())),
            project_subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe to events for a specific project
    pub async fn subscribe_to_project(
        &self, 
        project_id: Uuid, 
        user_id: Uuid
    ) -> impl Stream<Item = CollaborationEvent> {
        // Add user to project subscriptions
        {
            let mut subscriptions = self.project_subscriptions.write().await;
            subscriptions.entry(project_id).or_insert_with(Vec::new).push(user_id);
        }

        // Create receiver for this subscription
        let receiver = self.event_sender.subscribe();
        
        // Filter events for this project
        BroadcastStream::new(receiver)
            .filter_map(move |result| {
                async move {
                    match result {
                        Ok(event) if event.project_id == project_id => Some(event),
                        _ => None,
                    }
                }
            })
    }

    /// Subscribe to all events for a user (across all projects they have access to)
    #[allow(dead_code)]
    pub async fn subscribe_to_user_events(
        &self, 
        _user_id: Uuid
    ) -> impl Stream<Item = CollaborationEvent> {
        let receiver = self.event_sender.subscribe();
        
        // For now, return all events (in a real implementation, you'd filter by user permissions)
        BroadcastStream::new(receiver)
            .filter_map(move |result| {
                async move {
                    match result {
                        Ok(event) => Some(event),
                        Err(_) => None,
                    }
                }
            })
    }

    /// Broadcast an event to all subscribers
    pub async fn broadcast_event(&self, event: CollaborationEvent) -> Result<(), broadcast::error::SendError<CollaborationEvent>> {
        tracing::debug!("Broadcasting event: {} for project {}", event.event_type, event.project_id);
        self.event_sender.send(event)
            .map(|_| ())
            .map_err(|e| e)
    }

    /// Broadcast event to a specific project
    #[allow(dead_code)]
    pub async fn broadcast_to_project(&self, project_id: Uuid, event: CollaborationEvent) -> Result<(), broadcast::error::SendError<CollaborationEvent>> {
        // Ensure the event is tagged with the correct project
        let mut project_event = event;
        project_event.project_id = project_id;
        
        self.broadcast_event(project_event).await
    }

    /// Broadcast event to a specific user
    #[allow(dead_code)]
    pub async fn broadcast_to_user(&self, _user_id: Uuid, event: CollaborationEvent) -> Result<(), broadcast::error::SendError<CollaborationEvent>> {
        // For now, just broadcast globally (in a real implementation, you'd target specific users)
        self.broadcast_event(event).await
    }

    /// Register a new connection
    pub async fn register_connection(&self, connection_info: ConnectionInfo) {
        let connection_id = connection_info.connection_id;
        let user_id = connection_info.user_id;
        let project_id = connection_info.project_id;

        // Add connection
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id, connection_info);
        }

        // Update user presence to online
        {
            let mut presence = self.user_presence.write().await;
            if let Some(user_presence) = presence.get_mut(&user_id) {
                user_presence.status = PresenceStatus::Online;
                user_presence.last_seen = Utc::now();
                user_presence.current_project = Some(project_id);
            }
        }

        tracing::debug!("Registered connection {} for user {} on project {}", connection_id, user_id, project_id);
    }

    /// Unregister a connection
    pub async fn unregister_connection(&self, connection_id: Uuid) {
        let connection_info = {
            let mut connections = self.connections.write().await;
            connections.remove(&connection_id)
        };

        if let Some(connection) = connection_info {
            let user_id = connection.user_id;
            let project_id = connection.project_id;

            // Check if user has other active connections
            let has_other_connections = {
                let connections = self.connections.read().await;
                connections.values().any(|conn| conn.user_id == user_id)
            };

            // Update presence if no other connections
            if !has_other_connections {
                let mut presence = self.user_presence.write().await;
                if let Some(user_presence) = presence.get_mut(&user_id) {
                    user_presence.status = PresenceStatus::Offline;
                    user_presence.last_seen = Utc::now();
                    user_presence.current_project = None;
                }
            }

            // Remove from project subscriptions
            {
                let mut subscriptions = self.project_subscriptions.write().await;
                if let Some(project_users) = subscriptions.get_mut(&project_id) {
                    project_users.retain(|&id| id != user_id);
                    if project_users.is_empty() {
                        subscriptions.remove(&project_id);
                    }
                }
            }

            tracing::debug!("Unregistered connection {} for user {} on project {}", connection_id, user_id, project_id);
        }
    }

    /// Update user presence
    pub async fn update_user_presence(&self, user: &User, status: PresenceStatus, current_project: Option<Uuid>) {
        let user_presence = UserPresence {
            user_id: user.id,
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            avatar_url: user.avatar_url.clone(),
            last_seen: Utc::now(),
            status: status.clone(),
            current_project,
        };

        // Update presence state
        {
            let mut presence = self.user_presence.write().await;
            presence.insert(user.id, user_presence.clone());
        }

        // Broadcast presence event if there's a current project
        if let Some(project_id) = current_project {
            let presence_event = CollaborationEvent::new(
                "user_presence".to_string(),
                project_id,
                user,
                serde_json::to_value(&user_presence).unwrap_or_default(),
            );

            if let Err(e) = self.broadcast_event(presence_event).await {
                tracing::warn!("Failed to broadcast presence event: {}", e);
            }
        }
    }

    /// Get user presence for a project
    pub async fn get_project_presence(&self, project_id: Uuid) -> Vec<UserPresence> {
        let subscriptions = self.project_subscriptions.read().await;
        let presence = self.user_presence.read().await;

        if let Some(user_ids) = subscriptions.get(&project_id) {
            user_ids.iter()
                .filter_map(|user_id| presence.get(user_id))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all user presence
    #[allow(dead_code)]
    pub async fn get_all_presence(&self) -> Vec<UserPresence> {
        let presence = self.user_presence.read().await;
        presence.values().cloned().collect()
    }

    /// Get active connections count
    #[allow(dead_code)]
    pub async fn get_connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    /// Get active connections for a project
    #[allow(dead_code)]
    pub async fn get_project_connection_count(&self, project_id: Uuid) -> usize {
        let connections = self.connections.read().await;
        connections.values()
            .filter(|conn| conn.project_id == project_id)
            .count()
    }

    /// Cleanup stale connections (should be called periodically)
    #[allow(dead_code)]
    pub async fn cleanup_stale_connections(&self, max_age: chrono::Duration) {
        let cutoff = Utc::now() - max_age;
        let mut stale_connections = Vec::new();

        {
            let connections = self.connections.read().await;
            for (connection_id, connection) in connections.iter() {
                if connection.last_seen < cutoff {
                    stale_connections.push(*connection_id);
                }
            }
        }

        for connection_id in stale_connections {
            self.unregister_connection(connection_id).await;
        }
    }
}

impl Default for CollaborationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Event broadcasting helper functions
impl CollaborationService {
    /// Create and broadcast a task created event
    pub async fn broadcast_task_created(
        &self,
        task: &crate::models::task::Task,
        user: &User,
    ) -> Result<(), broadcast::error::SendError<CollaborationEvent>> {
        let event_data = serde_json::json!({
            "task": task,
            "created_by": PublicUser::from(user),
            "timestamp": Utc::now()
        });

        let event = CollaborationEvent::new(
            "task_created".to_string(),
            task.project_id,
            user,
            event_data,
        );

        self.broadcast_event(event).await
    }

    /// Create and broadcast a task updated event
    pub async fn broadcast_task_updated(
        &self,
        task: &crate::models::task::Task,
        user: &User,
        changes: Vec<String>,
    ) -> Result<(), broadcast::error::SendError<CollaborationEvent>> {
        let event_data = serde_json::json!({
            "task": task,
            "updated_by": PublicUser::from(user),
            "changes": changes,
            "timestamp": Utc::now()
        });

        let event = CollaborationEvent::new(
            "task_updated".to_string(),
            task.project_id,
            user,
            event_data,
        );

        self.broadcast_event(event).await
    }

    /// Create and broadcast a task attempt event
    pub async fn broadcast_task_attempt_created(
        &self,
        task_attempt: &crate::models::task_attempt::TaskAttempt,
        task: &crate::models::task::Task,
        user: &User,
    ) -> Result<(), broadcast::error::SendError<CollaborationEvent>> {
        let event_data = serde_json::json!({
            "task_attempt": task_attempt,
            "task": task,
            "created_by": PublicUser::from(user),
            "timestamp": Utc::now()
        });

        let event = CollaborationEvent::new(
            "task_attempt_created".to_string(),
            task.project_id,
            user,
            event_data,
        );

        self.broadcast_event(event).await
    }

    /// Create and broadcast a project updated event
    pub async fn broadcast_project_updated(
        &self,
        project: &crate::models::project::Project,
        user: &User,
        changes: Vec<String>,
    ) -> Result<(), broadcast::error::SendError<CollaborationEvent>> {
        let event_data = serde_json::json!({
            "project": project,
            "updated_by": PublicUser::from(user),
            "changes": changes,
            "timestamp": Utc::now()
        });

        let event = CollaborationEvent::new(
            "project_updated".to_string(),
            project.id,
            user,
            event_data,
        );

        self.broadcast_event(event).await
    }
}