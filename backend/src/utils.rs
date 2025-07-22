use std::{env, sync::OnceLock};

use directories::ProjectDirs;

pub mod path;
pub mod shell;
pub mod text;
pub mod worktree_manager;

const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Cache for WSL2 detection result
static WSL2_CACHE: OnceLock<bool> = OnceLock::new();

/// Check if running in WSL2 (cached)
pub fn is_wsl2() -> bool {
    *WSL2_CACHE.get_or_init(|| {
        // Check for WSL environment variables
        if std::env::var("WSL_DISTRO_NAME").is_ok() || std::env::var("WSLENV").is_ok() {
            tracing::debug!("WSL2 detected via environment variables");
            return true;
        }

        // Check /proc/version for WSL2 signature
        if let Ok(version) = std::fs::read_to_string("/proc/version") {
            if version.contains("WSL2") || version.contains("microsoft") {
                tracing::debug!("WSL2 detected via /proc/version");
                return true;
            }
        }

        tracing::debug!("WSL2 not detected");
        false
    })
}

pub fn asset_dir() -> std::path::PathBuf {
    if cfg!(debug_assertions) {
        std::path::PathBuf::from(PROJECT_ROOT).join("../dev_assets")
    } else if cfg!(target_os = "windows") {
        dirs::data_dir()
            .expect("Could not find data directory")
            .join("automagik-forge")
    } else {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".automagik-forge")
    }

    // ✔ Linux/macOS → ~/.automagik-forge
    // ✔ Windows → %APPDATA%\automagik-forge
}

pub fn config_path() -> std::path::PathBuf {
    asset_dir().join("config.json")
}

pub fn cache_dir() -> std::path::PathBuf {
    if cfg!(debug_assertions) {
        std::path::PathBuf::from(PROJECT_ROOT).join("../dev_assets/.cache")
    } else if cfg!(target_os = "windows") {
        dirs::cache_dir()
            .expect("Could not find cache directory")
            .join("automagik-forge")
    } else {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".automagik-forge")
            .join("cache")
    }

    // ✔ Linux/macOS → ~/.automagik-forge/cache
    // ✔ Windows → %LOCALAPPDATA%\automagik-forge
}

/// Get or create cached PowerShell script file
pub async fn get_powershell_script(
) -> Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;

    let cache_dir = cache_dir();
    let script_path = cache_dir.join("toast-notification.ps1");

    // Check if cached file already exists and is valid
    if script_path.exists() {
        // Verify file has content (basic validation)
        if let Ok(metadata) = std::fs::metadata(&script_path) {
            if metadata.len() > 0 {
                return Ok(script_path);
            }
        }
    }

    // File doesn't exist or is invalid, create it
    let script_content = crate::ScriptAssets::get("toast-notification.ps1")
        .ok_or("Embedded PowerShell script not found: toast-notification.ps1")?
        .data;

    // Ensure cache directory exists
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache directory: {}", e))?;

    let mut file = std::fs::File::create(&script_path)
        .map_err(|e| format!("Failed to create PowerShell script file: {}", e))?;

    file.write_all(&script_content)
        .map_err(|e| format!("Failed to write PowerShell script data: {}", e))?;

    drop(file); // Ensure file is closed

    Ok(script_path)
}

/// Open URL in browser with WSL2 support
pub async fn open_browser(url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if is_wsl2() {
        // In WSL2, use PowerShell to open the browser
        tokio::process::Command::new("powershell.exe")
            .arg("-Command")
            .arg(format!("Start-Process '{}'", url))
            .spawn()?;
        Ok(())
    } else {
        // Use the standard open crate for other platforms
        open::that(url).map_err(|e| e.into())
    }
}
