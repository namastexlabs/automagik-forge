use anyhow::Result;
use rmcp::{service::ServiceExt, transport::TokioChildProcess, transport::ConfigureCommandExt};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();
    
    // Initialize logging with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    println!("🔍 Listing available MCP tools from Evolution API server...");
    
    // Create STDIO transport using TokioChildProcess for MCP evolution-api tool
    let transport = TokioChildProcess::new(Command::new("uvx").configure(|cmd| {
        cmd.arg("automagik-tools@0.8.15")
            .arg("tool")
            .arg("evolution-api")
            .env("EVOLUTION_API_BASE_URL", std::env::var("EVOLUTION_API_BASE_URL").unwrap_or_default())
            .env("EVOLUTION_API_API_KEY", std::env::var("EVOLUTION_API_API_KEY").unwrap_or_default())
            .env("EVOLUTION_API_INSTANCE", std::env::var("EVOLUTION_API_INSTANCE").unwrap_or_default());
    }))
    .map_err(|e| anyhow::anyhow!("Failed to create STDIO transport: {}", e))?;
    
    // Create service
    let service = ().serve(transport).await
        .map_err(|e| anyhow::anyhow!("Failed to create MCP service: {}", e))?;
    
    println!("📋 Server info: {:?}", service.peer_info());
    
    // List available tools
    let tools = service.list_tools(Default::default()).await
        .map_err(|e| anyhow::anyhow!("Failed to list tools: {}", e))?;
    
    println!("🛠️  Available tools:");
    for tool in &tools.tools {
        println!("  • {} - {}", tool.name, tool.description.as_deref().unwrap_or("No description"));
        println!("    Input schema: {}", serde_json::to_string_pretty(&tool.input_schema).unwrap_or_default());
    }
    
    // Cancel the service to clean up
    service.cancel().await
        .map_err(|e| anyhow::anyhow!("Failed to cancel MCP service: {}", e))?;
    
    println!("✅ Tool listing complete!");
    Ok(())
}