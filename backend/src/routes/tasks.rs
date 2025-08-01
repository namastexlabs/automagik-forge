use axum::{
    extract::State, http::StatusCode, response::Json as ResponseJson, routing::get, Extension,
    Json, Router,
};
use utoipa;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    auth::UserContext,
    execution_monitor,
    models::{
        project::Project,
        task::{CreateTask, CreateTaskAndStart, Task, TaskWithAttemptStatus, UpdateTask},
        task_attempt::{CreateTaskAttempt, TaskAttempt},
        ApiResponse,
    },
};

#[utoipa::path(
    get,
    path = "/api/projects/{project_id}/tasks",
    params(
        ("project_id" = String, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "List all tasks for a project", body = ApiResponse<Vec<TaskWithAttemptStatus>>),
        (status = 404, description = "Project not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "tasks"
)]
pub async fn get_project_tasks(
    Extension(project): Extension<Project>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
) -> Result<ResponseJson<ApiResponse<Vec<TaskWithAttemptStatus>>>, StatusCode> {
    tracing::debug!("User {} requesting tasks for project {}", user_context.user.username, project.id);
    
    match Task::find_by_project_id_with_attempt_status(&app_state.db_pool, project.id).await {
        Ok(tasks) => Ok(ResponseJson(ApiResponse::success(tasks))),
        Err(e) => {
            tracing::error!("Failed to fetch tasks for project {}: {}", project.id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/projects/{project_id}/tasks/{task_id}",
    params(
        ("project_id" = String, Path, description = "Project ID"),
        ("task_id" = String, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Get task by ID", body = ApiResponse<Task>),
        (status = 404, description = "Task not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "tasks"
)]
pub async fn get_task(
    Extension(task): Extension<Task>,
) -> Result<ResponseJson<ApiResponse<Task>>, StatusCode> {
    Ok(ResponseJson(ApiResponse::success(task)))
}

#[utoipa::path(
    post,
    path = "/api/projects/{project_id}/tasks",
    params(
        ("project_id" = String, Path, description = "Project ID")
    ),
    request_body = CreateTask,
    responses(
        (status = 200, description = "Task created successfully", body = ApiResponse<Task>),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Project not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "tasks"
)]
pub async fn create_task(
    Extension(project): Extension<Project>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
    Json(mut payload): Json<CreateTask>,
) -> Result<ResponseJson<ApiResponse<Task>>, StatusCode> {
    let id = Uuid::new_v4();

    // Ensure the project_id in the payload matches the project from middleware
    payload.project_id = project.id;
    
    // Set the created_by field from the authenticated user if not already set
    if payload.created_by.is_none() {
        payload.created_by = Some(user_context.user.id);
    }

    tracing::debug!(
        "Creating task '{}' in project {} by user {}",
        payload.title,
        project.id,
        user_context.user.username
    );

    match Task::create(&app_state.db_pool, &payload, id).await {
        Ok(task) => {
            // Track task creation event
            app_state
                .track_analytics_event(
                    "task_created",
                    Some(serde_json::json!({
                    "task_id": task.id.to_string(),
                    "project_id": project.id.to_string(),
                    "has_description": task.description.is_some(),
                    })),
                )
                .await;

            // Broadcast real-time task creation event
            if let Err(e) = app_state.collaboration.broadcast_task_created(&task, &user_context.user).await {
                tracing::warn!("Failed to broadcast task creation event: {}", e);
            }

            Ok(ResponseJson(ApiResponse::success(task)))
        }
        Err(e) => {
            tracing::error!("Failed to create task: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_task_and_start(
    Extension(project): Extension<Project>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
    Json(mut payload): Json<CreateTaskAndStart>,
) -> Result<ResponseJson<ApiResponse<Task>>, StatusCode> {
    let task_id = Uuid::new_v4();

    // Ensure the project_id in the payload matches the project from middleware
    payload.project_id = project.id;
    
    // Set the created_by field from the authenticated user if not already set
    if payload.created_by.is_none() {
        payload.created_by = Some(user_context.user.id);
    }

    tracing::debug!(
        "Creating and starting task '{}' in project {} by user {}",
        payload.title,
        project.id,
        user_context.user.username
    );

    // Create the task first
    let create_task_payload = CreateTask {
        project_id: payload.project_id,
        title: payload.title.clone(),
        description: payload.description.clone(),
        wish_id: payload.wish_id.clone(),
        parent_task_attempt: payload.parent_task_attempt,
        created_by: payload.created_by,
        assigned_to: payload.assigned_to,
    };
    let task = match Task::create(&app_state.db_pool, &create_task_payload, task_id).await {
        Ok(task) => task,
        Err(e) => {
            tracing::error!("Failed to create task: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Create task attempt
    let executor_string = payload.executor.as_ref().map(|exec| exec.to_string());
    let attempt_payload = CreateTaskAttempt {
        executor: executor_string.clone(),
        base_branch: None, // Not supported in task creation endpoint, only in task attempts
        created_by: payload.created_by,
    };

    match TaskAttempt::create(&app_state.db_pool, &attempt_payload, task_id).await {
        Ok(attempt) => {
            app_state
                .track_analytics_event(
                    "task_created",
                    Some(serde_json::json!({
                        "task_id": task.id.to_string(),
                        "project_id": project.id.to_string(),
                        "has_description": task.description.is_some(),
                    })),
                )
                .await;

            app_state
                .track_analytics_event(
                    "task_attempt_started",
                    Some(serde_json::json!({
                        "task_id": task.id.to_string(),
                        "executor_type": executor_string.as_deref().unwrap_or("default"),
                        "attempt_id": attempt.id.to_string(),
                    })),
                )
                .await;

            // Broadcast real-time task creation event
            if let Err(e) = app_state.collaboration.broadcast_task_created(&task, &user_context.user).await {
                tracing::warn!("Failed to broadcast task creation event: {}", e);
            }

            // Broadcast real-time task attempt creation event
            if let Err(e) = app_state.collaboration.broadcast_task_attempt_created(&attempt, &task, &user_context.user).await {
                tracing::warn!("Failed to broadcast task attempt creation event: {}", e);
            }

            // Start execution asynchronously (don't block the response)
            let app_state_clone = app_state.clone();
            let attempt_id = attempt.id;
            tokio::spawn(async move {
                if let Err(e) = TaskAttempt::start_execution(
                    &app_state_clone.db_pool,
                    &app_state_clone,
                    attempt_id,
                    task_id,
                    project.id,
                )
                .await
                {
                    tracing::error!(
                        "Failed to start execution for task attempt {}: {}",
                        attempt_id,
                        e
                    );
                }
            });

            Ok(ResponseJson(ApiResponse::success(task)))
        }
        Err(e) => {
            tracing::error!("Failed to create task attempt: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    put,
    path = "/api/projects/{project_id}/tasks/{task_id}",
    params(
        ("project_id" = String, Path, description = "Project ID"),
        ("task_id" = String, Path, description = "Task ID")
    ),
    request_body = UpdateTask,
    responses(
        (status = 200, description = "Task updated successfully", body = ApiResponse<Task>),
        (status = 404, description = "Task or project not found"),
        (status = 400, description = "Invalid input"),
        (status = 500, description = "Internal server error")
    ),
    tag = "tasks"
)]
pub async fn update_task(
    Extension(project): Extension<Project>,
    Extension(existing_task): Extension<Task>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
    Json(payload): Json<UpdateTask>,
) -> Result<ResponseJson<ApiResponse<Task>>, StatusCode> {
    tracing::debug!(
        "User {} updating task {} in project {}", 
        user_context.user.username, 
        existing_task.id, 
        project.id
    );
    
    // Track what fields are changing
    let mut changes = Vec::new();
    
    // Use existing values if not provided in update, and track changes
    let title = if let Some(new_title) = &payload.title {
        if new_title != &existing_task.title {
            changes.push("title".to_string());
        }
        new_title.clone()
    } else {
        existing_task.title.clone()
    };
    
    let description = if let Some(new_desc) = &payload.description {
        if Some(new_desc) != existing_task.description.as_ref() {
            changes.push("description".to_string());
        }
        Some(new_desc.clone())
    } else {
        existing_task.description.clone()
    };
    
    let status = if let Some(new_status) = payload.status {
        if new_status != existing_task.status {
            changes.push("status".to_string());
        }
        new_status
    } else {
        existing_task.status
    };
    
    let wish_id = if let Some(new_wish_id) = &payload.wish_id {
        if new_wish_id != &existing_task.wish_id {
            changes.push("wish_id".to_string());
        }
        new_wish_id.clone()
    } else {
        existing_task.wish_id.clone()
    };
    
    let parent_task_attempt = if payload.parent_task_attempt != existing_task.parent_task_attempt {
        if payload.parent_task_attempt.is_some() {
            changes.push("parent_task_attempt".to_string());
        }
        payload.parent_task_attempt.or(existing_task.parent_task_attempt)
    } else {
        existing_task.parent_task_attempt
    };
    
    let assigned_to = if payload.assigned_to != existing_task.assigned_to {
        changes.push("assigned_to".to_string());
        payload.assigned_to.or(existing_task.assigned_to)
    } else {
        existing_task.assigned_to
    };

    match Task::update(
        &app_state.db_pool,
        existing_task.id,
        project.id,
        title,
        description,
        status,
        wish_id,
        parent_task_attempt,
        assigned_to,
    )
    .await
    {
        Ok(task) => {
            // Broadcast real-time task update event if there were changes
            if !changes.is_empty() {
                if let Err(e) = app_state.collaboration.broadcast_task_updated(&task, &user_context.user, changes).await {
                    tracing::warn!("Failed to broadcast task update event: {}", e);
                }
            }
            
            Ok(ResponseJson(ApiResponse::success(task)))
        }
        Err(e) => {
            tracing::error!("Failed to update task: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/projects/{project_id}/tasks/{task_id}",
    params(
        ("project_id" = String, Path, description = "Project ID"),
        ("task_id" = String, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task deleted successfully"),
        (status = 404, description = "Task or project not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "tasks"
)]
pub async fn delete_task(
    Extension(project): Extension<Project>,
    Extension(task): Extension<Task>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    tracing::debug!(
        "User {} deleting task {} in project {}", 
        user_context.user.username, 
        task.id, 
        project.id
    );
    // Clean up all worktrees for this task before deletion
    if let Err(e) = execution_monitor::cleanup_task_worktrees(&app_state.db_pool, task.id).await {
        tracing::error!("Failed to cleanup worktrees for task {}: {}", task.id, e);
        // Continue with deletion even if cleanup fails
    }

    // Clean up all executor sessions for this task before deletion
    match TaskAttempt::find_by_task_id(&app_state.db_pool, task.id).await {
        Ok(task_attempts) => {
            for attempt in task_attempts {
                if let Err(e) =
                    crate::models::executor_session::ExecutorSession::delete_by_task_attempt_id(
                        &app_state.db_pool,
                        attempt.id,
                    )
                    .await
                {
                    tracing::error!(
                        "Failed to cleanup executor sessions for task attempt {}: {}",
                        attempt.id,
                        e
                    );
                    // Continue with deletion even if session cleanup fails
                } else {
                    tracing::debug!(
                        "Cleaned up executor sessions for task attempt {}",
                        attempt.id
                    );
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get task attempts for session cleanup: {}", e);
            // Continue with deletion even if we can't get task attempts
        }
    }

    match Task::delete(&app_state.db_pool, task.id, project.id).await {
        Ok(rows_affected) => {
            if rows_affected == 0 {
                Err(StatusCode::NOT_FOUND)
            } else {
                Ok(ResponseJson(ApiResponse::success(())))
            }
        }
        Err(e) => {
            tracing::error!("Failed to delete task: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub fn tasks_project_router() -> Router<AppState> {
    use axum::routing::post;

    Router::new()
        .route(
            "/projects/:project_id/tasks",
            get(get_project_tasks).post(create_task),
        )
        .route(
            "/projects/:project_id/tasks/create-and-start",
            post(create_task_and_start),
        )
}

pub fn tasks_with_id_router() -> Router<AppState> {
    Router::new().route(
        "/projects/:project_id/tasks/:task_id",
        get(get_task).put(update_task).delete(delete_task),
    )
}
