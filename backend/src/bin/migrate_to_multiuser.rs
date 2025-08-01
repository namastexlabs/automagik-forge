use anyhow::Result;
use automagik_forge::models::config::Config;
use automagik_forge::models::user::{CreateUser, User};
// use automagik_forge::models::user_preferences::{CreateUserPreferences, UserPreferences};
use sqlx::SqlitePool;
use std::path::PathBuf;
use uuid::Uuid;

/// Data migration helper to set up initial admin user from existing config.json
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <config_path> <database_url>", args[0]);
        eprintln!("Example: {} ./dev_assets/config.json sqlite:./dev_assets/db.sqlite", args[0]);
        std::process::exit(1);
    }

    let config_path = PathBuf::from(&args[1]);
    let database_url = &args[2];

    println!("🔄 Starting multiuser data migration...");
    println!("📄 Config path: {}", config_path.display());
    println!("🗄️ Database URL: {}", database_url);

    // Connect to database
    let pool = SqlitePool::connect(database_url).await?;
    
    // Run the migration
    migrate_data(&pool, &config_path).await?;
    
    println!("✅ Multiuser data migration completed successfully!");
    Ok(())
}

async fn migrate_data(pool: &SqlitePool, config_path: &PathBuf) -> Result<()> {
    // Step 1: Load existing config.json
    let config = if config_path.exists() {
        println!("📖 Loading existing config from: {}", config_path.display());
        Config::load(config_path)?
    } else {
        println!("⚠️ Config file not found, using defaults");
        Config::default()
    };

    // Step 2: Check if admin user already exists
    let existing_admin = User::find_admins(pool).await?;
    if !existing_admin.is_empty() {
        println!("ℹ️ Admin user already exists, skipping user creation");
        return Ok(());
    }

    // Step 3: Create admin user from config
    let admin_user = create_admin_user_from_config(pool, &config).await?;
    println!("👤 Created admin user: {} ({})", admin_user.username, admin_user.email);

    // Step 4: Create user preferences from config (temporarily disabled)
    // let preferences = create_preferences_from_config(pool, admin_user.id, &config).await?;
    println!("⚙️ Skipped user preferences creation (temporarily disabled)");

    // Step 5: Update existing data to be owned by admin user
    update_existing_data_ownership(pool, admin_user.id).await?;
    println!("📝 Updated existing data ownership to admin user");

    // Step 6: Create backup of config.json
    if config_path.exists() {
        backup_config_file(config_path)?;
        println!("💾 Backed up original config.json");
    }

    Ok(())
}

async fn create_admin_user_from_config(pool: &SqlitePool, config: &Config) -> Result<User> {
    // Extract GitHub information from config
    let github_username = config.github.username.clone().unwrap_or_else(|| "admin".to_string());
    let github_id = 0; // Placeholder, will be updated when GitHub auth is implemented
    let email = config.github.primary_email.clone().unwrap_or_else(|| format!("{}@localhost", github_username));
    let display_name = Some("System Administrator".to_string());

    let create_data = CreateUser {
        github_id,
        username: github_username.clone(),
        email: email.clone(),
        display_name,
        avatar_url: None,
        github_token: config.github.token.clone(),
        is_admin: Some(true),
    };

    let user_id = Uuid::new_v4();
    let user = User::create(pool, &create_data, user_id).await?;

    // If we have a real GitHub username, add it to the whitelist
    if github_username != "admin" {
        use automagik_forge::models::github_whitelist::GitHubWhitelist;
        GitHubWhitelist::add_username(
            pool,
            &github_username,
            Some(user.id),
            Some("Initial admin user from config migration".to_string()),
        ).await?;
        println!("✅ Added {} to GitHub whitelist", github_username);
    }

    Ok(user)
}

// async fn create_preferences_from_config(pool: &SqlitePool, user_id: Uuid, config: &Config) -> Result<UserPreferences> {
//     let create_data = UserPreferences::from_legacy_config(user_id, config);
//     let preferences_id = Uuid::new_v4();
//     let preferences = UserPreferences::create(pool, &create_data, preferences_id).await?;
//     Ok(preferences)
// }

async fn update_existing_data_ownership(pool: &SqlitePool, admin_user_id: Uuid) -> Result<()> {
    // Update projects
    let projects_updated = sqlx::query!(
        "UPDATE projects SET created_by = $1 WHERE created_by IS NULL",
        admin_user_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    println!("📁 Updated {} projects to admin ownership", projects_updated);

    // Update tasks
    let tasks_updated = sqlx::query!(
        "UPDATE tasks SET created_by = $1 WHERE created_by IS NULL",
        admin_user_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    println!("📋 Updated {} tasks to admin ownership", tasks_updated);

    // Update task attempts
    let attempts_updated = sqlx::query!(
        "UPDATE task_attempts SET created_by = $1 WHERE created_by IS NULL",
        admin_user_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    println!("🔄 Updated {} task attempts to admin ownership", attempts_updated);

    Ok(())
}

fn backup_config_file(config_path: &PathBuf) -> Result<()> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("config_pre_multiuser_backup_{}.json", timestamp);
    
    let backup_path = config_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(backup_filename);

    std::fs::copy(config_path, &backup_path)?;
    println!("💾 Config backed up to: {}", backup_path.display());
    Ok(())
}