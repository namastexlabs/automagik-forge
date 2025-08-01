use automagik_forge::{
    models::{
        user::{User, CreateUser},
        project::{Project, CreateProject},
        task::{Task, CreateTask, TaskStatus},
        task_attempt::{TaskAttempt, CreateTaskAttempt, TaskAttemptStatus},
    },
    services::git_service::{GitService, GitServiceError},
};
use chrono::Utc;
use git2::{Repository, Signature, BranchType, ObjectType};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
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

/// Helper function to create a test user with specific Git config
async fn create_test_user_with_git_config(
    pool: &SqlitePool, 
    github_id: i64, 
    username: &str,
    email: &str,
    display_name: &str
) -> User {
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id,
        username: username.to_string(),
        email: email.to_string(),
        display_name: Some(display_name.to_string()),
        avatar_url: Some(format!("https://github.com/{}.png", username)),
        github_token: Some("encrypted_token".to_string()),
        is_admin: Some(false),
    };
    
    User::create(pool, &create_data, user_id).await.unwrap()
}

/// Helper function to create and configure a test Git repository
fn create_test_git_repo(user_name: &str, user_email: &str) -> (TempDir, Repository) {
    let temp_dir = TempDir::new().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();

    // Configure the repository with user information
    let mut config = repo.config().unwrap();
    config.set_str("user.name", user_name).unwrap();
    config.set_str("user.email", user_email).unwrap();
    config.set_str("init.defaultBranch", "main").unwrap();

    (temp_dir, repo)
}

/// Helper function to create initial commit with specific author
fn create_initial_commit(repo: &Repository, author_name: &str, author_email: &str, message: &str) -> git2::Oid {
    let signature = Signature::now(author_name, author_email).unwrap();
    
    // Create empty tree
    let tree_id = {
        let tree_builder = repo.treebuilder(None).unwrap();
        tree_builder.write().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();

    // Create initial commit
    let commit_id = repo.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        message,
        &tree,
        &[],
    ).unwrap();

    // Set HEAD to point to main branch
    repo.set_head("refs/heads/main").unwrap();

    commit_id
}

/// Helper function to create a file and commit it
fn create_file_and_commit(
    repo: &Repository, 
    repo_path: &Path, 
    filename: &str, 
    content: &str, 
    author_name: &str, 
    author_email: &str,
    message: &str
) -> git2::Oid {
    let file_path = repo_path.join(filename);
    std::fs::write(&file_path, content).unwrap();

    let signature = Signature::now(author_name, author_email).unwrap();
    
    // Stage the file
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(filename)).unwrap();
    index.write().unwrap();
    
    // Create commit
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let head = repo.head().unwrap();
    let parent_commit = head.peel_to_commit().unwrap();
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&parent_commit],
    ).unwrap()
}

#[tokio::test]
async fn test_git_service_creation_and_validation() {
    let (temp_dir, _repo) = create_test_git_repo("Test User", "test@example.com");
    
    // Valid repository should work
    let git_service = GitService::new(temp_dir.path()).unwrap();
    assert!(git_service.get_default_branch_name().is_ok());
    
    // Invalid path should fail
    let invalid_result = GitService::new("/nonexistent/path");
    assert!(invalid_result.is_err());
    match invalid_result.unwrap_err() {
        GitServiceError::InvalidPath(_) => {},
        _ => panic!("Expected InvalidPath error"),
    }
    
    // Non-git directory should fail
    let non_git_dir = TempDir::new().unwrap();
    let non_git_result = GitService::new(non_git_dir.path());
    assert!(non_git_result.is_err());
    match non_git_result.unwrap_err() {
        GitServiceError::InvalidRepository(_) => {},
        _ => panic!("Expected InvalidRepository error"),
    }
}

