use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::env;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub jwt_secret: String,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub database_url: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            jwt_secret: generate_jwt_secret(),
            github_client_id: Some("Ov23li2nd1KF5nCPbgoj".to_string()),
            github_client_secret: None,
            database_url: None,
        }
    }
}

impl AppConfig {
    /// Load configuration with fallback chain:
    /// 1. Environment variables
    /// 2. ~/.automagik-forge/config.toml  
    /// 3. .env file (if in development)
    /// 4. Default values (with generated JWT secret)
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Self::load_from_file().unwrap_or_default();
        
        // Load from .env file first (lowest priority)
        if let Ok(env_file_path) = find_env_file() {
            Self::load_env_file(&env_file_path);
        }
        
        // Override with environment variables (highest priority)
        if let Ok(jwt_secret) = env::var("JWT_SECRET") {
            if jwt_secret.len() >= 32 {
                config.jwt_secret = jwt_secret;
            }
        }
        
        if let Ok(github_client_id) = env::var("GITHUB_CLIENT_ID") {
            if !github_client_id.is_empty() {
                config.github_client_id = Some(github_client_id);
            }
        }
        
        if let Ok(github_client_secret) = env::var("GITHUB_CLIENT_SECRET") {
            if !github_client_secret.is_empty() {
                config.github_client_secret = Some(github_client_secret);
            }
        }
        
        if let Ok(database_url) = env::var("DATABASE_URL") {
            if !database_url.is_empty() {
                config.database_url = Some(database_url);
            }
        }
        
        // Validate JWT secret length
        if config.jwt_secret.len() < 32 {
            tracing::warn!("JWT_SECRET too short, generating new secure secret");
            config.jwt_secret = generate_jwt_secret();
        }
        
        // Save the configuration to persist any generated values
        config.save()?;
        
        Ok(config)
    }
    
    /// Load configuration from ~/.automagik-forge/config.toml
    fn load_from_file() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = get_config_file_path()?;
        
        if !config_path.exists() {
            return Err("Config file does not exist".into());
        }
        
        let content = fs::read_to_string(&config_path)?;
        let config: AppConfig = toml::from_str(&content)?;
        
        tracing::debug!("Loaded configuration from: {}", config_path.display());
        Ok(config)
    }
    
    /// Save configuration to ~/.automagik-forge/config.toml
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = get_config_file_path()?;
        
        // Ensure the directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        
        tracing::debug!("Saved configuration to: {}", config_path.display());
        Ok(())
    }
    
    /// Load environment variables from .env file
    fn load_env_file(env_path: &PathBuf) {
        if let Ok(content) = fs::read_to_string(env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    
                    // Only set if not already in environment
                    if env::var(key).is_err() {
                        env::set_var(key, value);
                    }
                }
            }
            tracing::debug!("Loaded environment variables from: {}", env_path.display());
        }
    }
}

/// Get the path to the configuration file
fn get_config_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home_dir = dirs::home_dir()
        .ok_or("Could not determine home directory")?;
    
    let config_dir = home_dir.join(".automagik-forge");
    Ok(config_dir.join("config.toml"))
}

/// Find .env file in current directory or parent directories
fn find_env_file() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut current_dir = env::current_dir()?;
    
    loop {
        let env_path = current_dir.join(".env");
        if env_path.exists() {
            return Ok(env_path);
        }
        
        if let Some(parent) = current_dir.parent() {
            current_dir = parent.to_path_buf();
        } else {
            break;
        }
    }
    
    Err("No .env file found".into())
}

/// Generate a cryptographically secure JWT secret
fn generate_jwt_secret() -> String {
    format!("{}{}{}", 
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_config_default() {
        let config = AppConfig::default();
        assert!(config.jwt_secret.len() >= 32);
        assert!(config.github_client_id.is_none());
    }
    
    #[test]
    fn test_jwt_secret_generation() {
        let secret1 = generate_jwt_secret();
        let secret2 = generate_jwt_secret();
        
        assert!(secret1.len() >= 32);
        assert!(secret2.len() >= 32);
        assert_ne!(secret1, secret2);
    }
    
    #[test]
    fn test_config_serialization() {
        let config = AppConfig {
            jwt_secret: "test-jwt-secret-with-32-characters".to_string(),
            github_client_id: Some("test-client-id".to_string()),
            github_client_secret: Some("test-client-secret".to_string()),
            database_url: Some("sqlite:test.db".to_string()),
        };
        
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        
        assert_eq!(config.jwt_secret, deserialized.jwt_secret);
        assert_eq!(config.github_client_id, deserialized.github_client_id);
    }
}