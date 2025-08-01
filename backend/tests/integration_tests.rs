use automagik_forge::{
    auth::{generate_jwt_token, hash_token, JwtConfig},
    models::{
        user::{User, CreateUser, UpdateUser},
        user_session::{UserSession, SessionType, CreateUserSession},
        project::{Project, CreateProject},
        task::{Task, CreateTask, TaskStatus},
        task_attempt::{TaskAttempt, CreateTaskAttempt, TaskAttemptStatus},
    },
    services::git_service::GitService,
    app_state::AppState,
};
use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, Method, StatusCode},
    middleware::Next,
    response::Response,
    body::Body,
};
use chrono::{Duration, Utc};
use git2::Repository;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;
use tokio;

/// Helper function to create an in-memory test database
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();
        
    pool
}

/// Helper function to create a test Git repository
fn create_test_git_repo(user_name: &str, user_email: &str) -> (TempDir, Repository) {
    let temp_dir = TempDir::new().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();

    // Configure the repository
    let mut config = repo.config().unwrap();
    config.set_str("user.name", user_name).unwrap();
    config.set_str("user.email", user_email).unwrap();
    config.set_str("init.defaultBranch", "main").unwrap();

    // Create initial commit
    let signature = git2::Signature::now(user_name, user_email).unwrap();
    let tree_id = {
        let tree_builder = repo.treebuilder(None).unwrap();
        tree_builder.write().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();

    repo.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    ).unwrap();

    repo.set_head("refs/heads/main").unwrap();

    (temp_dir, repo)
}

/// Helper function to create a test user with session
async fn create_user_with_session(
    pool: &SqlitePool,
    github_id: i64,
    username: &str,
    email: &str,
    session_type: SessionType,
) -> (User, UserSession, String) {
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id,
        username: username.to_string(),
        email: email.to_string(),
        display_name: Some(format!("User {}", username)),
        avatar_url: Some(format!("https://github.com/{}.png", username)),
        github_token: Some("encrypted_token".to_string()),
        is_admin: Some(false),
    };
    
    let user = User::create(pool, &create_data, user_id).await.unwrap();
    
    // Generate JWT token and create session
    let jwt_config = JwtConfig::default();
    let session_id = Uuid::new_v4();
    let jwt_token = generate_jwt_token(user.id, session_id, session_type, &jwt_config).unwrap();
    let token_hash = hash_token(&jwt_token);
    
    let expires_at = match session_type {
        SessionType::Web => Utc::now() + Duration::hours(UserSession::WEB_SESSION_DURATION_HOURS),
        SessionType::Mcp => Utc::now() + Duration::days(UserSession::MCP_SESSION_DURATION_DAYS),
    };
    
    let session_data = CreateUserSession {
        user_id: user.id,
        token_hash: token_hash.clone(),
        session_type,
        client_info: Some("Integration Test Client".to_string()),
        expires_at,
    };
    
    let session = UserSession::create(pool, &session_data, session_id).await.unwrap();
    
    (user, session, jwt_token)
}

