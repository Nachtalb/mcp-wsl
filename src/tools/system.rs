use super::{ToolArgs, ToolResult};
use std::fs;
use tokio::process::Command;

pub async fn get_system_info(_args: ToolArgs) -> ToolResult {
    let out = Command::new("uname")
        .arg("-a")
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub async fn get_os_info(_args: ToolArgs) -> ToolResult {
    for path in &["/etc/os-release", "/usr/lib/os-release", "/etc/lsb-release"] {
        if let Ok(content) = fs::read_to_string(path) {
            return Ok(format!("# {path}\n{content}"));
        }
    }
    Err("No OS release file found".to_string())
}
