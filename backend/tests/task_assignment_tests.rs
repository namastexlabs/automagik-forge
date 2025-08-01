use automagik_forge::models::{
    task::{Task, CreateTask, UpdateTask, TaskStatus, TaskWithUsers},
    user::{User, CreateUser},
    project::{Project, CreateProject},
};
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

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

/// Helper function to create a test user
async fn create_test_user(pool: &SqlitePool, github_id: i64, username: &str) -> User {
    let user_id = Uuid::new_v4();
    let create_data = CreateUser {
        github_id,
        username: username.to_string(),
        email: format!("{}@example.com", username),
        display_name: Some(format!("Test User {}", username)),
        avatar_url: Some(format!("https://github.com/{}.png", username)),
        github_token: Some("encrypted_token".to_string()),
        is_admin: Some(false),
    };
    
    User::create(pool, &create_data, user_id).await.unwrap()
}

/// Helper function to create a test project
async fn create_test_project(pool: &SqlitePool, name: &str, created_by: Option<Uuid>) -> Project {
    let project_id = Uuid::new_v4();
    let create_data = CreateProject {
        name: name.to_string(),
        git_repo_path: format!("/tmp/test-repo-{}", name),
        created_by,
    };
    
    Project::create(pool, &create_data, project_id).await.unwrap()
}

#[tokio::test]
async fn test_task_creation_with_user_assignment() {
    let pool = setup_test_db().await;
    
    // Create users
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee = create_test_user(&pool, 222, "assignee").await;
    
    // Create project
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create task with user assignment
    let task_id = Uuid::new_v4();
    let create_data = CreateTask {
        project_id: project.id,
        title: "Test Task".to_string(),
        description: Some("Test task description".to_string()),
        wish_id: "wish-123".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee.id),
    };
    
    let task = Task::create(&pool, &create_data, task_id).await.unwrap();
    
    assert_eq!(task.id, task_id);
    assert_eq!(task.project_id, project.id);
    assert_eq!(task.title, "Test Task");
    assert_eq!(task.description, Some("Test task description".to_string()));
    assert_eq!(task.status, TaskStatus::Todo);
    assert_eq!(task.wish_id, "wish-123");
    assert_eq!(task.created_by, Some(creator.id));
    assert_eq!(task.assigned_to, Some(assignee.id));
}

#[tokio::test]
async fn test_task_creation_without_assignment() {
    let pool = setup_test_db().await;
    
    // Create user and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create task without assignment
    let task_id = Uuid::new_v4();
    let create_data = CreateTask {
        project_id: project.id,
        title: "Unassigned Task".to_string(),
        description: None,
        wish_id: "wish-456".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: None, // No assignment
    };
    
    let task = Task::create(&pool, &create_data, task_id).await.unwrap();
    
    assert_eq!(task.title, "Unassigned Task");
    assert_eq!(task.created_by, Some(creator.id));
    assert_eq!(task.assigned_to, None);
    assert!(task.description.is_none());
}

#[tokio::test]
async fn test_task_assignment_update() {
    let pool = setup_test_db().await;
    
    // Create users
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee1 = create_test_user(&pool, 222, "assignee1").await;
    let assignee2 = create_test_user(&pool, 333, "assignee2").await;
    
    // Create project and task
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    let task_id = Uuid::new_v4();
    let create_data = CreateTask {
        project_id: project.id,
        title: "Test Task".to_string(),
        description: Some("Test description".to_string()),
        wish_id: "wish-123".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee1.id),
    };
    
    let original_task = Task::create(&pool, &create_data, task_id).await.unwrap();
    assert_eq!(original_task.assigned_to, Some(assignee1.id));
    let original_updated_at = original_task.updated_at;
    
    // Wait a bit to ensure updated_at changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    // Update task assignment
    let updated_task = Task::update(
        &pool,
        task_id,
        project.id,
        "Updated Task Title".to_string(),
        Some("Updated description".to_string()),
        TaskStatus::InProgress,
        "wish-123".to_string(),
        None,
        Some(assignee2.id), // Reassign to different user
    ).await.unwrap();
    
    assert_eq!(updated_task.title, "Updated Task Title");
    assert_eq!(updated_task.description, Some("Updated description".to_string()));
    assert_eq!(updated_task.status, TaskStatus::InProgress);
    assert_eq!(updated_task.assigned_to, Some(assignee2.id));
    assert!(updated_task.updated_at > original_updated_at);
}

