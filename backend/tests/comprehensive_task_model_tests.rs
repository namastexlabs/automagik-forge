use automagik_forge::models::{
    task::{Task, CreateTask, UpdateTask, TaskStatus, TaskWithAttemptStatus, TaskWithUsers},
    task_attempt::{TaskAttempt, CreateTaskAttempt},
    execution_process::{ExecutionProcess, ProcessType},
    user::{User, CreateUser},
    project::{Project, CreateProject},
};
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;
use std::collections::HashMap;

/// Comprehensive task model testing with complex query validation and edge cases
#[cfg(test)]
mod comprehensive_task_tests {
    use super::*;

    /// Setup in-memory test database with full migration
    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        
        // Run all migrations to ensure complete schema
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .unwrap();
            
        pool
    }

    /// Create test user with comprehensive data
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

    /// Create test project with validation
    async fn create_test_project(pool: &SqlitePool, name: &str, created_by: Option<Uuid>) -> Project {
        let project_id = Uuid::new_v4();
        let create_data = CreateProject {
            name: name.to_string(),
            git_repo_path: format!("/tmp/test-repo-{}", name),
            use_existing_repo: false,
            setup_script: Some("echo 'setup'".to_string()),
            dev_script: Some("echo 'dev'".to_string()),
            cleanup_script: Some("echo 'cleanup'".to_string()),
            created_by,
        };
        
        Project::create(pool, &create_data, project_id).await.unwrap()
    }

    /// Create task attempt with execution process for complex status testing
    async fn create_task_attempt_with_process(
        pool: &SqlitePool,
        task_id: Uuid,
        process_type: ProcessType,
        status: &str,
    ) -> (TaskAttempt, ExecutionProcess) {
        let attempt_id = Uuid::new_v4();
        let attempt_data = CreateTaskAttempt {
            executor: Some("test-executor".to_string()),
            base_branch: Some("main".to_string()),
            created_by: None,
        };
        
        let attempt = TaskAttempt::create(pool, &attempt_data, task_id).await.unwrap();
        
        // Create execution process
        let process_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO execution_processes (id, task_attempt_id, process_type, executor_type, status, exit_code, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            process_id,
            attempt_id,
            process_type as ProcessType,
            "test-executor",
            status,
            0,
            Utc::now(),
            Utc::now()
        )
        .execute(pool)
        .await
        .unwrap();
        
        let process = ExecutionProcess::find_by_id(pool, process_id).await.unwrap().unwrap();
        (attempt, process)
    }

    #[tokio::test]
    async fn test_task_creation_with_comprehensive_validation() {
        let pool = setup_test_db().await;
        
        // Create users
        let creator = create_test_user(&pool, 111, "creator").await;
        let assignee = create_test_user(&pool, 222, "assignee").await;
        
        // Create project
        let project = create_test_project(&pool, "validation-project", Some(creator.id)).await;
        
        // Test valid task creation
        let task_id = Uuid::new_v4();
        let create_data = CreateTask {
            project_id: project.id,
            title: "Comprehensive Test Task".to_string(),
            description: Some("Testing comprehensive validation".to_string()),
            wish_id: "wish-comprehensive-123".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: Some(assignee.id),
        };
        
        let task = Task::create(&pool, &create_data, task_id).await.unwrap();
        
        // Validate all fields are correctly set
        assert_eq!(task.id, task_id);
        assert_eq!(task.project_id, project.id);
        assert_eq!(task.title, "Comprehensive Test Task");
        assert_eq!(task.description, Some("Testing comprehensive validation".to_string()));
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(task.wish_id, "wish-comprehensive-123");
        assert_eq!(task.created_by, Some(creator.id));
        assert_eq!(task.assigned_to, Some(assignee.id));
        assert!(task.created_at <= Utc::now());
        assert!(task.updated_at <= Utc::now());
        assert!(task.created_at <= task.updated_at);
    }

    #[tokio::test]
    async fn test_task_creation_boundary_conditions() {
        let pool = setup_test_db().await;
        
        let creator = create_test_user(&pool, 111, "creator").await;
        let project = create_test_project(&pool, "boundary-project", Some(creator.id)).await;
        
        // Test minimum valid data
        let minimal_task_id = Uuid::new_v4();
        let minimal_data = CreateTask {
            project_id: project.id,
            title: "A".repeat(1), // Minimum title length
            description: None,
            wish_id: "1".to_string(), // Minimum wish_id
            parent_task_attempt: None,
            created_by: None,
            assigned_to: None,
        };
        
        let minimal_task = Task::create(&pool, &minimal_data, minimal_task_id).await.unwrap();
        assert_eq!(minimal_task.title.len(), 1);
        assert!(minimal_task.description.is_none());
        assert!(minimal_task.created_by.is_none());
        assert!(minimal_task.assigned_to.is_none());
        
        // Test maximum length data
        let max_task_id = Uuid::new_v4();
        let long_title = "A".repeat(255); // Test reasonable max length
        let long_description = "B".repeat(1000); // Test reasonable max description
        let long_wish_id = "wish-".to_string() + &"C".repeat(100);
        
        let max_data = CreateTask {
            project_id: project.id,
            title: long_title.clone(),
            description: Some(long_description.clone()),
            wish_id: long_wish_id.clone(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: None,
        };
        
        let max_task = Task::create(&pool, &max_data, max_task_id).await.unwrap();
        assert_eq!(max_task.title, long_title);
        assert_eq!(max_task.description, Some(long_description));
        assert_eq!(max_task.wish_id, long_wish_id);
    }

    #[tokio::test]
    async fn test_task_creation_constraint_violations() {
        let pool = setup_test_db().await;
        
        let creator = create_test_user(&pool, 111, "creator").await;
        let project = create_test_project(&pool, "constraint-project", Some(creator.id)).await;
        
        // Test foreign key violation - non-existent project
        let invalid_project_id = Uuid::new_v4();
        let invalid_project_data = CreateTask {
            project_id: invalid_project_id,
            title: "Invalid Project Task".to_string(),
            description: None,
            wish_id: "wish-invalid-project".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: None,
        };
        
        let result = Task::create(&pool, &invalid_project_data, Uuid::new_v4()).await;
        assert!(result.is_err());
        
        // Test foreign key violation - non-existent assigned user
        let invalid_user_id = Uuid::new_v4();
        let invalid_user_data = CreateTask {
            project_id: project.id,
            title: "Invalid User Task".to_string(),
            description: None,
            wish_id: "wish-invalid-user".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: Some(invalid_user_id),
        };
        
        let result = Task::create(&pool, &invalid_user_data, Uuid::new_v4()).await;
        assert!(result.is_err());
        
        // Test foreign key violation - non-existent creator
        let invalid_creator_id = Uuid::new_v4();
        let invalid_creator_data = CreateTask {
            project_id: project.id,
            title: "Invalid Creator Task".to_string(),
            description: None,
            wish_id: "wish-invalid-creator".to_string(),
            parent_task_attempt: None,
            created_by: Some(invalid_creator_id),
            assigned_to: None,
        };
        
        let result = Task::create(&pool, &invalid_creator_data, Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_complex_task_query_with_attempt_status() {
        let pool = setup_test_db().await;
        
        // Setup test data
        let creator = create_test_user(&pool, 111, "creator").await;
        let project = create_test_project(&pool, "complex-query-project", Some(creator.id)).await;
        
        // Create tasks with different attempt states
        let task1_id = Uuid::new_v4();
        let task1_data = CreateTask {
            project_id: project.id,
            title: "Task with Running Attempt".to_string(),
            description: Some("This task has a running attempt".to_string()),
            wish_id: "wish-running".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: None,
        };
        Task::create(&pool, &task1_data, task1_id).await.unwrap();
        
        // Create running attempt
        create_task_attempt_with_process(&pool, task1_id, ProcessType::CodingAgent, "running").await;
        
        let task2_id = Uuid::new_v4();
        let task2_data = CreateTask {
            project_id: project.id,
            title: "Task with Merged Attempt".to_string(),
            description: Some("This task has a merged attempt".to_string()),
            wish_id: "wish-merged".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: None,
        };
        Task::create(&pool, &task2_data, task2_id).await.unwrap();
        
        // Create merged attempt
        let (mut merged_attempt, _) = create_task_attempt_with_process(&pool, task2_id, ProcessType::CodingAgent, "completed").await;
        
        // Set merge commit to simulate merged state
        sqlx::query!(
            "UPDATE task_attempts SET merge_commit = ? WHERE id = ?",
            "abc123def456",
            merged_attempt.id
        )
        .execute(&pool)
        .await
        .unwrap();
        
        let task3_id = Uuid::new_v4();
        let task3_data = CreateTask {
            project_id: project.id,
            title: "Task with Failed Attempt".to_string(),
            description: Some("This task has a failed attempt".to_string()),
            wish_id: "wish-failed".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: None,
        };
        Task::create(&pool, &task3_data, task3_id).await.unwrap();
        
        // Create failed attempt
        create_task_attempt_with_process(&pool, task3_id, ProcessType::CodingAgent, "failed").await;
        
        // Query tasks with attempt status
        let tasks_with_status = Task::find_by_project_id_with_attempt_status(&pool, project.id).await.unwrap();
        
        assert_eq!(tasks_with_status.len(), 3);
        
        // Verify each task's attempt status
        let task_map: HashMap<String, &TaskWithAttemptStatus> = tasks_with_status
            .iter()
            .map(|t| (t.title.clone(), t))
            .collect();
        
        let running_task = task_map.get("Task with Running Attempt").unwrap();
        assert!(running_task.has_in_progress_attempt);
        assert!(!running_task.has_merged_attempt);
        assert!(!running_task.last_attempt_failed);
        
        let merged_task = task_map.get("Task with Merged Attempt").unwrap();
        assert!(!merged_task.has_in_progress_attempt);
        assert!(merged_task.has_merged_attempt);
        assert!(!merged_task.last_attempt_failed);
        
        let failed_task = task_map.get("Task with Failed Attempt").unwrap();
        assert!(!failed_task.has_in_progress_attempt);
        assert!(!failed_task.has_merged_attempt);
        assert!(failed_task.last_attempt_failed);
    }

    #[tokio::test]
    async fn test_complex_task_query_with_users() {
        let pool = setup_test_db().await;
        
        // Create multiple users
        let creator1 = create_test_user(&pool, 111, "creator1").await;
        let creator2 = create_test_user(&pool, 222, "creator2").await;
        let assignee1 = create_test_user(&pool, 333, "assignee1").await;
        let assignee2 = create_test_user(&pool, 444, "assignee2").await;
        
        let project = create_test_project(&pool, "user-query-project", Some(creator1.id)).await;
        
        // Create tasks with different user combinations
        let tasks_data = vec![
            ("Task by Creator1 for Assignee1", creator1.id, Some(assignee1.id)),
            ("Task by Creator1 for Assignee2", creator1.id, Some(assignee2.id)),
            ("Task by Creator2 for Assignee1", creator2.id, Some(assignee1.id)),
            ("Task by Creator2 unassigned", creator2.id, None),
        ];
        
        for (title, creator_id, assignee_id) in tasks_data {
            let task_id = Uuid::new_v4();
            let task_data = CreateTask {
                project_id: project.id,
                title: title.to_string(),
                description: Some(format!("Description for {}", title)),
                wish_id: format!("wish-{}", title.replace(' ', "-").to_lowercase()),
                parent_task_attempt: None,
                created_by: Some(creator_id),
                assigned_to: assignee_id,
            };
            
            Task::create(&pool, &task_data, task_id).await.unwrap();
        }
        
        // Query tasks with user information
        let tasks_with_users = Task::find_by_project_id_with_users(&pool, project.id).await.unwrap();
        assert_eq!(tasks_with_users.len(), 4);
        
        // Verify user information is correctly joined
        for task in &tasks_with_users {
            // All tasks should have creator information
            assert!(task.creator_username.is_some());
            assert!(task.creator_display_name.is_some());
            
            // Verify specific combinations
            match task.title.as_str() {
                "Task by Creator1 for Assignee1" => {
                    assert_eq!(task.creator_username, Some("creator1".to_string()));
                    assert_eq!(task.assignee_username, Some("assignee1".to_string()));
                    assert!(task.assignee_display_name.is_some());
                },
                "Task by Creator1 for Assignee2" => {
                    assert_eq!(task.creator_username, Some("creator1".to_string()));
                    assert_eq!(task.assignee_username, Some("assignee2".to_string()));
                },
                "Task by Creator2 for Assignee1" => {
                    assert_eq!(task.creator_username, Some("creator2".to_string()));
                    assert_eq!(task.assignee_username, Some("assignee1".to_string()));
                },
                "Task by Creator2 unassigned" => {
                    assert_eq!(task.creator_username, Some("creator2".to_string()));
                    assert_eq!(task.assignee_username, None);
                    assert_eq!(task.assignee_display_name, None);
                },
                _ => panic!("Unexpected task title: {}", task.title),
            }
        }
        
        // Verify tasks are ordered by created_at DESC
        for i in 1..tasks_with_users.len() {
            assert!(tasks_with_users[i-1].created_at >= tasks_with_users[i].created_at);
        }
    }

    #[tokio::test]
    async fn test_task_update_operations_comprehensive() {
        let pool = setup_test_db().await;
        
        let creator = create_test_user(&pool, 111, "creator").await;
        let assignee1 = create_test_user(&pool, 222, "assignee1").await;
        let assignee2 = create_test_user(&pool, 333, "assignee2").await;
        let project = create_test_project(&pool, "update-project", Some(creator.id)).await;
        
        // Create initial task
        let task_id = Uuid::new_v4();
        let initial_data = CreateTask {
            project_id: project.id,
            title: "Original Title".to_string(),
            description: Some("Original Description".to_string()),
            wish_id: "original-wish".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: Some(assignee1.id),
        };
        
        let original_task = Task::create(&pool, &initial_data, task_id).await.unwrap();
        let original_updated_at = original_task.updated_at;
        
        // Wait to ensure updated_at changes
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        // Test comprehensive update
        let updated_task = Task::update(
            &pool,
            task_id,
            project.id,
            "Updated Title".to_string(),
            Some("Updated Description".to_string()),
            TaskStatus::InProgress,
            "updated-wish".to_string(),
            None,
            Some(assignee2.id),
        ).await.unwrap();
        
        // Verify all fields were updated
        assert_eq!(updated_task.title, "Updated Title");
        assert_eq!(updated_task.description, Some("Updated Description".to_string()));
        assert_eq!(updated_task.status, TaskStatus::InProgress);
        assert_eq!(updated_task.wish_id, "updated-wish");
        assert_eq!(updated_task.assigned_to, Some(assignee2.id));
        assert!(updated_task.updated_at > original_updated_at);
        
        // Verify unchanged fields
        assert_eq!(updated_task.id, original_task.id);
        assert_eq!(updated_task.project_id, original_task.project_id);
        assert_eq!(updated_task.created_by, original_task.created_by);
        assert_eq!(updated_task.created_at, original_task.created_at);
    }

    #[tokio::test]
    async fn test_task_status_update_isolated() {
        let pool = setup_test_db().await;
        
        let creator = create_test_user(&pool, 111, "creator").await;
        let assignee = create_test_user(&pool, 222, "assignee").await;
        let project = create_test_project(&pool, "status-project", Some(creator.id)).await;
        
        // Create task
        let task_id = Uuid::new_v4();
        let task_data = CreateTask {
            project_id: project.id,
            title: "Status Update Test".to_string(),
            description: Some("Testing status updates".to_string()),
            wish_id: "status-wish".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: Some(assignee.id),
        };
        
        let original_task = Task::create(&pool, &task_data, task_id).await.unwrap();
        assert_eq!(original_task.status, TaskStatus::Todo);
        
        let original_updated_at = original_task.updated_at;
        
        // Wait to ensure updated_at changes
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        // Update status only
        Task::update_status(&pool, task_id, project.id, TaskStatus::InProgress).await.unwrap();
        
        // Verify status was updated and other fields unchanged
        let updated_task = Task::find_by_id(&pool, task_id).await.unwrap().unwrap();
        assert_eq!(updated_task.status, TaskStatus::InProgress);
        assert!(updated_task.updated_at > original_updated_at);
        
        // Verify other fields unchanged
        assert_eq!(updated_task.title, original_task.title);
        assert_eq!(updated_task.description, original_task.description);
        assert_eq!(updated_task.wish_id, original_task.wish_id);
        assert_eq!(updated_task.assigned_to, original_task.assigned_to);
        assert_eq!(updated_task.created_by, original_task.created_by);
        assert_eq!(updated_task.created_at, original_task.created_at);
    }

    #[tokio::test]
    async fn test_task_related_query_complex_hierarchy() {
        let pool = setup_test_db().await;
        
        let creator = create_test_user(&pool, 111, "creator").await;
        let project = create_test_project(&pool, "hierarchy-project", Some(creator.id)).await;
        
        // Create parent task
        let parent_task_id = Uuid::new_v4();
        let parent_data = CreateTask {
            project_id: project.id,
            title: "Parent Task".to_string(),
            description: Some("Parent task for hierarchy testing".to_string()),
            wish_id: "parent-wish".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: None,
        };
        Task::create(&pool, &parent_data, parent_task_id).await.unwrap();
        
        // Create parent task attempt
        let parent_attempt_id = Uuid::new_v4();
        let parent_attempt_data = CreateTaskAttempt {
            executor: Some("parent-executor".to_string()),
            base_branch: Some("main".to_string()),
            created_by: Some(creator.id),
        };
        let parent_attempt = TaskAttempt::create(&pool, &parent_attempt_data, parent_task_id).await.unwrap();
        
        // Create child tasks that reference the parent attempt
        let child1_id = Uuid::new_v4();
        let child1_data = CreateTask {
            project_id: project.id,
            title: "Child Task 1".to_string(),
            description: Some("First child task".to_string()),
            wish_id: "child1-wish".to_string(),
            parent_task_attempt: Some(parent_attempt.id),
            created_by: Some(creator.id),
            assigned_to: None,
        };
        Task::create(&pool, &child1_data, child1_id).await.unwrap();
        
        let child2_id = Uuid::new_v4();
        let child2_data = CreateTask {
            project_id: project.id,
            title: "Child Task 2".to_string(),
            description: Some("Second child task".to_string()),
            wish_id: "child2-wish".to_string(),
            parent_task_attempt: Some(parent_attempt.id),
            created_by: Some(creator.id),
            assigned_to: None,
        };
        Task::create(&pool, &child2_data, child2_id).await.unwrap();
        
        // Query related tasks from child1's attempt perspective
        let child1_attempt_id = Uuid::new_v4();
        let child1_attempt_data = CreateTaskAttempt {
            executor: Some("child1-executor".to_string()),
            base_branch: Some("main".to_string()),
            created_by: Some(creator.id),
        };
        let child1_attempt = TaskAttempt::create(&pool, &child1_attempt_data, child1_id).await.unwrap();
        
        // Find related tasks - should include parent and sibling
        let related_tasks = Task::find_related_tasks_by_attempt_id(&pool, child1_attempt.id, project.id).await.unwrap();
        
        // Should find sibling (child2) and potentially parent
        // Exact behavior depends on the complex SQL logic in find_related_tasks_by_attempt_id
        assert!(!related_tasks.is_empty());
        
        // Verify that the current task itself is excluded
        for related_task in &related_tasks {
            assert_ne!(related_task.id, child1_id);
        }
        
        // Verify all related tasks belong to the same project
        for related_task in &related_tasks {
            assert_eq!(related_task.project_id, project.id);
        }
    }

    #[tokio::test]
    async fn test_task_deletion_comprehensive() {
        let pool = setup_test_db().await;
        
        let creator = create_test_user(&pool, 111, "creator").await;
        let project = create_test_project(&pool, "deletion-project", Some(creator.id)).await;
        
        // Create task to delete
        let task_id = Uuid::new_v4();
        let task_data = CreateTask {
            project_id: project.id,
            title: "Task to Delete".to_string(),
            description: Some("This task will be deleted".to_string()),
            wish_id: "delete-wish".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: None,
        };
        
        Task::create(&pool, &task_data, task_id).await.unwrap();
        
        // Verify task exists
        assert!(Task::exists(&pool, task_id, project.id).await.unwrap());
        let found_task = Task::find_by_id(&pool, task_id).await.unwrap();
        assert!(found_task.is_some());
        
        // Delete task
        let rows_affected = Task::delete(&pool, task_id, project.id).await.unwrap();
        assert_eq!(rows_affected, 1);
        
        // Verify task no longer exists
        assert!(!Task::exists(&pool, task_id, project.id).await.unwrap());
        let deleted_task = Task::find_by_id(&pool, task_id).await.unwrap();
        assert!(deleted_task.is_none());
        
        // Try to delete non-existent task
        let non_existent_id = Uuid::new_v4();
        let rows_affected = Task::delete(&pool, non_existent_id, project.id).await.unwrap();
        assert_eq!(rows_affected, 0);
        
        // Try to delete with wrong project ID
        let another_task_id = Uuid::new_v4();
        let another_task_data = CreateTask {
            project_id: project.id,
            title: "Another Task".to_string(),
            description: None,
            wish_id: "another-wish".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: None,
        };
        Task::create(&pool, &another_task_data, another_task_id).await.unwrap();
        
        let wrong_project_id = Uuid::new_v4();
        let rows_affected = Task::delete(&pool, another_task_id, wrong_project_id).await.unwrap();
        assert_eq!(rows_affected, 0);
        
        // Verify task still exists
        assert!(Task::exists(&pool, another_task_id, project.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_task_concurrent_operations() {
        let pool = setup_test_db().await;
        
        let creator = create_test_user(&pool, 111, "creator").await;
        let assignee1 = create_test_user(&pool, 222, "assignee1").await;
        let assignee2 = create_test_user(&pool, 333, "assignee2").await;
        let project = create_test_project(&pool, "concurrent-project", Some(creator.id)).await;
        
        // Create task for concurrent operations
        let task_id = Uuid::new_v4();
        let task_data = CreateTask {
            project_id: project.id,
            title: "Concurrent Test Task".to_string(),
            description: Some("Testing concurrent operations".to_string()),
            wish_id: "concurrent-wish".to_string(),
            parent_task_attempt: None,
            created_by: Some(creator.id),
            assigned_to: Some(assignee1.id),
        };
        
        Task::create(&pool, &task_data, task_id).await.unwrap();
        
        // Spawn concurrent update operations
        let pool1 = pool.clone();
        let pool2 = pool.clone();
        let project_id = project.id;
        
        let update1 = tokio::spawn(async move {
            Task::update(
                &pool1,
                task_id,
                project_id,
                "Updated by Thread 1".to_string(),
                Some("Description by Thread 1".to_string()),
                TaskStatus::InProgress,
                "concurrent-wish".to_string(),
                None,
                Some(assignee1.id),
            ).await
        });
        
        let update2 = tokio::spawn(async move {
            Task::update(
                &pool2,
                task_id,
                project_id,
                "Updated by Thread 2".to_string(),
                Some("Description by Thread 2".to_string()),
                TaskStatus::InReview,
                "concurrent-wish".to_string(),
                None,
                Some(assignee2.id),
            ).await
        });
        
        // Both updates should succeed (SQLite handles concurrency)
        let result1 = update1.await.unwrap();
        let result2 = update2.await.unwrap();
        
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        
        // Verify final state (one of the updates should have won)
        let final_task = Task::find_by_id(&pool, task_id).await.unwrap().unwrap();
        assert!(
            final_task.title == "Updated by Thread 1" || 
            final_task.title == "Updated by Thread 2"
        );
        assert!(
            final_task.status == TaskStatus::InProgress || 
            final_task.status == TaskStatus::InReview
        );
    }

    #[tokio::test]
    async fn test_task_sql_injection_protection() {
        let pool = setup_test_db().await;
        
        let creator = create_test_user(&pool, 111, "creator").await;
        let project = create_test_project(&pool, "injection-project", Some(creator.id)).await;
        
        // Test SQL injection attempts in various fields
        let malicious_inputs = vec![
            "'; DROP TABLE tasks; --",
            "'; UPDATE tasks SET title = 'hacked'; --",
            "' OR '1'='1",
            "'; SELECT * FROM users; --",
            "<script>alert('xss')</script>",
            "\x00\x01\x02\x03", // Binary data
        ];
        
        for malicious_input in malicious_inputs {
            let task_id = Uuid::new_v4();
            let task_data = CreateTask {
                project_id: project.id,
                title: format!("Safe Title {}", malicious_input),
                description: Some(format!("Safe Description {}", malicious_input)),
                wish_id: format!("safe-wish-{}", malicious_input),
                parent_task_attempt: None,
                created_by: Some(creator.id),
                assigned_to: None,
            };
            
            // Should handle malicious input safely due to parameterized queries
            let result = Task::create(&pool, &task_data, task_id).await;
            if result.is_ok() {
                let created_task = result.unwrap();
                
                // Verify the malicious input was stored as literal text
                assert!(created_task.title.contains(malicious_input));
                if let Some(desc) = &created_task.description {
                    assert!(desc.contains(malicious_input));
                }
                assert!(created_task.wish_id.contains(malicious_input));
                
                // Verify we can query it back safely
                let found_task = Task::find_by_id(&pool, task_id).await.unwrap().unwrap();
                assert_eq!(found_task.title, created_task.title);
                assert_eq!(found_task.description, created_task.description);
                assert_eq!(found_task.wish_id, created_task.wish_id);
            }
        }
        
        // Verify tasks table still exists and is functional
        let all_tasks = Task::find_by_project_id_with_attempt_status(&pool, project.id).await.unwrap();
        assert!(!all_tasks.is_empty());
    }
}