#[tokio::test]
async fn test_complete_multiuser_project_workflow() {
    let pool = setup_test_db().await;
    
    // Create multiple users with different roles
    let (admin_user, admin_session, admin_token) = create_user_with_session(
        &pool, 111, "admin", "admin@example.com", SessionType::Web
    ).await;
    
    // Update admin user to be admin
    let admin_update = UpdateUser {
        username: None,
        email: None,
        display_name: None,
        avatar_url: None,
        github_token: None,
        is_admin: Some(true),
        is_whitelisted: None,
    };
    let admin_user = User::update(&pool, admin_user.id, &admin_update).await.unwrap();
    
    let (dev1_user, dev1_session, dev1_token) = create_user_with_session(
        &pool, 222, "dev1", "dev1@example.com", SessionType::Web
    ).await;
    
    let (dev2_user, dev2_session, dev2_token) = create_user_with_session(
        &pool, 333, "dev2", "dev2@example.com", SessionType::Web
    ).await;
    
    // Create project by admin
    let (repo_dir, _repo) = create_test_git_repo(&admin_user.username, &admin_user.email);
    let project_id = Uuid::new_v4();
    let create_project_data = CreateProject {
        name: "Multiuser Test Project".to_string(),
        git_repo_path: repo_dir.path().to_string_lossy().to_string(),
        created_by: Some(admin_user.id),
    };
    
    let project = Project::create(&pool, &create_project_data, project_id).await.unwrap();
    
    // Verify project creation
    assert_eq!(project.name, "Multiuser Test Project");
    assert_eq!(project.created_by, Some(admin_user.id));
    
    // Create tasks assigned to different developers
    let task1_id = Uuid::new_v4();
    let create_task1_data = CreateTask {
        project_id: project.id,
        title: "Implement user authentication".to_string(),
        description: Some("Create secure authentication system".to_string()),
        wish_id: "auth-feature".to_string(),
        parent_task_attempt: None,
        created_by: Some(admin_user.id),
        assigned_to: Some(dev1_user.id),
    };
    
    let task1 = Task::create(&pool, &create_task1_data, task1_id).await.unwrap();
    
    let task2_id = Uuid::new_v4();
    let create_task2_data = CreateTask {
        project_id: project.id,
        title: "Design user interface".to_string(),
        description: Some("Create responsive UI components".to_string()),
        wish_id: "ui-feature".to_string(),
        parent_task_attempt: None,
        created_by: Some(admin_user.id),
        assigned_to: Some(dev2_user.id),
    };
    
    let task2 = Task::create(&pool, &create_task2_data, task2_id).await.unwrap();
    
    // Verify task assignments
    assert_eq!(task1.assigned_to, Some(dev1_user.id));
    assert_eq!(task2.assigned_to, Some(dev2_user.id));
    
    // Developer 1 creates task attempt for authentication task
    let attempt1_id = Uuid::new_v4();
    let create_attempt1_data = CreateTaskAttempt {
        task_id: task1.id,
        project_id: project.id,
        branch_name: "feature/authentication".to_string(),
        base_branch: "main".to_string(),
        created_by: Some(dev1_user.id),
    };
    
    let task_attempt1 = TaskAttempt::create(&pool, &create_attempt1_data, attempt1_id).await.unwrap();
    
    // Developer 2 creates task attempt for UI task
    let attempt2_id = Uuid::new_v4();
    let create_attempt2_data = CreateTaskAttempt {
        task_id: task2.id,
        project_id: project.id,
        branch_name: "feature/ui-components".to_string(),
        base_branch: "main".to_string(),
        created_by: Some(dev2_user.id),
    };
    
    let task_attempt2 = TaskAttempt::create(&pool, &create_attempt2_data, attempt2_id).await.unwrap();
    
    // Verify task attempts
    assert_eq!(task_attempt1.created_by, Some(dev1_user.id));
    assert_eq!(task_attempt2.created_by, Some(dev2_user.id));
    assert_eq!(task_attempt1.status, TaskAttemptStatus::InProgress);
    assert_eq!(task_attempt2.status, TaskAttemptStatus::InProgress);
    
    // Verify that users can see their assigned tasks
    let tasks_with_users = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    assert_eq!(tasks_with_users.len(), 2);
    
    // Find dev1's task
    let dev1_task = tasks_with_users.iter()
        .find(|task| task.assignee_username == Some(dev1_user.username.clone()))
        .unwrap();
    assert_eq!(dev1_task.title, "Implement user authentication");
    assert_eq!(dev1_task.creator_username, Some(admin_user.username.clone()));
    
    // Find dev2's task
    let dev2_task = tasks_with_users.iter()
        .find(|task| task.assignee_username == Some(dev2_user.username.clone()))
        .unwrap();
    assert_eq!(dev2_task.title, "Design user interface");
    assert_eq!(dev2_task.creator_username, Some(admin_user.username.clone()));
    
    // Simulate completing task attempts
    let complete_attempt1_data = automagik_forge::models::task_attempt::UpdateTaskAttempt {
        status: Some(TaskAttemptStatus::Completed),
        merge_commit_id: Some("abc123".to_string()),
        completed_at: Some(Utc::now()),
        result_summary: Some("Authentication system implemented successfully".to_string()),
        error_message: None,
        worktree_path: Some("/tmp/auth-worktree".to_string()),
    };
    
    let completed_attempt1 = TaskAttempt::update(&pool, attempt1_id, &complete_attempt1_data).await.unwrap();
    
    // Update task status
    Task::update_status(&pool, task1.id, project.id, TaskStatus::Done).await.unwrap();
    
    // Verify completion
    assert_eq!(completed_attempt1.status, TaskAttemptStatus::Completed);
    assert!(completed_attempt1.completed_at.is_some());
    assert_eq!(completed_attempt1.merge_commit_id, Some("abc123".to_string()));
    
    // Verify task status update
    let updated_task1 = Task::find_by_id(&pool, task1.id).await.unwrap().unwrap();
    assert_eq!(updated_task1.status, TaskStatus::Done);
    
    // Admin can view all project activity
    let all_tasks = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    assert_eq!(all_tasks.len(), 2);
    
    // Check that completed task shows correct status
    let completed_task = all_tasks.iter()
        .find(|task| task.id == task1.id)
        .unwrap();
    assert_eq!(completed_task.status, TaskStatus::Done);
    
    // Verify user sessions are still valid
    let admin_session_check = UserSession::find_valid_by_token_hash(&pool, &admin_session.token_hash).await.unwrap();
    let dev1_session_check = UserSession::find_valid_by_token_hash(&pool, &dev1_session.token_hash).await.unwrap();
    let dev2_session_check = UserSession::find_valid_by_token_hash(&pool, &dev2_session.token_hash).await.unwrap();
    
    assert!(admin_session_check.is_some());
    assert!(dev1_session_check.is_some());
    assert!(dev2_session_check.is_some());
    
    // Test session cleanup for inactive sessions
    let initial_session_count = UserSession::find_all_active(&pool).await.unwrap().len();
    assert_eq!(initial_session_count, 3);
    
    // Create an expired session
    let expired_session_id = Uuid::new_v4();
    let expired_jwt_token = generate_jwt_token(dev1_user.id, expired_session_id, SessionType::Web, &JwtConfig::default()).unwrap();
    let expired_token_hash = hash_token(&expired_jwt_token);
    
    let expired_session_data = CreateUserSession {
        user_id: dev1_user.id,
        token_hash: expired_token_hash,
        session_type: SessionType::Web,
        client_info: Some("Expired Test Session".to_string()),
        expires_at: Utc::now() - Duration::hours(1), // Already expired
    };
    
    UserSession::create(&pool, &expired_session_data, expired_session_id).await.unwrap();
    
    // Verify cleanup removes expired sessions
    let cleaned_count = UserSession::cleanup_expired(&pool).await.unwrap();
    assert_eq!(cleaned_count, 1);
    
    let final_session_count = UserSession::find_all_active(&pool).await.unwrap().len();
    assert_eq!(final_session_count, 3); // Should still have 3 active sessions
}

