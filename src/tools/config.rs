use super::{ToolArgs, ToolResult};
use std::fs;

pub fn is_wsl() -> bool {
    fs::read_to_string("/proc/version")
        .map(|v| v.to_ascii_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

pub async fn get_wsl_config(_args: ToolArgs) -> ToolResult {
    if !is_wsl() {
        return Err("read:get_wsl_config is only available on WSL".to_string());
    }
    fs::read_to_string("/etc/wsl.conf")
        .map_err(|e| format!("Failed to read /etc/wsl.conf: {e}"))
}
