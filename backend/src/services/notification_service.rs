use std::sync::OnceLock;

use crate::models::config::SoundFile;
use crate::services::{WhatsAppConfig, WhatsAppNotifier};

/// Service for handling cross-platform notifications including sound alerts and push notifications
#[derive(Debug)]
pub struct NotificationService {
    sound_enabled: bool,
    push_enabled: bool,
    whatsapp_enabled: bool,
    whatsapp_notifier: Option<WhatsAppNotifier>,
}

/// Configuration for notifications
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    pub sound_enabled: bool,
    pub push_enabled: bool,
    pub whatsapp_enabled: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            push_enabled: true,
            whatsapp_enabled: true,
        }
    }
}

/// Cache for WSL root path from PowerShell
static WSL_ROOT_PATH_CACHE: OnceLock<Option<String>> = OnceLock::new();

impl NotificationService {
    /// Create a new NotificationService with the given configuration
    pub async fn new(config: NotificationConfig) -> Self {
        let whatsapp_notifier = if config.whatsapp_enabled {
            match WhatsAppConfig::from_env() {
                Ok(whatsapp_config) => {
                    match WhatsAppNotifier::new(whatsapp_config).await {
                        Ok(notifier) => Some(notifier),
                        Err(e) => {
                            tracing::warn!("Failed to initialize WhatsApp notifier: {}. WhatsApp notifications disabled.", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("WhatsApp configuration not available: {}. WhatsApp notifications disabled.", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            sound_enabled: config.sound_enabled,
            push_enabled: config.push_enabled,
            whatsapp_enabled: config.whatsapp_enabled && whatsapp_notifier.is_some(),
            whatsapp_notifier,
        }
    }

    /// Send sound, push, and WhatsApp notifications if enabled
    pub async fn notify(&self, title: &str, message: &str, sound_file: &SoundFile) {
        if self.sound_enabled {
            self.play_sound_notification(sound_file).await;
        }

        if self.push_enabled {
            self.send_push_notification(title, message).await;
        }

        if self.whatsapp_enabled {
            if let Some(ref whatsapp_notifier) = self.whatsapp_notifier {
                if let Err(e) = whatsapp_notifier.send_notification(title, message).await {
                    tracing::error!("Failed to send WhatsApp notification: {}", e);
                }
            }
        }
    }

    /// Play a system sound notification across platforms
    #[allow(dead_code)]
    pub async fn play_sound_notification(&self, sound_file: &SoundFile) {
        if !self.sound_enabled {
            return;
        }

        let file_path = match sound_file.get_path().await {
            Ok(path) => path,
            Err(e) => {
                tracing::error!("Failed to create cached sound file: {}", e);
                return;
            }
        };

        // Use platform-specific sound notification
        // Note: spawn() calls are intentionally not awaited - sound notifications should be fire-and-forget
        if cfg!(target_os = "macos") {
            let _ = tokio::process::Command::new("afplay")
                .arg(&file_path)
                .spawn();
        } else if cfg!(target_os = "linux") && !crate::utils::is_wsl2() {
            // Try different Linux audio players
            if tokio::process::Command::new("paplay")
                .arg(&file_path)
                .spawn()
                .is_ok()
            {
                // Success with paplay
            } else if tokio::process::Command::new("aplay")
                .arg(&file_path)
                .spawn()
                .is_ok()
            {
                // Success with aplay
            } else {
                // Try system bell as fallback
                let _ = tokio::process::Command::new("echo")
                    .arg("-e")
                    .arg("\\a")
                    .spawn();
            }
        } else if cfg!(target_os = "windows")
            || (cfg!(target_os = "linux") && crate::utils::is_wsl2())
        {
            // Convert WSL path to Windows path if in WSL2
            let file_path = if crate::utils::is_wsl2() {
                if let Some(windows_path) = Self::wsl_to_windows_path(&file_path).await {
                    windows_path
                } else {
                    file_path.to_string_lossy().to_string()
                }
            } else {
                file_path.to_string_lossy().to_string()
            };

            let _ = tokio::process::Command::new("powershell.exe")
                .arg("-c")
                .arg(format!(
                    r#"(New-Object Media.SoundPlayer "{}").PlaySync()"#,
                    file_path
                ))
                .spawn();
        }
    }

    /// Send a cross-platform push notification
    #[allow(dead_code)]
    pub async fn send_push_notification(&self, title: &str, message: &str) {
        if !self.push_enabled {
            return;
        }

        if cfg!(target_os = "macos") {
            self.send_macos_notification(title, message).await;
        } else if cfg!(target_os = "linux") && !crate::utils::is_wsl2() {
            self.send_linux_notification(title, message).await;
        } else if cfg!(target_os = "windows")
            || (cfg!(target_os = "linux") && crate::utils::is_wsl2())
        {
            self.send_windows_notification(title, message).await;
        }
    }

    /// Send macOS notification using osascript
    async fn send_macos_notification(&self, title: &str, message: &str) {
        let script = format!(
            r#"display notification "{message}" with title "{title}" sound name "Glass""#,
            message = message.replace('"', r#"\""#),
            title = title.replace('"', r#"\""#)
        );

        let _ = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();
    }

    /// Send Linux notification using notify-rust
    async fn send_linux_notification(&self, title: &str, message: &str) {
        use notify_rust::Notification;

        let title = title.to_string();
        let message = message.to_string();

        let _handle = tokio::task::spawn_blocking(move || {
            if let Err(e) = Notification::new()
                .summary(&title)
                .body(&message)
                .timeout(10000)
                .show()
            {
                tracing::error!("Failed to send Linux notification: {}", e);
            }
        });
        drop(_handle); // Don't await, fire-and-forget
    }

    /// Send Windows/WSL notification using PowerShell toast script
    async fn send_windows_notification(&self, title: &str, message: &str) {
        let script_path = match crate::utils::get_powershell_script().await {
            Ok(path) => path,
            Err(e) => {
                tracing::error!("Failed to get PowerShell script: {}", e);
                return;
            }
        };

        // Convert WSL path to Windows path if in WSL2
        let script_path_str = if crate::utils::is_wsl2() {
            if let Some(windows_path) = Self::wsl_to_windows_path(&script_path).await {
                windows_path
            } else {
                script_path.to_string_lossy().to_string()
            }
        } else {
            script_path.to_string_lossy().to_string()
        };

        let _ = tokio::process::Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(script_path_str)
            .arg("-Title")
            .arg(title)
            .arg("-Message")
            .arg(message)
            .spawn();
    }

    /// Get WSL root path via PowerShell (cached)
    async fn get_wsl_root_path() -> Option<String> {
        if let Some(cached) = WSL_ROOT_PATH_CACHE.get() {
            return cached.clone();
        }

        match tokio::process::Command::new("powershell.exe")
            .arg("-c")
            .arg("(Get-Location).Path -replace '^.*::', ''")
            .current_dir("/")
            .output()
            .await
        {
            Ok(output) => {
                match String::from_utf8(output.stdout) {
                    Ok(pwd_str) => {
                        let pwd = pwd_str.trim();
                        tracing::info!("WSL root path detected: {}", pwd);

                        // Cache the result
                        let _ = WSL_ROOT_PATH_CACHE.set(Some(pwd.to_string()));
                        return Some(pwd.to_string());
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse PowerShell pwd output as UTF-8: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to execute PowerShell pwd command: {}", e);
            }
        }

        // Cache the failure result
        let _ = WSL_ROOT_PATH_CACHE.set(None);
        None
    }

    /// Convert WSL path to Windows UNC path for PowerShell
    async fn wsl_to_windows_path(wsl_path: &std::path::Path) -> Option<String> {
        let path_str = wsl_path.to_string_lossy();

        // Relative paths work fine as-is in PowerShell
        if !path_str.starts_with('/') {
            tracing::debug!("Using relative path as-is: {}", path_str);
            return Some(path_str.to_string());
        }

        // Get cached WSL root path from PowerShell
        if let Some(wsl_root) = Self::get_wsl_root_path().await {
            // Simply concatenate WSL root with the absolute path - PowerShell doesn't mind /
            let windows_path = format!("{}{}", wsl_root, path_str);
            tracing::debug!("WSL path converted: {} -> {}", path_str, windows_path);
            Some(windows_path)
        } else {
            tracing::error!(
                "Failed to determine WSL root path for conversion: {}",
                path_str
            );
            None
        }
    }
}