#[tokio::test]
async fn test_task_unassignment() {
    let pool = setup_test_db().await;
    
    // Create users and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee = create_test_user(&pool, 222, "assignee").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create assigned task
    let task_id = Uuid::new_v4();
    let create_data = CreateTask {
        project_id: project.id,
        title: "Assigned Task".to_string(),
        description: Some("Test description".to_string()),
        wish_id: "wish-123".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee.id),
    };
    
    let task = Task::create(&pool, &create_data, task_id).await.unwrap();
    assert_eq!(task.assigned_to, Some(assignee.id));
    
    // Unassign task
    let unassigned_task = Task::update(
        &pool,
        task_id,
        project.id,
        task.title,
        task.description,
        task.status,
        task.wish_id,
        task.parent_task_attempt,
        None, // Remove assignment
    ).await.unwrap();
    
    assert_eq!(unassigned_task.assigned_to, None);
    assert_eq!(unassigned_task.created_by, Some(creator.id)); // Creator should remain
}

#[tokio::test]
async fn test_task_find_by_project_with_users() {
    let pool = setup_test_db().await;
    
    // Create users
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee1 = create_test_user(&pool, 222, "assignee1").await;
    let assignee2 = create_test_user(&pool, 333, "assignee2").await;
    
    // Create project
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create tasks with different assignments
    let task1_id = Uuid::new_v4();
    let task1_data = CreateTask {
        project_id: project.id,
        title: "Task 1".to_string(),
        description: Some("First task".to_string()),
        wish_id: "wish-1".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee1.id),
    };
    Task::create(&pool, &task1_data, task1_id).await.unwrap();
    
    let task2_id = Uuid::new_v4();
    let task2_data = CreateTask {
        project_id: project.id,
        title: "Task 2".to_string(),
        description: Some("Second task".to_string()),
        wish_id: "wish-2".to_string(),
        parent_task_attempt: None,
        created_by: Some(assignee1.id), // Different creator
        assigned_to: Some(assignee2.id), // Different assignee
    };
    Task::create(&pool, &task2_data, task2_id).await.unwrap();
    
    let task3_id = Uuid::new_v4();
    let task3_data = CreateTask {
        project_id: project.id,
        title: "Task 3".to_string(),
        description: Some("Third task - unassigned".to_string()),
        wish_id: "wish-3".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: None, // Unassigned
    };
    Task::create(&pool, &task3_data, task3_id).await.unwrap();
    
    // Fetch tasks with user information
    let tasks_with_users = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    
    assert_eq!(tasks_with_users.len(), 3);
    
    // Tasks should be ordered by created_at DESC (most recent first)
    let task3_with_users = &tasks_with_users[0]; // Most recent
    let task2_with_users = &tasks_with_users[1];
    let task1_with_users = &tasks_with_users[2]; // Oldest
    
    // Verify Task 1 user information
    assert_eq!(task1_with_users.title, "Task 1");
    assert_eq!(task1_with_users.creator_username, Some("creator".to_string()));
    assert_eq!(task1_with_users.creator_display_name, Some("Test User creator".to_string()));
    assert_eq!(task1_with_users.assignee_username, Some("assignee1".to_string()));
    assert_eq!(task1_with_users.assignee_display_name, Some("Test User assignee1".to_string()));
    
    // Verify Task 2 user information
    assert_eq!(task2_with_users.title, "Task 2");
    assert_eq!(task2_with_users.creator_username, Some("assignee1".to_string()));
    assert_eq!(task2_with_users.assignee_username, Some("assignee2".to_string()));
    
    // Verify Task 3 user information (unassigned)
    assert_eq!(task3_with_users.title, "Task 3");
    assert_eq!(task3_with_users.creator_username, Some("creator".to_string()));
    assert_eq!(task3_with_users.assignee_username, None);
    assert_eq!(task3_with_users.assignee_display_name, None);
}

