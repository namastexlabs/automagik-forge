use automagik_forge::services::{WhatsAppConfig, WhatsAppNotifier};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();
    
    // Initialize logging with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    println!("🧪 Testing WhatsApp notification integration...");
    
    // Load configuration from environment
    match WhatsAppConfig::from_env() {
        Ok(config) => {
            println!("✅ WhatsApp configuration loaded successfully");
            println!("   Base URL: {}", config.base_url);
            println!("   Instance: {}", config.instance);
            println!("   Timeout: {}ms", config.timeout_ms);
            
            // Create notifier
            let notifier = WhatsAppNotifier::new(config).await?;
            
            // Send test notification
            println!("📱 Sending test WhatsApp notification...");
            println!("   Message: 'Automagik-Forge Test: 🚀 WhatsApp integration is working!'");
            
            match notifier.send_notification(
                "Automagik-Forge Test", 
                "🚀 WhatsApp integration is working! This is a test message from automagik-forge with rmcp 0.3.2"
            ).await {
                Ok(()) => {
                    println!("✅ Test completed successfully!");
                    println!("📱 Check your WhatsApp for the message!");
                }
                Err(e) => {
                    println!("❌ Failed to send WhatsApp notification: {}", e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to load WhatsApp configuration: {}", e);
            println!("💡 Make sure you have set the required environment variables:");
            println!("   - EVOLUTION_API_BASE_URL");
            println!("   - EVOLUTION_API_API_KEY");
            println!("   - EVOLUTION_API_INSTANCE");
            println!("   - EVOLUTION_API_FIXED_RECIPIENT (optional)");
            return Err(e);
        }
    }
    
    Ok(())
}