#[tokio::test]
async fn test_worktree_creation_with_user_attribution() {
    let pool = setup_test_db().await;
    
    // Create test users
    let creator = create_test_user_with_git_config(
        &pool, 111, "creator", "creator@example.com", "Creator User"
    ).await;
    let assignee = create_test_user_with_git_config(
        &pool, 222, "assignee", "assignee@example.com", "Assignee User"
    ).await;
    
    // Create test repository with creator's credentials
    let (main_repo_dir, main_repo) = create_test_git_repo(&creator.username, &creator.email);
    create_initial_commit(&main_repo, &creator.username, &creator.email, "Initial commit by creator");
    
    let git_service = GitService::new(main_repo_dir.path()).unwrap();
    
    // Create worktree for a task
    let worktree_dir = TempDir::new().unwrap();
    let worktree_path = worktree_dir.path().join("task-branch");
    let branch_name = "task/feature-123";
    
    git_service.create_worktree(branch_name, &worktree_path, None).unwrap();
    
    // Verify worktree was created
    assert!(worktree_path.exists());
    let worktree_repo = Repository::open(&worktree_path).unwrap();
    
    // Check that the worktree has the correct branch
    let head = worktree_repo.head().unwrap();
    assert_eq!(head.shorthand().unwrap(), branch_name);
    
    // Verify branch exists in main repo
    let branch = main_repo.find_branch(branch_name, BranchType::Local).unwrap();
    assert!(branch.is_head());
}

#[tokio::test]
async fn test_commit_with_user_attribution() {
    let pool = setup_test_db().await;
    
    // Create test users with different Git configurations
    let creator = create_test_user_with_git_config(
        &pool, 111, "creator", "creator@example.com", "Creator User"
    ).await;
    let contributor = create_test_user_with_git_config(
        &pool, 222, "contributor", "contributor@example.com", "Contributor User"
    ).await;
    
    // Create test repository
    let (main_repo_dir, main_repo) = create_test_git_repo(&creator.username, &creator.email);
    create_initial_commit(&main_repo, &creator.username, &creator.email, "Initial commit");
    
    let git_service = GitService::new(main_repo_dir.path()).unwrap();
    
    // Create worktree and make changes as different user
    let worktree_dir = TempDir::new().unwrap();
    let worktree_path = worktree_dir.path().join("contributor-work");
    let branch_name = "task/contributor-feature";
    
    git_service.create_worktree(branch_name, &worktree_path, None).unwrap();
    
    // Configure worktree with contributor's information
    let worktree_repo = Repository::open(&worktree_path).unwrap();
    let mut config = worktree_repo.config().unwrap();
    config.set_str("user.name", &contributor.username).unwrap();
    config.set_str("user.email", &contributor.email).unwrap();
    
    // Create and commit file as contributor
    let commit_id = create_file_and_commit(
        &worktree_repo,
        &worktree_path,
        "feature.txt",
        "Feature implementation by contributor",
        &contributor.username,
        &contributor.email,
        "Add feature implementation"
    );
    
    // Verify commit has correct author
    let commit = worktree_repo.find_commit(commit_id).unwrap();
    let author = commit.author();
    assert_eq!(author.name().unwrap(), contributor.username);
    assert_eq!(author.email().unwrap(), contributor.email);
    assert_eq!(commit.message().unwrap(), "Add feature implementation");
    
    // Verify commit is in the correct branch
    let head = worktree_repo.head().unwrap();
    assert_eq!(head.target().unwrap(), commit_id);
    assert_eq!(head.shorthand().unwrap(), branch_name);
}