#[tokio::test]
async fn test_concurrent_multiuser_operations() {
    let pool = setup_test_db().await;
    
    // Create multiple users
    let (user1, _session1, _token1) = create_user_with_session(
        &pool, 111, "user1", "user1@example.com", SessionType::Web
    ).await;
    
    let (user2, _session2, _token2) = create_user_with_session(
        &pool, 222, "user2", "user2@example.com", SessionType::Web
    ).await;
    
    let (user3, _session3, _token3) = create_user_with_session(
        &pool, 333, "user3", "user3@example.com", SessionType::Web
    ).await;
    
    // Create shared project
    let (repo_dir, _repo) = create_test_git_repo(&user1.username, &user1.email);
    let project_id = Uuid::new_v4();
    let create_project_data = CreateProject {
        name: "Concurrent Test Project".to_string(),
        git_repo_path: repo_dir.path().to_string_lossy().to_string(),
        created_by: Some(user1.id),
    };
    
    let project = Project::create(&pool, &create_project_data, project_id).await.unwrap();
    
    // Create concurrent tasks
    let pool1 = pool.clone();
    let pool2 = pool.clone();
    let pool3 = pool.clone();
    
    let project_id1 = project.id;
    let project_id2 = project.id;
    let project_id3 = project.id;
    
    let user1_id = user1.id;
    let user2_id = user2.id;
    let user3_id = user3.id;
    
    let task1 = tokio::spawn(async move {
        let task_id = Uuid::new_v4();
        let create_data = CreateTask {
            project_id: project_id1,
            title: "Concurrent Task 1".to_string(),
            description: Some("Task created by user1".to_string()),
            wish_id: "concurrent-1".to_string(),
            parent_task_attempt: None,
            created_by: Some(user1_id),
            assigned_to: Some(user1_id),
        };
        
        Task::create(&pool1, &create_data, task_id).await
    });
    
    let task2 = tokio::spawn(async move {
        let task_id = Uuid::new_v4();
        let create_data = CreateTask {
            project_id: project_id2,
            title: "Concurrent Task 2".to_string(),
            description: Some("Task created by user2".to_string()),
            wish_id: "concurrent-2".to_string(),
            parent_task_attempt: None,
            created_by: Some(user2_id),
            assigned_to: Some(user2_id),
        };
        
        Task::create(&pool2, &create_data, task_id).await
    });
    
    let task3 = tokio::spawn(async move {
        let task_id = Uuid::new_v4();
        let create_data = CreateTask {
            project_id: project_id3,
            title: "Concurrent Task 3".to_string(),
            description: Some("Task created by user3".to_string()),
            wish_id: "concurrent-3".to_string(),
            parent_task_attempt: None,
            created_by: Some(user3_id),
            assigned_to: Some(user3_id),
        };
        
        Task::create(&pool3, &create_data, task_id).await
    });
    
    // Wait for all tasks to complete
    let result1 = task1.await.unwrap();
    let result2 = task2.await.unwrap();
    let result3 = task3.await.unwrap();
    
    // All tasks should succeed
    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());
    
    // Verify all tasks exist
    let tasks = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    assert_eq!(tasks.len(), 3);
    
    // Verify each user created their task
    let user1_tasks: Vec<_> = tasks.iter()
        .filter(|task| task.creator_username == Some(user1.username.clone()))
        .collect();
    let user2_tasks: Vec<_> = tasks.iter()
        .filter(|task| task.creator_username == Some(user2.username.clone()))
        .collect();
    let user3_tasks: Vec<_> = tasks.iter()
        .filter(|task| task.creator_username == Some(user3.username.clone()))
        .collect();
    
    assert_eq!(user1_tasks.len(), 1);
    assert_eq!(user2_tasks.len(), 1);
    assert_eq!(user3_tasks.len(), 1);
}

