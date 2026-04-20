use super::{ToolArgs, ToolResult};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub async fn execute_command(args: ToolArgs) -> ToolResult {
    let command = str_arg(&args, "command")
        .ok_or("command parameter is required")?
        .to_string();

    let cmd_args: Vec<String> = args
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let (uid, gid) = resolve_exec_user(str_arg(&args, "user"))?;

    let mut cmd = Command::new(&command);
    cmd.args(&cmd_args).uid(uid).gid(gid);

    run_with_io(cmd, command, args).await
}

pub async fn execute_shell_command(args: ToolArgs) -> ToolResult {
    let shell = str_arg(&args, "shell").unwrap_or("/bin/sh").to_string();
    let command = str_arg(&args, "command")
        .ok_or("command parameter is required")?
        .to_string();

    let (uid, gid) = resolve_exec_user(str_arg(&args, "user"))?;

    let mut cmd = Command::new(&shell);
    cmd.arg("-c").arg(&command).uid(uid).gid(gid);

    run_with_io(cmd, shell, args).await
}

// ── Shared execution logic ────────────────────────────────────────────────────

async fn run_with_io(mut cmd: Command, label: String, args: ToolArgs) -> ToolResult {
    let stdin_text = str_arg(&args, "stdin").map(String::from);
    let stdin_file = str_arg(&args, "stdin_file").map(String::from);
    let stdout_file = str_arg(&args, "stdout_file").map(String::from);
    let stderr_file = str_arg(&args, "stderr_file").map(String::from);
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30);

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    if stdin_text.is_some() || stdin_file.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }
    if let Some(dir) = str_arg(&args, "working_dir") {
        cmd.current_dir(dir);
    }

    let result = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn '{label}': {e}"))?;

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

// ── Privilege resolution ──────────────────────────────────────────────────────

fn resolve_exec_user(user: Option<&str>) -> Result<(u32, u32), String> {
    let real_uid = unsafe { libc::getuid() };
    let real_gid = unsafe { libc::getgid() };

    let username = match user {
        None => return Ok((real_uid, real_gid)),
        Some(u) => u,
    };

    let (target_uid, target_gid) = lookup_user(username)?;

    if target_uid == real_uid {
        return Ok((real_uid, real_gid));
    }

    // Switching to a different user requires effective root (setuid binary).
    if unsafe { libc::geteuid() } != 0 {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mcp-wsl"));
        let p = exe.to_string_lossy();
        return Err(format!(
            "Cannot run as '{username}': mcp-wsl must be installed with setuid root.\n\
             Fix:\n  sudo chown root:root \"{p}\"\n  sudo chmod u+s \"{p}\""
        ));
    }

    Ok((target_uid, target_gid))
}

fn lookup_user(username: &str) -> Result<(u32, u32), String> {
    let content = fs::read_to_string("/etc/passwd")
        .map_err(|e| format!("Cannot read /etc/passwd: {e}"))?;

    let numeric = username.parse::<u32>().ok();

    for line in content.lines() {
        let p: Vec<&str> = line.split(':').collect();
        if p.len() < 4 {
            continue;
        }
        let matched = match numeric {
            Some(uid) => p[2].parse::<u32>().ok() == Some(uid),
            None => p[0] == username,
        };
        if matched {
            let uid = p[2]
                .parse::<u32>()
                .map_err(|_| format!("Invalid UID for '{username}'"))?;
            let gid = p[3]
                .parse::<u32>()
                .map_err(|_| format!("Invalid GID for '{username}'"))?;
            return Ok((uid, gid));
        }
    }

    Err(format!("User '{username}' not found"))
}

fn str_arg<'a>(args: &'a ToolArgs, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}
