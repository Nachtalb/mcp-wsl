use super::{ToolArgs, ToolResult};
use std::fs;

pub async fn get_wsl_config(_args: ToolArgs) -> ToolResult {
    fs::read_to_string("/etc/wsl.conf")
        .map_err(|e| format!("Failed to read /etc/wsl.conf: {e}"))
}