#[tokio::test]
async fn test_user_permission_enforcement() {
    let pool = setup_test_db().await;
    
    // Create admin and regular users
    let (admin_user, _admin_session, _admin_token) = create_user_with_session(
        &pool, 111, "admin", "admin@example.com", SessionType::Web
    ).await;
    
    // Make user admin
    let admin_update = UpdateUser {
        username: None,
        email: None,
        display_name: None,
        avatar_url: None,
        github_token: None,
        is_admin: Some(true),
        is_whitelisted: None,
    };
    let admin_user = User::update(&pool, admin_user.id, &admin_update).await.unwrap();
    
    let (regular_user, _regular_session, _regular_token) = create_user_with_session(
        &pool, 222, "regular", "regular@example.com", SessionType::Web
    ).await;
    
    let (non_whitelisted_user, _nw_session, _nw_token) = create_user_with_session(
        &pool, 333, "nonwhitelisted", "nw@example.com", SessionType::Web
    ).await;
    
    // Make user non-whitelisted
    let nw_update = UpdateUser {
        username: None,
        email: None,
        display_name: None,
        avatar_url: None,
        github_token: None,
        is_admin: None,
        is_whitelisted: Some(false),
    };
    let non_whitelisted_user = User::update(&pool, non_whitelisted_user.id, &nw_update).await.unwrap();
    
    // Verify admin privileges
    let admins = User::find_admins(&pool).await.unwrap();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins[0].id, admin_user.id);
    assert!(admins[0].is_admin);
    
    // Verify whitelist status
    assert!(User::is_github_id_whitelisted(&pool, admin_user.github_id).await.unwrap());
    assert!(User::is_github_id_whitelisted(&pool, regular_user.github_id).await.unwrap());
    assert!(!User::is_github_id_whitelisted(&pool, non_whitelisted_user.github_id).await.unwrap());
    
    // Create project (only whitelisted users should be able to create)
    let (repo_dir, _repo) = create_test_git_repo(&admin_user.username, &admin_user.email);
    let project_id = Uuid::new_v4();
    let create_project_data = CreateProject {
        name: "Permission Test Project".to_string(),
        git_repo_path: repo_dir.path().to_string_lossy().to_string(),
        created_by: Some(admin_user.id),
    };
    
    let project = Project::create(&pool, &create_project_data, project_id).await.unwrap();
    
    // Regular user should be able to create tasks (whitelisted)
    let task_id = Uuid::new_v4();
    let create_task_data = CreateTask {
        project_id: project.id,
        title: "Regular User Task".to_string(),
        description: Some("Task by regular user".to_string()),
        wish_id: "regular-task".to_string(),
        parent_task_attempt: None,
        created_by: Some(regular_user.id),
        assigned_to: Some(regular_user.id),
    };
    
    let task = Task::create(&pool, &create_task_data, task_id).await.unwrap();
    assert_eq!(task.created_by, Some(regular_user.id));
    
    // Test task assignment changes
    // Regular user can be assigned tasks
    let updated_task = Task::update(
        &pool,
        task.id,
        project.id,
        "Updated Task Title".to_string(),
        task.description,
        task.status,
        task.wish_id,
        task.parent_task_attempt,
        Some(admin_user.id), // Reassign to admin
    ).await.unwrap();
    
    assert_eq!(updated_task.assigned_to, Some(admin_user.id));
    
    // Test session management for different user types
    let admin_sessions = UserSession::find_by_user_id(&pool, admin_user.id).await.unwrap();
    let regular_sessions = UserSession::find_by_user_id(&pool, regular_user.id).await.unwrap();
    let nw_sessions = UserSession::find_by_user_id(&pool, non_whitelisted_user.id).await.unwrap();
    
    assert_eq!(admin_sessions.len(), 1);
    assert_eq!(regular_sessions.len(), 1);
    assert_eq!(nw_sessions.len(), 1); // Session exists but user not whitelisted
    
    // Count sessions by type
    let web_session_count = UserSession::count_active_by_user_and_type(
        &pool, regular_user.id, SessionType::Web
    ).await.unwrap();
    assert_eq!(web_session_count, 1);
}

