pub mod config;
pub mod env;
pub mod exec;
pub mod files;
pub mod mounts;
pub mod packages;
pub mod procs;
pub mod shells;
pub mod system;

use rmcp::model::Tool;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub type ToolArgs = HashMap<String, Value>;
pub type ToolResult = Result<String, String>;

pub async fn dispatch(name: &str, args: ToolArgs) -> ToolResult {
    match name {
        "read:get_system_info" => system::get_system_info(args).await,
        "read:get_os_info" => system::get_os_info(args).await,
        "read:list_dir" => files::list_dir(args).await,
        "read:get_mounts" => mounts::get_mounts(args).await,
        "read:get_wsl_config" => config::get_wsl_config(args).await,
        "read:get_disk_usage" => mounts::get_disk_usage(args).await,
        "read:get_env" => env::get_env(args).await,
        "read:list_procs" => procs::list_procs(args).await,
        "read:get_file" => files::get_file(args).await,
        "read:get_package_manager" => packages::get_package_manager(args).await,
        "read:get_shells" => shells::get_shells(args).await,
        "read:get_default_shell" => shells::get_default_shell(args).await,
        "exec:execute_command" => exec::execute_command(args).await,
        "exec:execute_shell_command" => exec::execute_shell_command(args).await,
        _ => Err(format!("Unknown tool: {name}")),
    }
}

pub fn tool_list() -> Vec<Tool> {
    vec![
        tool(
            "read:get_system_info",
            "Returns system information equivalent to `uname -a`",
            json!({}),
            &[],
        ),
        tool(
            "read:get_os_info",
            "Returns OS distribution information from /etc/os-release and variants",
            json!({}),
            &[],
        ),
        tool(
            "read:list_dir",
            "Lists directory contents with optional stat fields",
            json!({
                "path": {"type": "string", "description": "Directory path (default: current working directory)"},
                "show_permissions": {"type": "boolean", "description": "Include Unix permission string"},
                "show_size": {"type": "boolean", "description": "Include size in bytes"},
                "show_modified": {"type": "boolean", "description": "Include last-modified timestamp"},
                "show_hidden": {"type": "boolean", "description": "Include dot-files"}
            }),
            &[],
        ),
        tool(
            "read:get_mounts",
            "Returns mounted filesystems (equivalent to `mount` or /proc/mounts)",
            json!({}),
            &[],
        ),
        tool(
            "read:get_wsl_config",
            "Returns the contents of /etc/wsl.conf",
            json!({}),
            &[],
        ),
        tool(
            "read:get_disk_usage",
            "Returns disk usage for a path, equivalent to `df -h`",
            json!({
                "path": {"type": "string", "description": "Path to check (default: /)"}
            }),
            &[],
        ),
        tool(
            "read:get_env",
            "Returns environment variables, optionally filtered by a substring",
            json!({
                "filter": {"type": "string", "description": "Case-insensitive substring to match against key or value"}
            }),
            &[],
        ),
        tool(
            "read:list_procs",
            "Lists running processes with optional filtering and field selection",
            json!({
                "filter": {"type": "string", "description": "Case-insensitive substring to match against command, name, or user"},
                "fields": {
                    "type": "array",
                    "items": {"type": "string", "enum": ["pid","user","cpu","memory","virtual_memory","time","status","name","command"]},
                    "description": "Fields to include (default: all)"
                }
            }),
            &[],
        ),
        tool(
            "read:get_file",
            "Retrieves file metadata and optionally content for files matching a glob pattern",
            json!({
                "glob": {"type": "string", "description": "Glob pattern to match files"},
                "limit": {"type": "integer", "description": "Maximum number of matched files to return"},
                "show_permissions": {"type": "boolean", "description": "Include Unix permission string"},
                "show_size": {"type": "boolean", "description": "Include size in bytes"},
                "show_modified": {"type": "boolean", "description": "Include last-modified timestamp"},
                "content": {
                    "type": "string",
                    "enum": ["none", "text", "hex"],
                    "description": "Whether and how to return file contents (default: none)"
                }
            }),
            &["glob"],
        ),
        tool(
            "read:get_package_manager",
            "Detects available package managers (system, AUR helpers, language-specific)",
            json!({}),
            &[],
        ),
        tool(
            "read:get_shells",
            "Lists available shells from /etc/shells",
            json!({}),
            &[],
        ),
        tool(
            "read:get_default_shell",
            "Returns the current user's default shell",
            json!({}),
            &[],
        ),
        tool(
            "exec:execute_command",
            "Executes a binary with explicit arguments. Captures stdout/stderr unless redirected to files.",
            json!({
                "command": {"type": "string", "description": "Binary name or absolute path"},
                "args": {"type": "array", "items": {"type": "string"}, "description": "Argument list"},
                "user": {"type": "string", "description": "Run as this user (name or numeric UID). Requires mcp-wsl to be installed with setuid root."},
                "stdin": {"type": "string", "description": "Text to pass to stdin"},
                "stdin_file": {"type": "string", "description": "Path to file whose contents are piped to stdin"},
                "stdout_file": {"type": "string", "description": "Write stdout to this file instead of returning it"},
                "stderr_file": {"type": "string", "description": "Write stderr to this file instead of returning it"},
                "timeout_secs": {"type": "integer", "description": "Timeout in seconds (default: 30)"},
                "working_dir": {"type": "string", "description": "Working directory for the command"}
            }),
            &["command"],
        ),
        tool(
            "exec:execute_shell_command",
            "Executes a full shell command string supporting pipes, redirects, and shell builtins.",
            json!({
                "command": {"type": "string", "description": "Full shell command string"},
                "shell": {"type": "string", "description": "Shell to use (default: /bin/sh)"},
                "user": {"type": "string", "description": "Run as this user (name or numeric UID). Requires mcp-wsl to be installed with setuid root."},
                "stdin": {"type": "string", "description": "Text to pass to stdin"},
                "stdout_file": {"type": "string", "description": "Write stdout to this file instead of returning it"},
                "stderr_file": {"type": "string", "description": "Write stderr to this file instead of returning it"},
                "timeout_secs": {"type": "integer", "description": "Timeout in seconds (default: 30)"},
                "working_dir": {"type": "string", "description": "Working directory for the command"}
            }),
            &["command"],
        ),
    ]
}

fn tool(name: &'static str, description: &'static str, properties: Value, required: &[&str]) -> Tool {
    let mut schema = Map::new();
    schema.insert("type".into(), json!("object"));
    schema.insert("properties".into(), properties);
    if !required.is_empty() {
        schema.insert("required".into(), json!(required));
    }
    Tool {
        name: name.into(),
        description: description.into(),
        input_schema: Arc::new(schema),
    }
}
