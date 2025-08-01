use std::time::Duration;
use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, Sse},
    routing::get,
    Router,
    Extension,
};
use futures_util::{stream::Stream, StreamExt};
use serde::Deserialize;
use uuid::Uuid;
use chrono::Utc;

use crate::{
    app_state::AppState,
    auth::UserContext,
    models::collaboration::{ConnectionInfo, PresenceStatus},
};

/// Query parameters for SSE streams
#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    /// Optional cursor to resume streaming from specific event
    since_event_id: Option<Uuid>,
}

/// SSE handler for project-specific collaboration events
///
/// GET /api/projects/:project_id/events/stream
pub async fn project_events_stream(
    Path(project_id): Path<Uuid>,
    Query(_query): Query<StreamQuery>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let user = &user_context.user;
    let connection_id = Uuid::new_v4();

    // Register connection
    let connection_info = ConnectionInfo {
        user_id: user.id,
        project_id,
        connection_id,
        connected_at: Utc::now(),
        last_seen: Utc::now(),
    };

    app_state.collaboration.register_connection(connection_info).await;

    // Update user presence to online for this project
    app_state.collaboration.update_user_presence(
        user, 
        PresenceStatus::Online, 
        Some(project_id)
    ).await;

    // Subscribe to project events
    let event_stream = app_state.collaboration.subscribe_to_project(project_id, user.id).await;

    // Convert collaboration events to SSE events
    let sse_stream = async_stream::stream! {
        let mut event_stream = Box::pin(event_stream);
        
        loop {
            tokio::select! {
                // Wait for collaboration events
                event_result = event_stream.next() => {
                    match event_result {
                        Some(collaboration_event) => {
                            let event_data = match serde_json::to_string(&collaboration_event) {
                                Ok(data) => data,
                                Err(e) => {
                                    tracing::error!("Failed to serialize collaboration event: {}", e);
                                    continue;
                                }
                            };
                            
                            let sse_event = Event::default()
                                .event(&collaboration_event.event_type)
                                .id(collaboration_event.event_id.to_string())
                                .data(event_data);
                                
                            yield Ok(sse_event);
                        }
                        None => {
                            tracing::debug!("Collaboration event stream ended for project {}", project_id);
                            break;
                        }
                    }
                }
                
                // Send heartbeat every 30 seconds
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    let heartbeat = Event::default()
                        .event("heartbeat")
                        .data("ping");
                    yield Ok(heartbeat);
                }
            }
        }
        
        // Cleanup when stream ends
        app_state.collaboration.unregister_connection(connection_id).await;
    };

    Sse::new(sse_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive")
    )
}

/// SSE handler for user presence events
///
/// GET /api/projects/:project_id/presence/stream
pub async fn project_presence_stream(
    Path(project_id): Path<Uuid>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let user = &user_context.user;

    // Subscribe to all events and filter for presence events
    let event_stream = app_state.collaboration.subscribe_to_project(project_id, user.id).await;

    // Convert presence events to SSE events
    let sse_stream = async_stream::stream! {
        let mut event_stream = Box::pin(event_stream);
        
        // Send initial presence data
        let initial_presence = app_state.collaboration.get_project_presence(project_id).await;
        let initial_data = match serde_json::to_string(&initial_presence) {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("Failed to serialize initial presence: {}", e);
                "[]".to_string()
            }
        };
        
        let initial_event = Event::default()
            .event("presence_init")
            .data(initial_data);
        yield Ok(initial_event);
        
        loop {
            tokio::select! {
                // Wait for collaboration events
                event_result = event_stream.next() => {
                    match event_result {
                        Some(collaboration_event) if collaboration_event.event_type == "user_presence" => {
                            let event_data = match serde_json::to_string(&collaboration_event) {
                                Ok(data) => data,
                                Err(e) => {
                                    tracing::error!("Failed to serialize presence event: {}", e);
                                    continue;
                                }
                            };
                            
                            let sse_event = Event::default()
                                .event("presence_update")
                                .id(collaboration_event.event_id.to_string())
                                .data(event_data);
                                
                            yield Ok(sse_event);
                        }
                        Some(_) => {
                            // Ignore non-presence events
                            continue;
                        }
                        None => {
                            tracing::debug!("Presence event stream ended for project {}", project_id);
                            break;
                        }
                    }
                }
                
                // Send heartbeat every 30 seconds
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    let heartbeat = Event::default()
                        .event("heartbeat")
                        .data("ping");
                    yield Ok(heartbeat);
                }
            }
        }
    };

    Sse::new(sse_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive")
    )
}

/// Get current presence for a project
///
/// GET /api/projects/:project_id/presence
pub async fn get_project_presence(
    Path(project_id): Path<Uuid>,
    State(app_state): State<AppState>,
    Extension(_user_context): Extension<UserContext>,
) -> axum::response::Json<crate::models::ApiResponse<Vec<crate::models::collaboration::UserPresence>>> {
    let presence = app_state.collaboration.get_project_presence(project_id).await;
    axum::response::Json(crate::models::ApiResponse::success(presence))
}

/// Update user presence for a project
///
/// POST /api/projects/:project_id/presence
#[derive(Debug, Deserialize)]
pub struct UpdatePresenceRequest {
    pub status: PresenceStatus,
}

pub async fn update_user_presence(
    Path(project_id): Path<Uuid>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
    axum::extract::Json(request): axum::extract::Json<UpdatePresenceRequest>,
) -> axum::response::Json<crate::models::ApiResponse<String>> {
    let user = &user_context.user;

    app_state.collaboration.update_user_presence(
        user, 
        request.status, 
        Some(project_id)
    ).await;

    axum::response::Json(crate::models::ApiResponse::success("Presence updated".to_string()))
}

/// Router for collaboration routes
pub fn collaboration_router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/events/stream", get(project_events_stream))
        .route("/projects/:project_id/presence/stream", get(project_presence_stream))
        .route("/projects/:project_id/presence", get(get_project_presence).post(update_user_presence))
}