use super::{ToolArgs, ToolResult};
use serde_json::json;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub async fn execute_command(args: ToolArgs) -> ToolResult {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("command parameter is required")?
        .to_string();

    let cmd_args: Vec<String> = args
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let stdin_text = args.get("stdin").and_then(|v| v.as_str()).map(String::from);
    let stdin_file = args.get("stdin_file").and_then(|v| v.as_str()).map(String::from);
    let stdout_file = args.get("stdout_file").and_then(|v| v.as_str()).map(String::from);
    let stderr_file = args.get("stderr_file").and_then(|v| v.as_str()).map(String::from);
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30);
    let working_dir = args.get("working_dir").and_then(|v| v.as_str()).map(String::from);

    let mut cmd = Command::new(&command);
    cmd.args(&cmd_args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    if stdin_text.is_some() || stdin_file.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }
    if let Some(ref dir) = working_dir {
        cmd.current_dir(dir);
    }

    let result = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn '{command}': {e}"))?;

        if let Some(ref text) = stdin_text {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes()).await.ok();
            }
        } else if let Some(ref path) = stdin_file {
            let content = tokio::fs::read(path)
                .await
                .map_err(|e| format!("stdin_file read error: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&content).await.ok();
            }
        }

        child
            .wait_with_output()
            .await
            .map_err(|e| format!("Wait error: {e}"))
    })
    .await;

    let output = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(format!("Command timed out after {timeout_secs}s")),
    };

    if let Some(ref path) = stdout_file {
        tokio::fs::write(path, &output.stdout)
            .await
            .map_err(|e| format!("stdout_file write error: {e}"))?;
    }
    if let Some(ref path) = stderr_file {
        tokio::fs::write(path, &output.stderr)
            .await
            .map_err(|e| format!("stderr_file write error: {e}"))?;
    }

    let result = json!({
        "status": output.status.code().unwrap_or(-1),
        "stdout": if stdout_file.is_none() { String::from_utf8_lossy(&output.stdout).to_string() } else { String::new() },
        "stderr": if stderr_file.is_none() { String::from_utf8_lossy(&output.stderr).to_string() } else { String::new() },
    });

    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

pub async fn execute_shell_command(args: ToolArgs) -> ToolResult {
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("/bin/sh")
        .to_string();

    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("command parameter is required")?
        .to_string();

    let stdin_text = args.get("stdin").and_then(|v| v.as_str()).map(String::from);
    let stdout_file = args.get("stdout_file").and_then(|v| v.as_str()).map(String::from);
    let stderr_file = args.get("stderr_file").and_then(|v| v.as_str()).map(String::from);
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30);
    let working_dir = args.get("working_dir").and_then(|v| v.as_str()).map(String::from);

    let mut cmd = Command::new(&shell);
    cmd.arg("-c").arg(&command);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    if stdin_text.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }
    if let Some(ref dir) = working_dir {
        cmd.current_dir(dir);
    }

    let result = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn shell '{shell}': {e}"))?;

        if let Some(ref text) = stdin_text {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes()).await.ok();
            }
        }

        child
            .wait_with_output()
            .await
            .map_err(|e| format!("Wait error: {e}"))
    })
    .await;

    let output = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(format!("Command timed out after {timeout_secs}s")),
    };

    if let Some(ref path) = stdout_file {
        tokio::fs::write(path, &output.stdout)
            .await
            .map_err(|e| format!("stdout_file write error: {e}"))?;
    }
    if let Some(ref path) = stderr_file {
        tokio::fs::write(path, &output.stderr)
            .await
            .map_err(|e| format!("stderr_file write error: {e}"))?;
    }

    let result = json!({
        "status": output.status.code().unwrap_or(-1),
        "stdout": if stdout_file.is_none() { String::from_utf8_lossy(&output.stdout).to_string() } else { String::new() },
        "stderr": if stderr_file.is_none() { String::from_utf8_lossy(&output.stderr).to_string() } else { String::new() },
    });

    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}
