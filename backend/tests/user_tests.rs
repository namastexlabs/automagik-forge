use automagik_forge::models::{
    user::{User, CreateUser, UpdateUser},
    github_whitelist::GitHubWhitelist,
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

/// Helper function to create test user data
fn create_test_user_data() -> CreateUser {
    CreateUser {
        github_id: 123456,
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        display_name: Some("Test User".to_string()),
        avatar_url: Some("https://github.com/testuser.png".to_string()),
        github_token: Some("encrypted_token".to_string()),
        is_admin: Some(false),
    }
}

#[tokio::test]
async fn test_user_create() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    let user = User::create(&pool, &create_data, user_id).await.unwrap();
    
    assert_eq!(user.id, user_id);
    assert_eq!(user.github_id, 123456);
    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, "test@example.com");
    assert_eq!(user.display_name, Some("Test User".to_string()));
    assert_eq!(user.is_admin, false);
    assert_eq!(user.is_whitelisted, true); // Default value
    assert!(user.last_login_at.is_none()); // Should be None on creation
}

#[tokio::test]
async fn test_user_find_by_id() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // Create user
    let created_user = User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Find by ID
    let found_user = User::find_by_id(&pool, user_id).await.unwrap();
    assert!(found_user.is_some());
    
    let found_user = found_user.unwrap();
    assert_eq!(found_user.id, created_user.id);
    assert_eq!(found_user.username, created_user.username);
    assert_eq!(found_user.email, created_user.email);
    
    // Test non-existent user
    let non_existent_id = Uuid::new_v4();
    let not_found = User::find_by_id(&pool, non_existent_id).await.unwrap();
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_user_find_by_github_id() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // Create user
    User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Find by GitHub ID
    let found_user = User::find_by_github_id(&pool, 123456).await.unwrap();
    assert!(found_user.is_some());
    
    let found_user = found_user.unwrap();
    assert_eq!(found_user.github_id, 123456);
    assert_eq!(found_user.username, "testuser");
    
    // Test non-existent GitHub ID
    let not_found = User::find_by_github_id(&pool, 999999).await.unwrap();
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_user_find_by_username() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // Create user
    User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Find by username
    let found_user = User::find_by_username(&pool, "testuser").await.unwrap();
    assert!(found_user.is_some());
    
    let found_user = found_user.unwrap();
    assert_eq!(found_user.username, "testuser");
    assert_eq!(found_user.github_id, 123456);
    
    // Test non-existent username
    let not_found = User::find_by_username(&pool, "nonexistent").await.unwrap();
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_user_update() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // Create user
    let created_user = User::create(&pool, &create_data, user_id).await.unwrap();
    let original_updated_at = created_user.updated_at;
    
    // Wait a bit to ensure updated_at changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    // Update user
    let update_data = UpdateUser {
        username: Some("updated_username".to_string()),
        email: Some("updated@example.com".to_string()),
        display_name: Some("Updated Display Name".to_string()),
        avatar_url: Some("https://github.com/updated.png".to_string()),
        github_token: Some("new_encrypted_token".to_string()),
        is_admin: Some(true),
        is_whitelisted: Some(false),
    };
    
    let updated_user = User::update(&pool, user_id, &update_data).await.unwrap();
    
    assert_eq!(updated_user.id, user_id);
    assert_eq!(updated_user.github_id, 123456); // Should not change
    assert_eq!(updated_user.username, "updated_username");
    assert_eq!(updated_user.email, "updated@example.com");
    assert_eq!(updated_user.display_name, Some("Updated Display Name".to_string()));
    assert_eq!(updated_user.is_admin, true);
    assert_eq!(updated_user.is_whitelisted, false);
    assert!(updated_user.updated_at > original_updated_at);
}

#[tokio::test]
async fn test_user_partial_update() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // Create user
    let created_user = User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Partial update (only username)
    let update_data = UpdateUser {
        username: Some("partially_updated".to_string()),
        email: None,
        display_name: None,
        avatar_url: None,
        github_token: None,
        is_admin: None,
        is_whitelisted: None,
    };
    
    let updated_user = User::update(&pool, user_id, &update_data).await.unwrap();
    
    // Username should change
    assert_eq!(updated_user.username, "partially_updated");
    
    // Other fields should remain the same
    assert_eq!(updated_user.email, created_user.email);
    assert_eq!(updated_user.display_name, created_user.display_name);
    assert_eq!(updated_user.avatar_url, created_user.avatar_url);
    assert_eq!(updated_user.is_admin, created_user.is_admin);
    assert_eq!(updated_user.is_whitelisted, created_user.is_whitelisted);
}