#[tokio::test]
async fn test_task_assignment_to_nonexistent_user() {
    let pool = setup_test_db().await;
    
    // Create user and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Attempt to create task assigned to non-existent user
    let task_id = Uuid::new_v4();
    let nonexistent_user_id = Uuid::new_v4();
    let create_data = CreateTask {
        project_id: project.id,
        title: "Invalid Assignment".to_string(),
        description: Some("Task assigned to non-existent user".to_string()),
        wish_id: "wish-invalid".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(nonexistent_user_id),
    };
    
    // This should fail due to foreign key constraint
    let result = Task::create(&pool, &create_data, task_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_task_status_management_with_assignments() {
    let pool = setup_test_db().await;
    
    // Create users and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee = create_test_user(&pool, 222, "assignee").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create task
    let task_id = Uuid::new_v4();
    let create_data = CreateTask {
        project_id: project.id,
        title: "Status Test Task".to_string(),
        description: Some("Testing status changes".to_string()),
        wish_id: "wish-status".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee.id),
    };
    
    let task = Task::create(&pool, &create_data, task_id).await.unwrap();
    assert_eq!(task.status, TaskStatus::Todo);
    
    // Update status to InProgress
    Task::update_status(&pool, task_id, project.id, TaskStatus::InProgress).await.unwrap();
    let updated_task = Task::find_by_id(&pool, task_id).await.unwrap().unwrap();
    assert_eq!(updated_task.status, TaskStatus::InProgress);
    assert_eq!(updated_task.assigned_to, Some(assignee.id)); // Assignment should remain
    
    // Update status to Done
    Task::update_status(&pool, task_id, project.id, TaskStatus::Done).await.unwrap();
    let completed_task = Task::find_by_id(&pool, task_id).await.unwrap().unwrap();
    assert_eq!(completed_task.status, TaskStatus::Done);
    assert_eq!(completed_task.assigned_to, Some(assignee.id)); // Assignment should still remain
}

#[tokio::test]
async fn test_multiple_tasks_same_assignee() {
    let pool = setup_test_db().await;
    
    // Create users and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee = create_test_user(&pool, 222, "busy_assignee").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create multiple tasks assigned to same user
    for i in 1..=3 {
        let task_id = Uuid::new_v4();
        let create_data = CreateTask {
            project_id: project.id,
            title: format!("Task {}", i),
            description: Some(format!("Task {} description", i)),
            wish_id: format!("wish-{}", i),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: Some(assignee.id),
        };
        
        Task::create(&pool, &create_data, task_id).await.unwrap();
    }
    
    // Verify all tasks are assigned to the same user
    let tasks_with_users = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    assert_eq!(tasks_with_users.len(), 3);
    
    for task in &tasks_with_users {
        assert_eq!(task.assignee_username, Some("busy_assignee".to_string()));
        assert_eq!(task.creator_username, Some("creator".to_string()));
    }
}

#[tokio::test]
async fn test_task_hierarchical_relationships_with_users() {
    let pool = setup_test_db().await;
    
    // Create users and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee = create_test_user(&pool, 222, "assignee").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create parent task
    let parent_task_id = Uuid::new_v4();
    let parent_data = CreateTask {
        project_id: project.id,
        title: "Parent Task".to_string(),
        description: Some("Parent task description".to_string()),
        wish_id: "parent-wish".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee.id),
    };
    
    Task::create(&pool, &parent_data, parent_task_id).await.unwrap();
    
    // Create child task with parent reference
    // Note: This would typically reference a task_attempt_id, but for this test
    // we'll just verify the basic structure works
    let child_task_id = Uuid::new_v4();
    let child_data = CreateTask {
        project_id: project.id,
        title: "Child Task".to_string(),
        description: Some("Child task description".to_string()),
        wish_id: "child-wish".to_string(),
        parent_task_attempt: None, // Would normally be set to parent's attempt ID
        created_by: Some(assignee.id), // Different creator
        assigned_to: Some(creator.id), // Different assignee
    };
    
    Task::create(&pool, &child_data, child_task_id).await.unwrap();
    
    // Verify both tasks exist with correct assignments
    let tasks = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    assert_eq!(tasks.len(), 2);
    
    // Find parent and child tasks
    let parent_task = tasks.iter().find(|t| t.title == "Parent Task").unwrap();
    let child_task = tasks.iter().find(|t| t.title == "Child Task").unwrap();
    
    // Verify parent task assignments
    assert_eq!(parent_task.creator_username, Some("creator".to_string()));
    assert_eq!(parent_task.assignee_username, Some("assignee".to_string()));
    
    // Verify child task assignments (reversed)
    assert_eq!(child_task.creator_username, Some("assignee".to_string()));
    assert_eq!(child_task.assignee_username, Some("creator".to_string()));
}

#[tokio::test]
async fn test_task_deletion_with_assignments() {
    let pool = setup_test_db().await;
    
    // Create users and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee = create_test_user(&pool, 222, "assignee").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create task
    let task_id = Uuid::new_v4();
    let create_data = CreateTask {
        project_id: project.id,
        title: "Task to Delete".to_string(),
        description: Some("This task will be deleted".to_string()),
        wish_id: "delete-wish".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee.id),
    };
    
    Task::create(&pool, &create_data, task_id).await.unwrap();
    
    // Verify task exists
    let task = Task::find_by_id(&pool, task_id).await.unwrap();
    assert!(task.is_some());
    
    // Delete task
    let rows_affected = Task::delete(&pool, task_id, project.id).await.unwrap();
    assert_eq!(rows_affected, 1);
    
    // Verify task no longer exists
    let deleted_task = Task::find_by_id(&pool, task_id).await.unwrap();
    assert!(deleted_task.is_none());
    
    // Verify users still exist (should not be affected by task deletion)
    let creator_exists = User::exists(&pool, creator.id).await.unwrap();
    let assignee_exists = User::exists(&pool, assignee.id).await.unwrap();
    assert!(creator_exists);
    assert!(assignee_exists);
}