#[tokio::test]
async fn test_merge_changes_with_user_attribution() {
    let pool = setup_test_db().await;
    
    // Create test users
    let creator = create_test_user_with_git_config(
        &pool, 111, "creator", "creator@example.com", "Project Creator"
    ).await;
    let developer = create_test_user_with_git_config(
        &pool, 222, "developer", "dev@example.com", "Feature Developer"
    ).await;
    
    // Create main repository
    let (main_repo_dir, main_repo) = create_test_git_repo(&creator.username, &creator.email);
    create_initial_commit(&main_repo, &creator.username, &creator.email, "Project initialization");
    
    // Add some base content
    create_file_and_commit(
        &main_repo,
        main_repo_dir.path(),
        "README.md",
        "# Project README\nInitial content",
        &creator.username,
        &creator.email,
        "Add initial README"
    );
    
    let git_service = GitService::new(main_repo_dir.path()).unwrap();
    
    // Create feature branch and worktree
    let worktree_dir = TempDir::new().unwrap();
    let worktree_path = worktree_dir.path().join("feature-work");
    let branch_name = "feature/user-auth";
    
    git_service.create_worktree(branch_name, &worktree_path, Some("main")).unwrap();
    
    // Configure worktree with developer's credentials
    let worktree_repo = Repository::open(&worktree_path).unwrap();
    let mut config = worktree_repo.config().unwrap();
    config.set_str("user.name", &developer.username).unwrap();
    config.set_str("user.email", &developer.email).unwrap();
    
    // Make multiple commits as developer
    create_file_and_commit(
        &worktree_repo,
        &worktree_path,
        "auth.rs",
        "pub fn authenticate(user: &str) -> bool { true }",
        &developer.username,
        &developer.email,
        "Add authentication module"
    );
    
    create_file_and_commit(
        &worktree_repo,
        &worktree_path,
        "user.rs",
        "pub struct User { name: String }",
        &developer.username,
        &developer.email,
        "Add user model"
    );
    
    // Merge changes back to main
    let commit_message = format!(
        "Implement user authentication system\n\nContributed by: {} <{}>", 
        developer.display_name.as_ref().unwrap(), 
        developer.email
    );
    
    let merge_commit_id = git_service.merge_changes(
        &worktree_path,
        branch_name,
        "main",
        &commit_message
    ).unwrap();
    
    // Verify merge commit exists and has correct attribution
    let merge_commit = main_repo.find_commit(git2::Oid::from_str(&merge_commit_id).unwrap()).unwrap();
    
    // Merge commit should be authored by the developer (the one who made the changes)
    let author = merge_commit.author();
    assert_eq!(author.name().unwrap(), developer.username);
    assert_eq!(author.email().unwrap(), developer.email);
    assert!(merge_commit.message().unwrap().contains("user authentication system"));
    assert!(merge_commit.message().unwrap().contains(&developer.display_name.as_ref().unwrap()));
    
    // Verify files were merged
    let head_commit = main_repo.head().unwrap().peel_to_commit().unwrap();
    let tree = head_commit.tree().unwrap();
    assert!(tree.get_path(Path::new("auth.rs")).is_ok());
    assert!(tree.get_path(Path::new("user.rs")).is_ok());
    assert!(tree.get_path(Path::new("README.md")).is_ok());
}

