use super::{ToolArgs, ToolResult};
use std::fs;
use tokio::process::Command;

pub async fn get_mounts(_args: ToolArgs) -> ToolResult {
    if let Ok(content) = fs::read_to_string("/proc/mounts") {
        return Ok(content);
    }
    let out = Command::new("mount")
        .output()
        .await
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub async fn get_disk_usage(args: ToolArgs) -> ToolResult {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("/");

    let out = Command::new("df")
        .arg("-h")
        .arg(path)
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