#[tokio::test]
async fn test_concurrent_task_assignments() {
    let pool = setup_test_db().await;
    
    // Create users and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee1 = create_test_user(&pool, 222, "assignee1").await;
    let assignee2 = create_test_user(&pool, 333, "assignee2").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create task
    let task_id = Uuid::new_v4();
    let create_data = CreateTask {
        project_id: project.id,
        title: "Concurrent Assignment Test".to_string(),
        description: Some("Testing concurrent assignments".to_string()),
        wish_id: "concurrent-wish".to_string(),
        parent_task_attempt: None,
        created_by: Some(creator.id),
        assigned_to: Some(assignee1.id),
    };
    
    Task::create(&pool, &create_data, task_id).await.unwrap();
    
    // Simulate concurrent assignment updates
    let pool1 = pool.clone();
    let pool2 = pool.clone();
    let project_id = project.id;
    
    let task1 = tokio::spawn(async move {
        Task::update(
            &pool1,
            task_id,
            project_id,
            "Updated by Task 1".to_string(),
            Some("Description 1".to_string()),
            TaskStatus::InProgress,
            "concurrent-wish".to_string(),
            None,
            Some(assignee1.id),
        ).await
    });
    
    let task2 = tokio::spawn(async move {
        Task::update(
            &pool2,
            task_id,
            project_id,
            "Updated by Task 2".to_string(),
            Some("Description 2".to_string()),
            TaskStatus::InReview,
            "concurrent-wish".to_string(),
            None,
            Some(assignee2.id),
        ).await
    });
    
    // Both updates should succeed (last one wins)
    let result1 = task1.await.unwrap();
    let result2 = task2.await.unwrap();
    
    assert!(result1.is_ok());
    assert!(result2.is_ok());
    
    // Verify final state
    let final_task = Task::find_by_id(&pool, task_id).await.unwrap().unwrap();
    // One of the updates should have won (non-deterministic which one due to concurrency)
    assert!(final_task.assigned_to == Some(assignee1.id) || final_task.assigned_to == Some(assignee2.id));
}

#[tokio::test]
async fn test_task_filtering_by_user_assignments() {
    let pool = setup_test_db().await;
    
    // Create users and project
    let creator = create_test_user(&pool, 111, "creator").await;
    let assignee1 = create_test_user(&pool, 222, "assignee1").await;
    let assignee2 = create_test_user(&pool, 333, "assignee2").await;
    let project = create_test_project(&pool, "test-project", Some(creator.id)).await;
    
    // Create tasks with different assignments
    let tasks_data = vec![
        ("Task for Assignee 1", Some(assignee1.id)),
        ("Task for Assignee 2", Some(assignee2.id)),
        ("Unassigned Task", None),
        ("Another Task for Assignee 1", Some(assignee1.id)),
    ];
    
    for (title, assigned_to) in tasks_data {
        let task_id = Uuid::new_v4();
        let create_data = CreateTask {
            project_id: project.id,
            title: title.to_string(),
            description: Some(format!("Description for {}", title)),
            wish_id: format!("wish-{}", title.replace(' ', "-").to_lowercase()),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to,
        };
        
        Task::create(&pool, &create_data, task_id).await.unwrap();
    }
    
    // Get all tasks with user info
    let all_tasks = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
    assert_eq!(all_tasks.len(), 4);
    
    // Filter tasks assigned to assignee1
    let assignee1_tasks: Vec<_> = all_tasks.iter()
        .filter(|task| task.assignee_username == Some("assignee1".to_string()))
        .collect();
    assert_eq!(assignee1_tasks.len(), 2);
    
    // Filter tasks assigned to assignee2
    let assignee2_tasks: Vec<_> = all_tasks.iter()
        .filter(|task| task.assignee_username == Some("assignee2".to_string()))
        .collect();
    assert_eq!(assignee2_tasks.len(), 1);
    
    // Filter unassigned tasks
    let unassigned_tasks: Vec<_> = all_tasks.iter()
        .filter(|task| task.assignee_username.is_none())
        .collect();
    assert_eq!(unassigned_tasks.len(), 1);
    
    // All tasks should be created by the same user
    for task in &all_tasks {
        assert_eq!(task.creator_username, Some("creator".to_string()));
    }
}