#[tokio::test]
async fn test_branch_management_with_multiple_users() {
    let pool = setup_test_db().await;
    
    // Create test users
    let maintainer = create_test_user_with_git_config(
        &pool, 111, "maintainer", "maintainer@example.com", "Project Maintainer"
    ).await;
    let dev1 = create_test_user_with_git_config(
        &pool, 222, "dev1", "dev1@example.com", "Developer One"
    ).await;
    let dev2 = create_test_user_with_git_config(
        &pool, 333, "dev2", "dev2@example.com", "Developer Two"
    ).await;
    
    // Create main repository
    let (main_repo_dir, main_repo) = create_test_git_repo(&maintainer.username, &maintainer.email);
    create_initial_commit(&main_repo, &maintainer.username, &maintainer.email, "Initial commit");
    
    let git_service = GitService::new(main_repo_dir.path()).unwrap();
    
    // Create multiple feature branches for different users
    let worktree_dir1 = TempDir::new().unwrap();
    let worktree_path1 = worktree_dir1.path().join("feature1");
    let branch_name1 = "feature/component-a";
    
    let worktree_dir2 = TempDir::new().unwrap();
    let worktree_path2 = worktree_dir2.path().join("feature2");
    let branch_name2 = "feature/component-b";
    
    // Create worktrees for each developer
    git_service.create_worktree(branch_name1, &worktree_path1, None).unwrap();
    git_service.create_worktree(branch_name2, &worktree_path2, None).unwrap();
    
    // Configure each worktree with respective developer credentials
    let worktree_repo1 = Repository::open(&worktree_path1).unwrap();
    let mut config1 = worktree_repo1.config().unwrap();
    config1.set_str("user.name", &dev1.username).unwrap();
    config1.set_str("user.email", &dev1.email).unwrap();
    
    let worktree_repo2 = Repository::open(&worktree_path2).unwrap();
    let mut config2 = worktree_repo2.config().unwrap();
    config2.set_str("user.name", &dev2.username).unwrap();
    config2.set_str("user.email", &dev2.email).unwrap();
    
    // Each developer makes changes
    create_file_and_commit(
        &worktree_repo1,
        &worktree_path1,
        "component_a.rs",
        "// Component A implementation",
        &dev1.username,
        &dev1.email,
        "Implement component A"
    );
    
    create_file_and_commit(
        &worktree_repo2,
        &worktree_path2,
        "component_b.rs",
        "// Component B implementation", 
        &dev2.username,
        &dev2.email,
        "Implement component B"
    );
    
    // Verify both branches exist in main repo
    assert!(main_repo.find_branch(branch_name1, BranchType::Local).is_ok());
    assert!(main_repo.find_branch(branch_name2, BranchType::Local).is_ok());
    
    // Merge both features (first dev1, then dev2)
    let merge_commit_id1 = git_service.merge_changes(
        &worktree_path1,
        branch_name1,
        "main",
        &format!("Merge component A by {}", dev1.username)
    ).unwrap();
    
    let merge_commit_id2 = git_service.merge_changes(
        &worktree_path2,
        branch_name2,
        "main",
        &format!("Merge component B by {}", dev2.username)
    ).unwrap();
    
    // Verify commit history shows correct authors
    let merge_commit1 = main_repo.find_commit(git2::Oid::from_str(&merge_commit_id1).unwrap()).unwrap();
    let merge_commit2 = main_repo.find_commit(git2::Oid::from_str(&merge_commit_id2).unwrap()).unwrap();
    
    assert_eq!(merge_commit1.author().name().unwrap(), dev1.username);
    assert_eq!(merge_commit2.author().name().unwrap(), dev2.username);
    
    // Verify both components are present in final state
    let head_commit = main_repo.head().unwrap().peel_to_commit().unwrap();
    let tree = head_commit.tree().unwrap();
    assert!(tree.get_path(Path::new("component_a.rs")).is_ok());
    assert!(tree.get_path(Path::new("component_b.rs")).is_ok());
}