#[tokio::test]
async fn test_multiuser_git_workflow_integration() {
    let pool = setup_test_db().await;
    
    // Create users
    let (maintainer, _m_session, _m_token) = create_user_with_session(
        &pool, 111, "maintainer", "maintainer@example.com", SessionType::Web
    ).await;
    
    let (contributor, _c_session, _c_token) = create_user_with_session(
        &pool, 222, "contributor", "contributor@example.com", SessionType::Web
    ).await;
    
    // Create Git repository and project
    let (repo_dir, repo) = create_test_git_repo(&maintainer.username, &maintainer.email);
    
    // Add some initial content
    let readme_path = repo_dir.path().join("README.md");
    std::fs::write(&readme_path, "# Test Project\nInitial content").unwrap();
    
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("README.md")).unwrap();
    index.write().unwrap();
    
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let head = repo.head().unwrap();
    let parent_commit = head.peel_to_commit().unwrap();
    let signature = git2::Signature::now(&maintainer.username, &maintainer.email).unwrap();
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add initial README",
        &tree,
        &[&parent_commit],
    ).unwrap();
    
    let project_id = Uuid::new_v4();
    let create_project_data = CreateProject {
        name: "Git Integration Test".to_string(),
        git_repo_path: repo_dir.path().to_string_lossy().to_string(),
        created_by: Some(maintainer.id),
    };
    
    let project = Project::create(&pool, &create_project_data, project_id).await.unwrap();
    
    // Create task assigned to contributor
    let task_id = Uuid::new_v4();
    let create_task_data = CreateTask {
        project_id: project.id,
        title: "Add feature documentation".to_string(),
        description: Some("Document the new feature in README".to_string()),
        wish_id: "doc-feature".to_string(),
        parent_task_attempt: None,
        created_by: Some(maintainer.id),
        assigned_to: Some(contributor.id),
    };
    
    let task = Task::create(&pool, &create_task_data, task_id).await.unwrap();
    
    // Create task attempt
    let attempt_id = Uuid::new_v4();
    let create_attempt_data = CreateTaskAttempt {
        task_id: task.id,
        project_id: project.id,
        branch_name: "feature/documentation".to_string(),
        base_branch: "main".to_string(),
        created_by: Some(contributor.id),
    };
    
    let task_attempt = TaskAttempt::create(&pool, &create_attempt_data, attempt_id).await.unwrap();
    
    // Initialize Git service
    let git_service = GitService::new(&project.git_repo_path).unwrap();
    
    // Test Git operations
    let default_branch = git_service.get_default_branch_name().unwrap();
    assert_eq!(default_branch, "main");
    
    // Verify task attempt was created with correct user attribution
    assert_eq!(task_attempt.created_by, Some(contributor.id));
    assert_eq!(task_attempt.branch_name, "feature/documentation");
    assert_eq!(task_attempt.base_branch, "main");
    
    // Verify task assignment and ownership
    let tasks_with_users = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    assert_eq!(tasks_with_users.len(), 1);
    
    let task_with_users = &tasks_with_users[0];
    assert_eq!(task_with_users.creator_username, Some(maintainer.username.clone()));
    assert_eq!(task_with_users.assignee_username, Some(contributor.username.clone()));
    assert_eq!(task_with_users.creator_display_name, Some(format!("User {}", maintainer.username)));
    assert_eq!(task_with_users.assignee_display_name, Some(format!("User {}", contributor.username)));
    
    // Test completing the workflow
    let complete_attempt_data = automagik_forge::models::task_attempt::UpdateTaskAttempt {
        status: Some(TaskAttemptStatus::Completed),
        merge_commit_id: Some("feature_commit_123".to_string()),
        completed_at: Some(Utc::now()),
        result_summary: Some("Documentation updated successfully".to_string()),
        error_message: None,
        worktree_path: Some("/tmp/feature-worktree".to_string()),
    };
    
    let completed_attempt = TaskAttempt::update(&pool, attempt_id, &complete_attempt_data).await.unwrap();
    
    // Update task status
    Task::update_status(&pool, task.id, project.id, TaskStatus::Done).await.unwrap();
    
    // Verify completion with user context
    assert_eq!(completed_attempt.status, TaskAttemptStatus::Completed);
    assert_eq!(completed_attempt.merge_commit_id, Some("feature_commit_123".to_string()));
    assert_eq!(completed_attempt.created_by, Some(contributor.id));
    
    let final_task = Task::find_by_id(&pool, task.id).await.unwrap().unwrap();
    assert_eq!(final_task.status, TaskStatus::Done);
    assert_eq!(final_task.assigned_to, Some(contributor.id));
    assert_eq!(final_task.created_by, Some(maintainer.id));
}