#[tokio::test]
async fn test_user_update_last_login() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // Create user
    let created_user = User::create(&pool, &create_data, user_id).await.unwrap();
    assert!(created_user.last_login_at.is_none());
    
    // Update last login
    User::update_last_login(&pool, user_id).await.unwrap();
    
    // Verify last login was updated
    let updated_user = User::find_by_id(&pool, user_id).await.unwrap().unwrap();
    assert!(updated_user.last_login_at.is_some());
    assert!(updated_user.last_login_at.unwrap() > created_user.created_at);
}

#[tokio::test]
async fn test_user_delete() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // Create user
    User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Verify user exists
    let found_user = User::find_by_id(&pool, user_id).await.unwrap();
    assert!(found_user.is_some());
    
    // Delete user
    let rows_affected = User::delete(&pool, user_id).await.unwrap();
    assert_eq!(rows_affected, 1);
    
    // Verify user no longer exists
    let not_found = User::find_by_id(&pool, user_id).await.unwrap();
    assert!(not_found.is_none());
    
    // Delete non-existent user should affect 0 rows
    let rows_affected = User::delete(&pool, user_id).await.unwrap();
    assert_eq!(rows_affected, 0);
}

#[tokio::test]
async fn test_user_exists() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // User should not exist initially
    let exists = User::exists(&pool, user_id).await.unwrap();
    assert!(!exists);
    
    // Create user
    User::create(&pool, &create_data, user_id).await.unwrap();
    
    // User should now exist
    let exists = User::exists(&pool, user_id).await.unwrap();
    assert!(exists);
}

#[tokio::test]
async fn test_user_find_all() {
    let pool = setup_test_db().await;
    
    // Initially should be empty
    let users = User::find_all(&pool).await.unwrap();
    assert_eq!(users.len(), 0);
    
    // Create multiple users
    let user1_data = CreateUser {
        github_id: 111,
        username: "user1".to_string(),
        email: "user1@example.com".to_string(),
        display_name: Some("User One".to_string()),
        avatar_url: None,
        github_token: None,
        is_admin: Some(false),
    };
    
    let user2_data = CreateUser {
        github_id: 222,
        username: "user2".to_string(),
        email: "user2@example.com".to_string(),
        display_name: Some("User Two".to_string()),
        avatar_url: None,
        github_token: None,
        is_admin: Some(true),
    };
    
    User::create(&pool, &user1_data, Uuid::new_v4()).await.unwrap();
    User::create(&pool, &user2_data, Uuid::new_v4()).await.unwrap();
    
    // Should now have 2 users
    let users = User::find_all(&pool).await.unwrap();
    assert_eq!(users.len(), 2);
    
    // Should be ordered by created_at DESC
    assert_eq!(users[0].username, "user2"); // Most recent first
    assert_eq!(users[1].username, "user1");
}

#[tokio::test]
async fn test_admin_user_management() {
    let pool = setup_test_db().await;
    
    // Create regular user
    let regular_user_data = CreateUser {
        github_id: 111,
        username: "regular".to_string(),
        email: "regular@example.com".to_string(),
        display_name: None,
        avatar_url: None,
        github_token: None,
        is_admin: Some(false),
    };
    
    // Create admin user
    let admin_user_data = CreateUser {
        github_id: 222,
        username: "admin".to_string(),
        email: "admin@example.com".to_string(),
        display_name: None,
        avatar_url: None,
        github_token: None,
        is_admin: Some(true),
    };
    
    User::create(&pool, &regular_user_data, Uuid::new_v4()).await.unwrap();
    User::create(&pool, &admin_user_data, Uuid::new_v4()).await.unwrap();
    
    // Find admin users
    let admins = User::find_admins(&pool).await.unwrap();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins[0].username, "admin");
    assert!(admins[0].is_admin);
    
    // Get first admin
    let first_admin = User::get_first_admin(&pool).await.unwrap();
    assert!(first_admin.is_some());
    assert_eq!(first_admin.unwrap().username, "admin");
}

#[tokio::test]
async fn test_github_whitelist_validation() {
    let pool = setup_test_db().await;
    
    // Initially, no user should be whitelisted
    let is_whitelisted = User::is_github_id_whitelisted(&pool, 123456).await.unwrap();
    assert!(!is_whitelisted);
    
    // Add user to whitelist via database
    sqlx::query!(
        "INSERT INTO github_whitelist (id, github_username, github_id, is_active) VALUES (?, ?, ?, ?)",
        Uuid::new_v4().to_string(),
        "testuser",
        123456,
        true
    )
    .execute(&pool)
    .await
    .unwrap();
    
    // User should now be whitelisted
    let is_whitelisted = User::is_github_id_whitelisted(&pool, 123456).await.unwrap();
    assert!(is_whitelisted);
    
    // Test username-based whitelist check
    let is_whitelisted = User::is_github_username_whitelisted(&pool, "testuser").await.unwrap();
    assert!(is_whitelisted);
    
    // Non-whitelisted user
    let is_whitelisted = User::is_github_username_whitelisted(&pool, "nonexistent").await.unwrap();
    assert!(!is_whitelisted);
}

