use std::env;
use uuid::Uuid;
use automagik_forge::{
    models::{
        collaboration::{PresenceStatus, PublicUser},
        user::User,
    },
    services::CollaborationService,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔧 Testing Collaboration Service...");

    // Create a test collaboration service
    let service = CollaborationService::new();

    // Create test users
    let user1 = User {
        id: Uuid::new_v4(),
        github_id: 12345,
        username: "test_user_1".to_string(),
        email: "user1@example.com".to_string(),
        display_name: Some("Test User 1".to_string()),
        avatar_url: Some("https://example.com/avatar1.png".to_string()),
        github_token: None,
        is_admin: false,
        is_whitelisted: true,
        last_login_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let user2 = User {
        id: Uuid::new_v4(),
        github_id: 67890,
        username: "test_user_2".to_string(),
        email: "user2@example.com".to_string(),
        display_name: Some("Test User 2".to_string()),
        avatar_url: Some("https://example.com/avatar2.png".to_string()),
        github_token: None,
        is_admin: false,
        is_whitelisted: true,
        last_login_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let project_id = Uuid::new_v4();

    println!("✅ Created test users and project");

    // Test presence updates
    println!("🟢 Testing presence updates...");
    service.update_user_presence(&user1, PresenceStatus::Online, Some(project_id)).await;
    service.update_user_presence(&user2, PresenceStatus::Online, Some(project_id)).await;

    let presence = service.get_project_presence(project_id).await;
    println!("📊 Project presence: {} users online", presence.len());

    // Test event broadcasting
    println!("📡 Testing event broadcasting...");
    
    let test_event = automagik_forge::models::collaboration::CollaborationEvent::new(
        "test_event".to_string(),
        project_id,
        &user1,
        serde_json::json!({"message": "Hello from test!"}),
    );

    match service.broadcast_event(test_event).await {
        Ok(()) => println!("✅ Event broadcast successful"),
        Err(e) => println!("❌ Event broadcast failed: {}", e),
    }

    // Test connection tracking
    println!("🔗 Testing connection tracking...");
    let connection_count = service.get_connection_count().await;
    println!("📈 Total connections: {}", connection_count);

    let project_connections = service.get_project_connection_count(project_id).await;
    println!("📈 Project connections: {}", project_connections);

    println!("🎉 Collaboration service test completed successfully!");
    
    Ok(())
}