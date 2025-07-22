use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Vibe Kanban API",
        version = "1.0.0",
        description = "A task and project management API for Vibe Kanban",
        contact(
            name = "Vibe Kanban Team",
            url = "https://github.com/your-org/vibe-kanban"
        )
    ),
    paths(
        crate::routes::health::health_check,
        crate::routes::projects::get_projects,
        crate::routes::projects::get_project,
        crate::routes::projects::create_project,
        crate::routes::projects::update_project,
        crate::routes::projects::delete_project,
        crate::routes::tasks::get_project_tasks,
        crate::routes::tasks::get_task,
        crate::routes::tasks::create_task,
        crate::routes::tasks::update_task,
        crate::routes::tasks::delete_task,
        crate::routes::task_attempts::get_task_attempts,
        crate::routes::task_attempts::create_task_attempt,
        crate::routes::task_templates::list_templates,
        crate::routes::task_templates::list_project_templates,
        crate::routes::task_templates::list_global_templates,
        crate::routes::task_templates::get_template,
        crate::routes::task_templates::create_template,
        crate::routes::task_templates::update_template,
        crate::routes::task_templates::delete_template,
        // Auth routes
        crate::routes::auth::device_start,
        crate::routes::auth::device_poll,
        crate::routes::auth::github_check_token,
        // Config routes
        crate::routes::config::get_config,
        crate::routes::config::update_config,
        crate::routes::config::get_config_constants,
        crate::routes::config::get_mcp_servers,
        crate::routes::config::update_mcp_servers,
        // Filesystem routes
        crate::routes::filesystem::list_directory,
        crate::routes::filesystem::validate_git_path,
        crate::routes::filesystem::create_git_repo,
    ),
    components(
        schemas(
            crate::models::project::Project,
            crate::models::project::CreateProject,
            crate::models::project::UpdateProject,
            crate::models::project::ProjectWithBranch,
            crate::models::project::GitBranch,
            crate::models::task::Task,
            crate::models::task::TaskStatus,
            crate::models::task::TaskWithAttemptStatus,
            crate::models::task::CreateTask,
            crate::models::task::UpdateTask,
            crate::models::task_attempt::TaskAttempt,
            crate::models::task_attempt::TaskAttemptStatus,
            crate::models::task_attempt::CreateTaskAttempt,
            crate::models::task_template::TaskTemplate,
            crate::models::task_template::CreateTaskTemplate,
            crate::models::task_template::UpdateTaskTemplate,
            crate::executor::ExecutorConfig,
            crate::executor::NormalizedConversation,
            crate::executor::NormalizedEntry,
            crate::executor::NormalizedEntryType,
            crate::executor::ActionType,
            // Auth schemas
            crate::routes::auth::DeviceStartResponse,
            // Config schemas
            crate::routes::config::ConfigConstants,
            // Filesystem schemas
            crate::routes::filesystem::DirectoryEntry,
            crate::routes::filesystem::DirectoryListResponse,
        )
    ),
    tags(
        (name = "health", description = "Health check operations"),
        (name = "projects", description = "Project management operations"),
        (name = "tasks", description = "Task management operations"),
        (name = "task_attempts", description = "Task execution attempt operations"),
        (name = "task_templates", description = "Task template operations"),
        (name = "auth", description = "Authentication operations"),
        (name = "config", description = "Configuration operations"),
        (name = "filesystem", description = "File system operations"),
    )
)]
pub struct ApiDoc;