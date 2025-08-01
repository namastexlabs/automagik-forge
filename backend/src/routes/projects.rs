use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::get,
    Extension, Json, Router,
};
use utoipa;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    auth::UserContext,
    models::{
        project::{
            CreateBranch, CreateProject, GitBranch, Project, ProjectWithBranch, ProjectWithCreator, SearchMatchType,
            SearchResult, UpdateProject,
        },
        // user_preferences::UserPreferences,
        ApiResponse,
    },
};

#[utoipa::path(
    get,
    path = "/api/projects",
    responses(
        (status = 200, description = "List all projects", body = ApiResponse<Vec<ProjectWithCreator>>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    tag = "projects"
)]
pub async fn get_projects(
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
) -> Result<ResponseJson<ApiResponse<Vec<ProjectWithCreator>>>, StatusCode> {
    tracing::debug!("User {} requesting projects list", user_context.user.username);
    
    match Project::find_all_with_creators(&app_state.db_pool).await {
        Ok(projects) => Ok(ResponseJson(ApiResponse::success(projects))),
        Err(e) => {
            tracing::error!("Failed to fetch projects: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/projects/{id}",
    params(
        ("id" = String, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Get project by ID", body = ApiResponse<Project>),
        (status = 404, description = "Project not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "projects"
)]
pub async fn get_project(
    Extension(project): Extension<Project>,
) -> Result<ResponseJson<ApiResponse<Project>>, StatusCode> {
    Ok(ResponseJson(ApiResponse::success(project)))
}

pub async fn get_project_with_branch(
    Extension(project): Extension<Project>,
) -> Result<ResponseJson<ApiResponse<ProjectWithBranch>>, StatusCode> {
    Ok(ResponseJson(ApiResponse::success(
        project.with_branch_info(),
    )))
}

pub async fn get_project_branches(
    Extension(project): Extension<Project>,
) -> Result<ResponseJson<ApiResponse<Vec<GitBranch>>>, StatusCode> {
    match project.get_all_branches() {
        Ok(branches) => Ok(ResponseJson(ApiResponse::success(branches))),
        Err(e) => {
            tracing::error!("Failed to get branches for project {}: {}", project.id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_project_branch(
    Extension(project): Extension<Project>,
    Json(payload): Json<CreateBranch>,
) -> Result<ResponseJson<ApiResponse<GitBranch>>, StatusCode> {
    // Validate branch name
    if payload.name.trim().is_empty() {
        return Ok(ResponseJson(ApiResponse::error(
            "Branch name cannot be empty",
        )));
    }

    // Check if branch name contains invalid characters
    if payload.name.contains(' ') {
        return Ok(ResponseJson(ApiResponse::error(
            "Branch name cannot contain spaces",
        )));
    }

    match project.create_branch(&payload.name, payload.base_branch.as_deref()) {
        Ok(branch) => Ok(ResponseJson(ApiResponse::success(branch))),
        Err(e) => {
            tracing::error!(
                "Failed to create branch '{}' for project {}: {}",
                payload.name,
                project.id,
                e
            );
            Ok(ResponseJson(ApiResponse::error(&format!(
                "Failed to create branch: {}",
                e
            ))))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/projects",
    request_body = CreateProject,
    responses(
        (status = 200, description = "Project created successfully", body = ApiResponse<Project>),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    tag = "projects"
)]
pub async fn create_project(
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
    Json(mut payload): Json<CreateProject>,
) -> Result<ResponseJson<ApiResponse<Project>>, StatusCode> {
    let id = Uuid::new_v4();
    
    // Set the created_by field from the authenticated user
    payload.created_by = Some(user_context.user.id);

    tracing::debug!("Creating project '{}' by user {}", payload.name, user_context.user.username);

    // Check if git repo path is already used by another project
    match Project::find_by_git_repo_path(&app_state.db_pool, &payload.git_repo_path).await {
        Ok(Some(_)) => {
            return Ok(ResponseJson(ApiResponse::error(
                "A project with this git repository path already exists",
            )));
        }
        Ok(None) => {
            // Path is available, continue
        }
        Err(e) => {
            tracing::error!("Failed to check for existing git repo path: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Validate and setup git repository
    let path = std::path::Path::new(&payload.git_repo_path);

    if payload.use_existing_repo {
        // For existing repos, validate that the path exists and is a git repository
        if !path.exists() {
            return Ok(ResponseJson(ApiResponse::error(
                "The specified path does not exist",
            )));
        }

        if !path.is_dir() {
            return Ok(ResponseJson(ApiResponse::error(
                "The specified path is not a directory",
            )));
        }

        if !path.join(".git").exists() {
            return Ok(ResponseJson(ApiResponse::error(
                "The specified directory is not a git repository",
            )));
        }
    } else {
        // For new repos, create directory and initialize git

        // Create directory if it doesn't exist
        if !path.exists() {
            if let Err(e) = std::fs::create_dir_all(path) {
                tracing::error!("Failed to create directory: {}", e);
                return Ok(ResponseJson(ApiResponse::error(&format!(
                    "Failed to create directory: {}",
                    e
                ))));
            }
        }

        // Check if it's already a git repo, if not initialize it
        if !path.join(".git").exists() {
            match std::process::Command::new("git")
                .arg("init")
                .current_dir(path)
                .output()
            {
                Ok(output) => {
                    if !output.status.success() {
                        let error_msg = String::from_utf8_lossy(&output.stderr);
                        tracing::error!("Git init failed: {}", error_msg);
                        return Ok(ResponseJson(ApiResponse::error(&format!(
                            "Git init failed: {}",
                            error_msg
                        ))));
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to run git init: {}", e);
                    return Ok(ResponseJson(ApiResponse::error(&format!(
                        "Failed to run git init: {}",
                        e
                    ))));
                }
            }
        }
    }

    match Project::create(&app_state.db_pool, &payload, id).await {
        Ok(project) => {
            // Track project creation event
            app_state
                .track_analytics_event(
                    "project_created",
                    Some(serde_json::json!({
                        "project_id": project.id.to_string(),
                        "use_existing_repo": payload.use_existing_repo,
                        "has_setup_script": payload.setup_script.is_some(),
                        "has_dev_script": payload.dev_script.is_some(),
                    })),
                )
                .await;

            Ok(ResponseJson(ApiResponse::success(project)))
        }
        Err(e) => {
            tracing::error!("Failed to create project: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    put,
    path = "/api/projects/{id}",
    params(
        ("id" = String, Path, description = "Project ID")
    ),
    request_body = UpdateProject,
    responses(
        (status = 200, description = "Project updated successfully", body = ApiResponse<Project>),
        (status = 404, description = "Project not found"),
        (status = 400, description = "Invalid input"),
        (status = 500, description = "Internal server error")
    ),
    tag = "projects"
)]
pub async fn update_project(
    Extension(existing_project): Extension<Project>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
    Json(payload): Json<UpdateProject>,
) -> Result<ResponseJson<ApiResponse<Project>>, StatusCode> {
    tracing::debug!("User {} updating project {}", user_context.user.username, existing_project.id);
    
    // If git_repo_path is being changed, check if the new path is already used by another project
    if let Some(new_git_repo_path) = &payload.git_repo_path {
        if new_git_repo_path != &existing_project.git_repo_path {
            match Project::find_by_git_repo_path_excluding_id(
                &app_state.db_pool,
                new_git_repo_path,
                existing_project.id,
            )
            .await
            {
                Ok(Some(_)) => {
                    return Ok(ResponseJson(ApiResponse::error(
                        "A project with this git repository path already exists",
                    )));
                }
                Ok(None) => {
                    // Path is available, continue
                }
                Err(e) => {
                    tracing::error!("Failed to check for existing git repo path: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }
    }

    // Destructure payload to handle field updates.
    // This allows us to treat `None` from the payload as an explicit `null` to clear a field,
    // as the frontend currently sends all fields on update.
    let UpdateProject {
        name,
        git_repo_path,
        setup_script,
        dev_script,
        cleanup_script,
    } = payload;

    // Track what fields are changing
    let mut changes = Vec::new();
    
    let name = if let Some(new_name) = name {
        if new_name != existing_project.name {
            changes.push("name".to_string());
        }
        new_name
    } else {
        existing_project.name.clone()
    };
    
    let git_repo_path = if let Some(new_path) = git_repo_path {
        if new_path != existing_project.git_repo_path {
            changes.push("git_repo_path".to_string());
        }
        new_path
    } else {
        existing_project.git_repo_path.clone()
    };
    
    if setup_script != existing_project.setup_script {
        changes.push("setup_script".to_string());
    }
    
    if dev_script != existing_project.dev_script {
        changes.push("dev_script".to_string());
    }
    
    if cleanup_script != existing_project.cleanup_script {
        changes.push("cleanup_script".to_string());
    }

    match Project::update(
        &app_state.db_pool,
        existing_project.id,
        name,
        git_repo_path,
        setup_script,
        dev_script,
        cleanup_script,
    )
    .await
    {
        Ok(project) => {
            
            Ok(ResponseJson(ApiResponse::success(project)))
        }
        Err(e) => {
            tracing::error!("Failed to update project: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/projects/{id}",
    params(
        ("id" = String, Path, description = "Project ID")
    ),
    responses(
        (status = 200, description = "Project deleted successfully"),
        (status = 404, description = "Project not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "projects"
)]
pub async fn delete_project(
    Extension(project): Extension<Project>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    tracing::debug!("User {} deleting project {}", user_context.user.username, project.id);
    match Project::delete(&app_state.db_pool, project.id).await {
        Ok(rows_affected) => {
            if rows_affected == 0 {
                Err(StatusCode::NOT_FOUND)
            } else {
                Ok(ResponseJson(ApiResponse::success(())))
            }
        }
        Err(e) => {
            tracing::error!("Failed to delete project: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(serde::Deserialize)]
pub struct OpenEditorRequest {
    #[allow(dead_code)]
    editor_type: Option<String>,
}

/*
pub async fn open_project_in_editor(
    Extension(project): Extension<Project>,
    State(app_state): State<AppState>,
    Extension(user_context): Extension<UserContext>,
    Json(payload): Json<Option<OpenEditorRequest>>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    tracing::debug!("User {} opening project {} in editor", user_context.user.username, project.id);
    
    // Get user preferences for editor settings
    let user_preferences = match UserPreferences::get_or_create_for_user(&app_state.db_pool, user_context.user.id).await {
        Ok(prefs) => prefs,
        Err(e) => {
            tracing::error!("Failed to get user preferences: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Get editor command from user preferences or override
    let editor_command = if let Some(ref request) = payload {
        if let Some(ref editor_type) = request.editor_type {
            // Create a temporary editor config with the override
            use crate::models::config::{EditorConfig, EditorType};
            let override_editor_type = match editor_type.as_str() {
                "vscode" => EditorType::VSCode,
                "cursor" => EditorType::Cursor,
                "windsurf" => EditorType::Windsurf,
                "intellij" => EditorType::IntelliJ,
                "zed" => EditorType::Zed,
                "custom" => EditorType::Custom,
                _ => user_preferences.editor_type.clone(),
            };
            let temp_config = EditorConfig {
                editor_type: override_editor_type,
                custom_command: user_preferences.editor_custom_command.clone(),
            };
            temp_config.get_command()
        } else {
            user_preferences.get_editor_command()
        }
    } else {
        user_preferences.get_editor_command()
    };

    // Open editor in the project directory
    let mut cmd = std::process::Command::new(&editor_command[0]);
    for arg in &editor_command[1..] {
        cmd.arg(arg);
    }
    cmd.arg(&project.git_repo_path);

    match cmd.spawn() {
        Ok(_) => {
            tracing::info!(
                "Opened editor ({}) for project {} at path: {}",
                editor_command.join(" "),
                project.id,
                project.git_repo_path
            );
            Ok(ResponseJson(ApiResponse::success(())))
        }
        Err(e) => {
            tracing::error!(
                "Failed to open editor ({}) for project {}: {}",
                editor_command.join(" "),
                project.id,
                e
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
*/

pub async fn search_project_files(
    Extension(project): Extension<Project>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<ResponseJson<ApiResponse<Vec<SearchResult>>>, StatusCode> {
    let query = match params.get("q") {
        Some(q) if !q.trim().is_empty() => q.trim(),
        _ => {
            return Ok(ResponseJson(ApiResponse::error(
                "Query parameter 'q' is required and cannot be empty",
            )));
        }
    };

    // Search files in the project repository
    match search_files_in_repo(&project.git_repo_path, query).await {
        Ok(results) => Ok(ResponseJson(ApiResponse::success(results))),
        Err(e) => {
            tracing::error!("Failed to search files: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn search_files_in_repo(
    repo_path: &str,
    query: &str,
) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
    use std::path::Path;

    use ignore::WalkBuilder;

    let repo_path = Path::new(repo_path);

    if !repo_path.exists() {
        return Err("Repository path does not exist".into());
    }

    let mut results = Vec::new();
    let query_lower = query.to_lowercase();

    // Use ignore::WalkBuilder to respect gitignore files
    let walker = WalkBuilder::new(repo_path)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .hidden(false)
        .build();

    for result in walker {
        let entry = result?;
        let path = entry.path();

        // Skip the root directory itself
        if path == repo_path {
            continue;
        }

        let relative_path = path.strip_prefix(repo_path)?;

        // Skip .git directory and its contents
        if relative_path
            .components()
            .any(|component| component.as_os_str() == ".git")
        {
            continue;
        }
        let relative_path_str = relative_path.to_string_lossy().to_lowercase();

        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // Check for matches
        if file_name.contains(&query_lower) {
            results.push(SearchResult {
                path: relative_path.to_string_lossy().to_string(),
                is_file: path.is_file(),
                match_type: SearchMatchType::FileName,
            });
        } else if relative_path_str.contains(&query_lower) {
            // Check if it's a directory name match or full path match
            let match_type = if path
                .parent()
                .and_then(|p| p.file_name())
                .map(|name| name.to_string_lossy().to_lowercase())
                .unwrap_or_default()
                .contains(&query_lower)
            {
                SearchMatchType::DirectoryName
            } else {
                SearchMatchType::FullPath
            };

            results.push(SearchResult {
                path: relative_path.to_string_lossy().to_string(),
                is_file: path.is_file(),
                match_type,
            });
        }
    }

    // Sort results by priority: FileName > DirectoryName > FullPath
    results.sort_by(|a, b| {
        use SearchMatchType::*;
        let priority = |match_type: &SearchMatchType| match match_type {
            FileName => 0,
            DirectoryName => 1,
            FullPath => 2,
        };

        priority(&a.match_type)
            .cmp(&priority(&b.match_type))
            .then_with(|| a.path.cmp(&b.path))
    });

    // Limit to top 10 results
    results.truncate(10);

    Ok(results)
}

pub fn projects_base_router() -> Router<AppState> {
    Router::new().route("/projects", get(get_projects).post(create_project))
}

pub fn projects_with_id_router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:id",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/projects/:id/with-branch", get(get_project_with_branch))
        .route(
            "/projects/:id/branches",
            get(get_project_branches).post(create_project_branch),
        )
        .route("/projects/:id/search", get(search_project_files))
        // .route("/projects/:id/open-editor", post(open_project_in_editor))
}