#[tokio::test]
async fn test_git_diff_with_user_context() {
    let pool = setup_test_db().await;
    
    // Create test users
    let author = create_test_user_with_git_config(
        &pool, 111, "author", "author@example.com", "Code Author"
    ).await;
    
    // Create repository and worktree
    let (main_repo_dir, main_repo) = create_test_git_repo(&author.username, &author.email);
    create_initial_commit(&main_repo, &author.username, &author.email, "Initial commit");
    
    // Add base file
    create_file_and_commit(
        &main_repo,
        main_repo_dir.path(),
        "app.rs",
        "fn main() {\n    println!(\"Hello, world!\");\n}",
        &author.username,
        &author.email,
        "Add initial app"
    );
    
    let git_service = GitService::new(main_repo_dir.path()).unwrap();
    
    // Create feature branch
    let worktree_dir = TempDir::new().unwrap();
    let worktree_path = worktree_dir.path().join("feature");
    let branch_name = "feature/enhanced-greeting";
    
    git_service.create_worktree(branch_name, &worktree_path, Some("main")).unwrap();
    
    // Configure worktree
    let worktree_repo = Repository::open(&worktree_path).unwrap();
    let mut config = worktree_repo.config().unwrap();
    config.set_str("user.name", &author.username).unwrap();
    config.set_str("user.email", &author.email).unwrap();
    
    // Modify file
    let modified_content = "fn main() {\n    let name = \"Automagik Forge\";\n    println!(\"Hello, {}!\", name);\n}";
    create_file_and_commit(
        &worktree_repo,
        &worktree_path,
        "app.rs",
        modified_content,
        &author.username,
        &author.email,
        "Enhance greeting with dynamic name"
    );
    
    // Get enhanced diff
    let diff = git_service.get_enhanced_diff(&worktree_path, None, "main").unwrap();
    
    // Verify diff contains expected changes
    assert_eq!(diff.files.len(), 1);
    let file_diff = &diff.files[0];
    assert_eq!(file_diff.path, "app.rs");
    
    // Check that diff chunks contain the changes
    let has_insert = file_diff.chunks.iter().any(|chunk| {
        chunk.chunk_type == automagik_forge::models::task_attempt::DiffChunkType::Insert
            && chunk.content.contains("let name = \"Automagik Forge\"")
    });
    assert!(has_insert, "Diff should contain the inserted line with dynamic name");
}

