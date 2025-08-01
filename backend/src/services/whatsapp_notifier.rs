use crate::services::whatsapp_config::WhatsAppConfig;
use anyhow::Result;
use rmcp::{model::CallToolRequestParam, service::ServiceExt, transport::TokioChildProcess, transport::ConfigureCommandExt};
use serde_json::json;
use tokio::process::Command;
use std::time::Duration;

#[derive(Debug)]
pub struct WhatsAppNotifier {
    config: WhatsAppConfig,
}

impl WhatsAppNotifier {
    pub async fn new(config: WhatsAppConfig) -> Result<Self> {
        tracing::info!("Initializing WhatsApp notifier with base URL: {}", config.base_url);
        Ok(Self { config })
    }
    
    pub async fn send_notification(&self, title: &str, message: &str) -> Result<()> {
        tracing::info!("Attempting to send WhatsApp notification: {}", title);
        
        // Format message for WhatsApp with optional URL
        let formatted_message = format!("*{}*\n\n{}", title, message);
        
        // Use include_task_url for future URL functionality
        let _include_url = self.config.include_task_url;
        
        // Prepare WhatsApp message parameters
        let params = json!({
            "instance": self.config.instance,
            "message": formatted_message,
            "number": self.config.fixed_recipient,
            "linkPreview": true,
            "delay": 0
        });
        
        tracing::debug!("WhatsApp message params: {}", serde_json::to_string_pretty(&params).unwrap_or_default());
        
        // Create SSE transport and service for this request
        match self.send_via_mcp(&params).await {
            Ok(_) => {
                tracing::info!("WhatsApp notification sent successfully: {}", title);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to send WhatsApp notification: {}. Notification will be skipped.", e);
                Ok(()) // Don't fail the entire operation if notification fails
            }
        }
    }
    
    async fn send_via_mcp(&self, params: &serde_json::Value) -> Result<()> {
        // Create STDIO transport using TokioChildProcess for MCP evolution-api tool
        let transport = TokioChildProcess::new(Command::new("uvx").configure(|cmd| {
            cmd.arg("automagik-tools@0.8.15")
                .arg("tool")
                .arg("evolution-api")
                .env("EVOLUTION_API_BASE_URL", &self.config.base_url)
                .env("EVOLUTION_API_API_KEY", &self.config.api_key)
                .env("EVOLUTION_API_INSTANCE", &self.config.instance);
            
            // Set fixed recipient if configured
            if let Some(ref fixed_recipient) = self.config.fixed_recipient {
                if !fixed_recipient.is_empty() {
                    cmd.env("EVOLUTION_API_FIXED_RECIPIENT", fixed_recipient);
                }
            }
        }))
        .map_err(|e| anyhow::anyhow!("Failed to create STDIO transport: {}", e))?;
        
        // Create service with timeout
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);
        let service_future = ().serve(transport);
        let service = tokio::time::timeout(timeout_duration, service_future).await
            .map_err(|_| anyhow::anyhow!("MCP service creation timed out after {}ms", self.config.timeout_ms))?
            .map_err(|e| anyhow::anyhow!("Failed to create MCP service: {}", e))?;
        
        // Call WhatsApp MCP tool with timeout
        let tool_call_future = service.call_tool(CallToolRequestParam {
            name: "send_text_message".into(),
            arguments: params.as_object().cloned(),
        });
        let result = tokio::time::timeout(timeout_duration, tool_call_future).await
            .map_err(|_| anyhow::anyhow!("MCP tool call timed out after {}ms", self.config.timeout_ms))?
            .map_err(|e| anyhow::anyhow!("Failed to call MCP tool: {}", e))?;
        
        tracing::debug!("MCP tool result: {:?}", result);
        
        // Cancel the service to clean up
        let cancel_future = service.cancel();
        let _ = tokio::time::timeout(Duration::from_millis(5000), cancel_future).await;
        
        Ok(())
    }
}