#[tokio::test]
async fn test_existing_user_whitelist_override() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    
    // Create user with is_whitelisted = true
    let create_data = create_test_user_data();
    let user = User::create(&pool, &create_data, user_id).await.unwrap();
    assert!(user.is_whitelisted);
    
    // User whitelist status should override database whitelist
    let is_whitelisted = User::is_github_id_whitelisted(&pool, user.github_id).await.unwrap();
    assert!(is_whitelisted);
    
    // Update user to not whitelisted
    let update_data = UpdateUser {
        username: None,
        email: None,
        display_name: None,
        avatar_url: None,
        github_token: None,
        is_admin: None,
        is_whitelisted: Some(false),
    };
    
    User::update(&pool, user_id, &update_data).await.unwrap();
    
    // Should now not be whitelisted
    let is_whitelisted = User::is_github_id_whitelisted(&pool, user.github_id).await.unwrap();
    assert!(!is_whitelisted);
}

#[tokio::test]
async fn test_user_creation_with_admin_flag() {
    let pool = setup_test_db().await;
    
    // Create user with admin flag explicitly set to true
    let admin_data = CreateUser {
        github_id: 123456,
        username: "admin_user".to_string(),
        email: "admin@example.com".to_string(),
        display_name: Some("Admin User".to_string()),
        avatar_url: None,
        github_token: None,
        is_admin: Some(true),
    };
    
    let admin_user = User::create(&pool, &admin_data, Uuid::new_v4()).await.unwrap();
    assert!(admin_user.is_admin);
    
    // Create user with admin flag explicitly set to false
    let regular_data = CreateUser {
        github_id: 789012,
        username: "regular_user".to_string(),
        email: "regular@example.com".to_string(),
        display_name: Some("Regular User".to_string()),
        avatar_url: None,
        github_token: None,
        is_admin: Some(false),
    };
    
    let regular_user = User::create(&pool, &regular_data, Uuid::new_v4()).await.unwrap();
    assert!(!regular_user.is_admin);
    
    // Create user with admin flag not set (should default to false)
    let default_data = CreateUser {
        github_id: 345678,
        username: "default_user".to_string(),
        email: "default@example.com".to_string(),
        display_name: Some("Default User".to_string()),
        avatar_url: None,
        github_token: None,
        is_admin: None,
    };
    
    let default_user = User::create(&pool, &default_data, Uuid::new_v4()).await.unwrap();
    assert!(!default_user.is_admin); // Should default to false
}

#[tokio::test]
async fn test_user_unique_constraints() {
    let pool = setup_test_db().await;
    let user_id1 = Uuid::new_v4();
    let user_id2 = Uuid::new_v4();
    
    // Create first user
    let create_data1 = create_test_user_data();
    User::create(&pool, &create_data1, user_id1).await.unwrap();
    
    // Attempt to create second user with same GitHub ID should fail
    let create_data2 = CreateUser {
        github_id: 123456, // Same GitHub ID
        username: "different_username".to_string(),
        email: "different@example.com".to_string(),
        display_name: Some("Different User".to_string()),
        avatar_url: None,
        github_token: None,
        is_admin: Some(false),
    };
    
    let result = User::create(&pool, &create_data2, user_id2).await;
    assert!(result.is_err()); // Should fail due to unique constraint on github_id
}

#[tokio::test]
async fn test_concurrent_user_operations() {
    let pool = setup_test_db().await;
    let user_id = Uuid::new_v4();
    let create_data = create_test_user_data();
    
    // Create user
    User::create(&pool, &create_data, user_id).await.unwrap();
    
    // Simulate concurrent last login updates
    let pool1 = pool.clone();
    let pool2 = pool.clone();
    
    let task1 = tokio::spawn(async move {
        User::update_last_login(&pool1, user_id).await
    });
    
    let task2 = tokio::spawn(async move {
        User::update_last_login(&pool2, user_id).await
    });
    
    // Both should succeed
    let result1 = task1.await.unwrap();
    let result2 = task2.await.unwrap();
    
    assert!(result1.is_ok());
    assert!(result2.is_ok());
    
    // Verify user still exists and has valid last_login_at
    let user = User::find_by_id(&pool, user_id).await.unwrap().unwrap();
    assert!(user.last_login_at.is_some());
}

#[tokio::test]
async fn test_user_data_validation() {
    let pool = setup_test_db().await;
    
    // Test with empty strings (should be allowed)
    let empty_string_data = CreateUser {
        github_id: 123456,
        username: "".to_string(),
        email: "".to_string(),
        display_name: Some("".to_string()),
        avatar_url: Some("".to_string()),
        github_token: Some("".to_string()),
        is_admin: Some(false),
    };
    
    // This should succeed (database doesn't enforce non-empty strings)
    let result = User::create(&pool, &empty_string_data, Uuid::new_v4()).await;
    assert!(result.is_ok());
}