#[tokio::test]
async fn test_merge_conflict_handling() {
    let pool = setup_test_db().await;
    
    // Create test users
    let dev1 = create_test_user_with_git_config(
        &pool, 111, "dev1", "dev1@example.com", "Developer One"
    ).await;
    let dev2 = create_test_user_with_git_config(
        &pool, 222, "dev2", "dev2@example.com", "Developer Two"
    ).await;
    
    // Create main repository with shared file
    let (main_repo_dir, main_repo) = create_test_git_repo(&dev1.username, &dev1.email);
    create_initial_commit(&main_repo, &dev1.username, &dev1.email, "Initial commit");
    
    // Create shared file that both devs will modify
    create_file_and_commit(
        &main_repo,
        main_repo_dir.path(),
        "config.rs",
        "pub const VERSION: &str = \"1.0.0\";",
        &dev1.username,
        &dev1.email,
        "Add version config"
    );
    
    let git_service = GitService::new(main_repo_dir.path()).unwrap();
    
    // Dev1 creates feature branch and modifies file
    let worktree_dir1 = TempDir::new().unwrap();
    let worktree_path1 = worktree_dir1.path().join("feature1");
    let branch_name1 = "feature/version-update-1";
    
    git_service.create_worktree(branch_name1, &worktree_path1, Some("main")).unwrap();
    
    let worktree_repo1 = Repository::open(&worktree_path1).unwrap();
    let mut config1 = worktree_repo1.config().unwrap();
    config1.set_str("user.name", &dev1.username).unwrap();
    config1.set_str("user.email", &dev1.email).unwrap();
    
    create_file_and_commit(
        &worktree_repo1,
        &worktree_path1,
        "config.rs",
        "pub const VERSION: &str = \"1.1.0\";", // Dev1's version
        &dev1.username,
        &dev1.email,
        "Update version to 1.1.0"
    );
    
    // Merge dev1's changes first
    git_service.merge_changes(
        &worktree_path1,
        branch_name1,
        "main",
        "Merge version update from dev1"
    ).unwrap();
    
    // Dev2 creates feature branch from original main (before dev1's merge)
    let worktree_dir2 = TempDir::new().unwrap();
    let worktree_path2 = worktree_dir2.path().join("feature2");
    let branch_name2 = "feature/version-update-2";
    
    // Create worktree from main branch (which now has dev1's changes)
    git_service.create_worktree(branch_name2, &worktree_path2, Some("main")).unwrap();
    
    let worktree_repo2 = Repository::open(&worktree_path2).unwrap();
    let mut config2 = worktree_repo2.config().unwrap();
    config2.set_str("user.name", &dev2.username).unwrap();
    config2.set_str("user.email", &dev2.email).unwrap();
    
    // Reset to original main to simulate conflicting change
    let original_main_commit = main_repo.revparse_single("HEAD~1").unwrap();
    worktree_repo2.reset(
        &original_main_commit,
        git2::ResetType::Hard,
        None
    ).unwrap();
    
    create_file_and_commit(
        &worktree_repo2,
        &worktree_path2,
        "config.rs",
        "pub const VERSION: &str = \"1.2.0\";", // Dev2's conflicting version
        &dev2.username,
        &dev2.email,
        "Update version to 1.2.0"
    );
    
    // Attempt to merge dev2's changes should result in conflict
    let merge_result = git_service.merge_changes(
        &worktree_path2,
        branch_name2,
        "main",
        "Merge version update from dev2"
    );
    
    // Should fail due to merge conflicts
    assert!(merge_result.is_err());
    match merge_result.unwrap_err() {
        GitServiceError::MergeConflicts(_) => {
            // Expected error
        },
        e => panic!("Expected MergeConflicts error, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_git_service_with_task_attempt_workflow() {
    let pool = setup_test_db().await;
    
    // Create test data
    let creator = create_test_user_with_git_config(
        &pool, 111, "creator", "creator@example.com", "Task Creator"
    ).await;
    let assignee = create_test_user_with_git_config(
        &pool, 222, "assignee", "assignee@example.com", "Task Assignee"
    ).await;
    
    // Create project
    let project_id = Uuid::new_v4();
    let create_project_data = CreateProject {
        name: "Test Project".to_string(),
        git_repo_path: "temp_path".to_string(), // Will be updated
        created_by: Some(creator.id),
    };
    let mut project = Project::create(&pool, &create_project_data, project_id).await.unwrap();
    
    // Create Git repository and update project path
    let (main_repo_dir, main_repo) = create_test_git_repo(&creator.username, &creator.email);
    create_initial_commit(&main_repo, &creator.username, &creator.email, "Project initialization");
    
    // Update project with actual repo path
    project.git_repo_path = main_repo_dir.path().to_string_lossy().to_string();
    let update_data = automagik_forge::models::project::UpdateProject {
        name: None,
        git_repo_path: Some(project.git_repo_path.clone()),
    };
    project = Project::update(&pool, project.id, &update_data).await.unwrap();
    
    // Create task
    let task_id = Uuid::new_v4();
    let create_task_data = CreateTask {
        project_id: project.id,
        title: "Implement user service".to_string(),
        description: Some("Create user service with CRUD operations".to_string()),
        wish_id: "wish-123".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee.id),
    };
    let task = Task::create(&pool, &create_task_data, task_id).await.unwrap();
    
    // Create task attempt
    let attempt_id = Uuid::new_v4();
    let create_attempt_data = CreateTaskAttempt {
        task_id: task.id,
        project_id: project.id,
        branch_name: "task/user-service-123".to_string(),
        base_branch: "main".to_string(),
        created_by: Some(assignee.id),
    };
    
    let mut task_attempt = TaskAttempt::create(&pool, &create_attempt_data, attempt_id).await.unwrap();
    
    // Initialize Git service
    let git_service = GitService::new(&project.git_repo_path).unwrap();
    
    // Create worktree for task attempt
    let worktree_dir = TempDir::new().unwrap();
    let worktree_path = worktree_dir.path().join("task-attempt");
    
    git_service.create_worktree(&task_attempt.branch_name, &worktree_path, Some("main")).unwrap();
    
    // Configure worktree with assignee's credentials
    let worktree_repo = Repository::open(&worktree_path).unwrap();
    let mut config = worktree_repo.config().unwrap();
    config.set_str("user.name", &assignee.username).unwrap();
    config.set_str("user.email", &assignee.email).unwrap();
    
    // Implement the feature (create user service files)
    create_file_and_commit(
        &worktree_repo,
        &worktree_path,
        "user_service.rs",
        r#"pub struct UserService;

impl UserService {
    pub fn new() -> Self {
        Self
    }
    
    pub fn create_user(&self, name: &str) -> String {
        format!("Created user: {}", name)
    }
    
    pub fn get_user(&self, id: u32) -> Option<String> {
        Some(format!("User {}", id))
    }
}"#,
        &assignee.username,
        &assignee.email,
        "Add UserService with basic CRUD operations"
    );
    
    create_file_and_commit(
        &worktree_repo,
        &worktree_path,
        "user_service_tests.rs",
        r#"#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_user() {
        let service = UserService::new();
        let result = service.create_user("Alice");
        assert_eq!(result, "Created user: Alice");
    }
    
    #[test]
    fn test_get_user() {
        let service = UserService::new();
        let result = service.get_user(1);
        assert_eq!(result, Some("User 1".to_string()));
    }
}"#,
        &assignee.username,
        &assignee.email,
        "Add comprehensive tests for UserService"
    );
    
    // Complete the task attempt by merging changes
    let commit_message = format!(
        "Implement user service (automagik-forge {})\n\n{}\n\nAssigned to: {} <{}>",
        attempt_id.to_string().split('-').next().unwrap(),
        task.description.as_ref().unwrap_or(&"No description".to_string()),
        assignee.display_name.as_ref().unwrap(),
        assignee.email
    );
    
    let merge_commit_id = git_service.merge_changes(
        &worktree_path,
        &task_attempt.branch_name,
        &task_attempt.base_branch,
        &commit_message
    ).unwrap();
    
    // Update task attempt with merge commit
    let update_attempt_data = automagik_forge::models::task_attempt::UpdateTaskAttempt {
        status: Some(TaskAttemptStatus::Completed),
        merge_commit_id: Some(merge_commit_id.clone()),
        completed_at: Some(Utc::now()),
        result_summary: Some("Successfully implemented UserService with CRUD operations and tests".to_string()),
        error_message: None,
        worktree_path: Some(worktree_path.to_string_lossy().to_string()),
    };
    
    task_attempt = TaskAttempt::update(&pool, attempt_id, &update_attempt_data).await.unwrap();
    
    // Verify the merge commit exists and has correct attribution
    let merge_commit = main_repo.find_commit(git2::Oid::from_str(&merge_commit_id).unwrap()).unwrap();
    let author = merge_commit.author();
    assert_eq!(author.name().unwrap(), assignee.username);
    assert_eq!(author.email().unwrap(), assignee.email);
    assert!(merge_commit.message().unwrap().contains("Implement user service"));
    assert!(merge_commit.message().unwrap().contains(&assignee.display_name.as_ref().unwrap()));
    
    // Verify files are in the main branch
    let head_commit = main_repo.head().unwrap().peel_to_commit().unwrap();
    let tree = head_commit.tree().unwrap();
    assert!(tree.get_path(Path::new("user_service.rs")).is_ok());
    assert!(tree.get_path(Path::new("user_service_tests.rs")).is_ok());
    
    // Verify task attempt status
    assert_eq!(task_attempt.status, TaskAttemptStatus::Completed);
    assert_eq!(task_attempt.merge_commit_id, Some(merge_commit_id));
    assert!(task_attempt.completed_at.is_some());
}

