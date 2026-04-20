use super::{ToolArgs, ToolResult};
use std::fs;
use tokio::process::Command;

pub async fn get_shells(_args: ToolArgs) -> ToolResult {
    match fs::read_to_string("/etc/shells") {
        Ok(content) => {
            let shells: Vec<&str> = content
                .lines()
                .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                .collect();
            Ok(shells.join("\n"))
        }
        Err(e) => Err(format!("Failed to read /etc/shells: {e}")),
    }
}

pub async fn get_default_shell(_args: ToolArgs) -> ToolResult {
    if let Ok(shell) = std::env::var("SHELL") {
        return Ok(shell);
    }

    let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
    let out = Command::new("getent")
        .args(["passwd", &user])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if out.status.success() {
        let line = String::from_utf8_lossy(&out.stdout);
        if let Some(shell) = line.trim().split(':').nth(6) {
            if !shell.is_empty() {
                return Ok(shell.to_string());
            }
        }
    }

    Err("Could not determine default shell".to_string())
}
