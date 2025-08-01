use std::env;

#[derive(Debug, Clone)]
pub struct WhatsAppConfig {
    pub base_url: String,
    pub api_key: String,
    pub instance: String,
    pub fixed_recipient: Option<String>,
    pub timeout_ms: u64,
    pub include_task_url: bool,
}

impl WhatsAppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            base_url: env::var("EVOLUTION_API_BASE_URL")
                .map_err(|_| anyhow::anyhow!("EVOLUTION_API_BASE_URL not set"))?,
            api_key: env::var("EVOLUTION_API_API_KEY")
                .map_err(|_| anyhow::anyhow!("EVOLUTION_API_API_KEY not set"))?,
            instance: env::var("EVOLUTION_API_INSTANCE")
                .map_err(|_| anyhow::anyhow!("EVOLUTION_API_INSTANCE not set"))?,
            fixed_recipient: env::var("EVOLUTION_API_FIXED_RECIPIENT").ok(),
            timeout_ms: env::var("WHATSAPP_MCP_SERVER_TIMEOUT")
                .unwrap_or_else(|_| "30000".to_string())
                .parse()
                .unwrap_or(30000),
            include_task_url: env::var("WHATSAPP_NOTIFICATION_INCLUDE_URL")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        })
    }
}