#[tokio::test]
async fn test_github_repository_info_extraction() {
    let (temp_dir, repo) = create_test_git_repo("Test User", "test@example.com");
    
    // Add a GitHub remote (HTTPS format)
    repo.remote("origin", "https://github.com/automagik-forge/test-repo.git").unwrap();
    
    let git_service = GitService::new(temp_dir.path()).unwrap();
    let (owner, repo_name) = git_service.get_github_repo_info().unwrap();
    
    assert_eq!(owner, "automagik-forge");
    assert_eq!(repo_name, "test-repo");
    
    // Test SSH format
    repo.remote_delete("origin").unwrap();
    repo.remote("origin", "git@github.com:automagik-forge/another-repo.git").unwrap();
    
    let (owner, repo_name) = git_service.get_github_repo_info().unwrap();
    
    assert_eq!(owner, "automagik-forge");
    assert_eq!(repo_name, "another-repo");
}

#[tokio::test]
async fn test_concurrent_git_operations() {
    let pool = setup_test_db().await;
    
    // Create test users
    let dev1 = create_test_user_with_git_config(
        &pool, 111, "dev1", "dev1@example.com", "Developer One"
    ).await;
    let dev2 = create_test_user_with_git_config(
        &pool, 222, "dev2", "dev2@example.com", "Developer Two"
    ).await;
    
    // Create main repository
    let (main_repo_dir, main_repo) = create_test_git_repo(&dev1.username, &dev1.email);
    create_initial_commit(&main_repo, &dev1.username, &dev1.email, "Initial commit");
    
    let git_service = std::sync::Arc::new(GitService::new(main_repo_dir.path()).unwrap());
    
    // Create concurrent worktrees and operations
    let git_service1 = git_service.clone();
    let git_service2 = git_service.clone();
    
    let dev1_clone = dev1.clone();
    let dev2_clone = dev2.clone();
    
    let task1 = tokio::spawn(async move {
        let worktree_dir = TempDir::new().unwrap();
        let worktree_path = worktree_dir.path().join("concurrent1");
        let branch_name = "concurrent/feature1";
        
        git_service1.create_worktree(branch_name, &worktree_path, None).unwrap();
        
        let worktree_repo = Repository::open(&worktree_path).unwrap();
        let mut config = worktree_repo.config().unwrap();
        config.set_str("user.name", &dev1_clone.username).unwrap();
        config.set_str("user.email", &dev1_clone.email).unwrap();
        
        create_file_and_commit(
            &worktree_repo,
            &worktree_path,
            "feature1.rs",
            "// Feature 1 implementation",
            &dev1_clone.username,
            &dev1_clone.email,
            "Add feature 1"
        );
        
        git_service1.merge_changes(
            &worktree_path,
            branch_name,
            "main",
            "Merge feature 1"
        )
    });
    
    let task2 = tokio::spawn(async move {
        let worktree_dir = TempDir::new().unwrap();
        let worktree_path = worktree_dir.path().join("concurrent2");
        let branch_name = "concurrent/feature2";
        
        git_service2.create_worktree(branch_name, &worktree_path, None).unwrap();
        
        let worktree_repo = Repository::open(&worktree_path).unwrap();
        let mut config = worktree_repo.config().unwrap();
        config.set_str("user.name", &dev2_clone.username).unwrap();
        config.set_str("user.email", &dev2_clone.email).unwrap();
        
        create_file_and_commit(
            &worktree_repo,
            &worktree_path,
            "feature2.rs",
            "// Feature 2 implementation",
            &dev2_clone.username,
            &dev2_clone.email,
            "Add feature 2"
        );
        
        git_service2.merge_changes(
            &worktree_path,
            branch_name,
            "main",
            "Merge feature 2"
        )
    });
    
    // Both operations should succeed
    let result1 = task1.await.unwrap();
    let result2 = task2.await.unwrap();
    
    assert!(result1.is_ok());
    assert!(result2.is_ok());
    
    // Verify both features are present
    let final_head = main_repo.head().unwrap().peel_to_commit().unwrap();
    let tree = final_head.tree().unwrap();
    assert!(tree.get_path(Path::new("feature1.rs")).is_ok());
    assert!(tree.get_path(Path::new("feature2.rs")).is_ok());
}