#[tokio::test]
async fn test_session_management_across_multiple_clients() {
    let pool = setup_test_db().await;
    
    // Create user with multiple session types
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id: 123456,
        username: "multidevice".to_string(),
        email: "multidevice@example.com".to_string(),
        display_name: Some("Multi Device User".to_string()),
        avatar_url: Some("https://github.com/multidevice.png".to_string()),
        github_token: Some("encrypted_token".to_string()),
        is_admin: Some(false),
    };
    
    let user = User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Create multiple sessions for different clients
    let jwt_config = JwtConfig::default();
    
    // Web session (browser)
    let web_session_id = Uuid::new_v4();
    let web_token = generate_jwt_token(user.id, web_session_id, SessionType::Web, &jwt_config).unwrap();
    let web_token_hash = hash_token(&web_token);
    
    let web_session = UserSession::create_with_defaults(
        &pool,
        user.id,
        web_token_hash,
        SessionType::Web,
        Some("Web Browser - Chrome".to_string()),
    ).await.unwrap();
    
    // MCP session (Claude Desktop)
    let mcp_session_id = Uuid::new_v4();
    let mcp_token = generate_jwt_token(user.id, mcp_session_id, SessionType::Mcp, &jwt_config).unwrap();
    let mcp_token_hash = hash_token(&mcp_token);
    
    let mcp_session = UserSession::create_with_defaults(
        &pool,
        user.id,
        mcp_token_hash,
        SessionType::Mcp,
        Some("Claude Desktop".to_string()),
    ).await.unwrap();
    
    // Second web session (mobile browser)
    let mobile_session_id = Uuid::new_v4();
    let mobile_token = generate_jwt_token(user.id, mobile_session_id, SessionType::Web, &jwt_config).unwrap();
    let mobile_token_hash = hash_token(&mobile_token);
    
    let mobile_session = UserSession::create_with_defaults(
        &pool,
        user.id,
        mobile_token_hash,
        SessionType::Web,
        Some("Mobile Browser - Safari".to_string()),
    ).await.unwrap();
    
    // Verify all sessions exist
    let user_sessions = UserSession::find_by_user_id(&pool, user.id).await.unwrap();
    assert_eq!(user_sessions.len(), 3);
    
    // Verify session types and expiration times
    let web_sessions: Vec<_> = user_sessions.iter()
        .filter(|s| s.session_type == SessionType::Web)
        .collect();
    let mcp_sessions: Vec<_> = user_sessions.iter()
        .filter(|s| s.session_type == SessionType::Mcp)
        .collect();
    
    assert_eq!(web_sessions.len(), 2);
    assert_eq!(mcp_sessions.len(), 1);
    
    // MCP session should have longer expiration
    let web_session_expires = web_sessions[0].expires_at;
    let mcp_session_expires = mcp_sessions[0].expires_at;
    assert!(mcp_session_expires > web_session_expires);
    
    // Test session counting by type
    let web_count = UserSession::count_active_by_user_and_type(
        &pool, user.id, SessionType::Web
    ).await.unwrap();
    let mcp_count = UserSession::count_active_by_user_and_type(
        &pool, user.id, SessionType::Mcp
    ).await.unwrap();
    
    assert_eq!(web_count, 2);
    assert_eq!(mcp_count, 1);
    
    // Test session cleanup - all should be active
    let active_sessions_before = UserSession::find_all_active(&pool).await.unwrap();
    assert_eq!(active_sessions_before.len(), 3);
    
    let cleaned_before = UserSession::cleanup_expired(&pool).await.unwrap();
    assert_eq!(cleaned_before, 0); // No expired sessions
    
    // Expire one web session manually
    sqlx::query!(
        "UPDATE user_sessions SET expires_at = ? WHERE id = ?",
        Utc::now() - Duration::hours(1),
        web_session.id.to_string()
    )
    .execute(&pool)
    .await
    .unwrap();
    
    // Clean up expired sessions
    let cleaned_after = UserSession::cleanup_expired(&pool).await.unwrap();
    assert_eq!(cleaned_after, 1);
    
    // Verify remaining sessions
    let remaining_sessions = UserSession::find_by_user_id(&pool, user.id).await.unwrap();
    assert_eq!(remaining_sessions.len(), 2);
    
    let active_sessions_after = UserSession::find_all_active(&pool).await.unwrap();
    assert_eq!(active_sessions_after.len(), 2);
    
    // Test deleting all sessions for user (logout from all devices)
    let deleted_count = UserSession::delete_all_for_user(&pool, user.id).await.unwrap();
    assert_eq!(deleted_count, 2);
    
    let final_sessions = UserSession::find_by_user_id(&pool, user.id).await.unwrap();
    assert_eq!(final_sessions.len(), 0);
}

#[tokio::test]
async fn test_complete_oauth_to_task_completion_workflow() {
    let pool = setup_test_db().await;
    
    // Simulate OAuth user creation (user coming from GitHub OAuth)
    let github_id = 987654321;
    let username = "githubuser";
    let email = "githubuser@example.com";
    
    // Add user to whitelist (simulating environment variable or database whitelist)
    sqlx::query!(
        "INSERT INTO github_whitelist (id, github_username, github_id, is_active) VALUES (?, ?, ?, ?)",
        Uuid::new_v4().to_string(),
        username,
        github_id,
        true
    )
    .execute(&pool)
    .await
    .unwrap();
    
    // Verify user is whitelisted
    let is_whitelisted = User::is_github_id_whitelisted(&pool, github_id).await.unwrap();
    assert!(is_whitelisted);
    
    // Create user (as would happen during OAuth callback)
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id,
        username: username.to_string(),
        email: email.to_string(),
        display_name: Some("GitHub User".to_string()),
        avatar_url: Some(format!("https://github.com/{}.png", username)),
        github_token: Some("oauth_github_token_encrypted".to_string()),
        is_admin: Some(false),
    };
    
    let user = User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Create session (as would happen after successful OAuth)
    let session_id = Uuid::new_v4();
    let jwt_config = JwtConfig::default();
    let jwt_token = generate_jwt_token(user.id, session_id, SessionType::Web, &jwt_config).unwrap();
    let token_hash = hash_token(&jwt_token);
    
    let session_data = CreateUserSession {
        user_id: user.id,
        token_hash,
        session_type: SessionType::Web,
        client_info: Some("OAuth Web Client".to_string()),
        expires_at: Utc::now() + Duration::hours(UserSession::WEB_SESSION_DURATION_HOURS),
    };
    
    let session = UserSession::create(&pool, &session_data, session_id).await.unwrap();
    
    // Update last login
    User::update_last_login(&pool, user.id).await.unwrap();
    
    let updated_user = User::find_by_id(&pool, user.id).await.unwrap().unwrap();
    assert!(updated_user.last_login_at.is_some());
    
    // User creates a project
    let (repo_dir, _repo) = create_test_git_repo(&user.username, &user.email);
    let project_id = Uuid::new_v4();
    let create_project_data = CreateProject {
        name: "OAuth User Project".to_string(),
        git_repo_path: repo_dir.path().to_string_lossy().to_string(),
        created_by: Some(user.id),
    };
    
    let project = Project::create(&pool, &create_project_data, project_id).await.unwrap();
    
    // User creates and self-assigns a task
    let task_id = Uuid::new_v4();
    let create_task_data = CreateTask {
        project_id: project.id,
        title: "Setup project structure".to_string(),
        description: Some("Initialize project with basic structure".to_string()),
        wish_id: "setup-project".to_string(),
        parent_task_attempt: None,
        created_by: Some(user.id),
        assigned_to: Some(user.id),
    };
    
    let task = Task::create(&pool, &create_task_data, task_id).await.unwrap();
    
    // User creates task attempt
    let attempt_id = Uuid::new_v4();
    let create_attempt_data = CreateTaskAttempt {
        task_id: task.id,
        project_id: project.id,
        branch_name: "setup/project-structure".to_string(),
        base_branch: "main".to_string(),
        created_by: Some(user.id),
    };
    
    let task_attempt = TaskAttempt::create(&pool, &create_attempt_data, attempt_id).await.unwrap();
    
    // Complete the task
    let complete_data = automagik_forge::models::task_attempt::UpdateTaskAttempt {
        status: Some(TaskAttemptStatus::Completed),
        merge_commit_id: Some("setup_commit_456".to_string()),
        completed_at: Some(Utc::now()),
        result_summary: Some("Project structure setup completed".to_string()),
        error_message: None,
        worktree_path: Some("/tmp/setup-worktree".to_string()),
    };
    
    let completed_attempt = TaskAttempt::update(&pool, attempt_id, &complete_data).await.unwrap();
    Task::update_status(&pool, task.id, project.id, TaskStatus::Done).await.unwrap();
    
    // Verify the complete workflow
    assert_eq!(user.github_id, github_id);
    assert!(user.is_whitelisted);
    assert!(!user.is_admin);
    assert!(updated_user.last_login_at.is_some());
    
    assert_eq!(session.user_id, user.id);
    assert_eq!(session.session_type, SessionType::Web);
    assert!(session.is_valid());
    
    assert_eq!(project.created_by, Some(user.id));
    assert_eq!(task.created_by, Some(user.id));
    assert_eq!(task.assigned_to, Some(user.id));
    
    assert_eq!(task_attempt.created_by, Some(user.id));
    assert_eq!(completed_attempt.status, TaskAttemptStatus::Completed);
    
    let final_task = Task::find_by_id(&pool, task.id).await.unwrap().unwrap();
    assert_eq!(final_task.status, TaskStatus::Done);
    
    // Verify user can see their work
    let user_tasks = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    assert_eq!(user_tasks.len(), 1);
    
    let user_task = &user_tasks[0];
    assert_eq!(user_task.creator_username, Some(user.username.clone()));
    assert_eq!(user_task.assignee_username, Some(user.username.clone()));
    assert_eq!(user_task.status, TaskStatus::Done);
}