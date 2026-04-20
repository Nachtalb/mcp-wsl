use super::{ToolArgs, ToolResult};
use serde_json::{json, Value};
use std::fs;
use sysinfo::System;

const ALL_FIELDS: &[&str] = &["pid", "user", "cpu", "memory", "virtual_memory", "time", "status", "name", "command"];

pub async fn list_procs(args: ToolArgs) -> ToolResult {
    let filter = args
        .get("filter")
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase());

    let fields: Vec<String> = args
        .get("fields")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_else(|| ALL_FIELDS.iter().map(|s| s.to_string()).collect());

    let uid_map = build_uid_map();

    let mut system = System::new_all();
    system.refresh_all();

    let mut processes: Vec<Value> = system
        .processes()
        .iter()
        .map(|(pid, proc)| {
            let mut obj = serde_json::Map::new();
            for field in &fields {
                match field.as_str() {
                    "pid" => {
                        obj.insert("pid".into(), json!(pid.as_u32()));
                    }
                    "user" => {
                        let name = proc
                            .user_id()
                            .and_then(|uid| {
                                let uid_num: u32 = **uid;
                                uid_map.get(&uid_num).cloned()
                            })
                            .unwrap_or_else(|| "?".to_string());
                        obj.insert("user".into(), json!(name));
                    }
                    "cpu" => {
                        obj.insert("cpu".into(), json!(format!("{:.1}%", proc.cpu_usage())));
                    }
                    "memory" => {
                        obj.insert("memory".into(), json!(format_bytes(proc.memory())));
                    }
                    "virtual_memory" => {
                        obj.insert(
                            "virtual_memory".into(),
                            json!(format_bytes(proc.virtual_memory())),
                        );
                    }
                    "time" => {
                        obj.insert("time".into(), json!(format_duration(proc.run_time())));
                    }
                    "status" => {
                        obj.insert("status".into(), json!(format!("{:?}", proc.status())));
                    }
                    "name" => {
                        obj.insert(
                            "name".into(),
                            json!(proc.name().to_string_lossy().to_string()),
                        );
                    }
                    "command" => {
                        let cmd: Vec<String> = proc
                            .cmd()
                            .iter()
                            .map(|s| s.to_string_lossy().to_string())
                            .collect();
                        obj.insert("command".into(), json!(cmd.join(" ")));
                    }
                    _ => {}
                }
            }
            Value::Object(obj)
        })
        .collect();

    if let Some(f) = &filter {
        processes.retain(|p| {
            ["command", "name", "user"].iter().any(|key| {
                p.get(*key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase().contains(f.as_str()))
                    .unwrap_or(false)
            })
        });
    }

    processes.sort_by_key(|p| p.get("pid").and_then(|v| v.as_u64()).unwrap_or(0));

    serde_json::to_string_pretty(&processes).map_err(|e| e.to_string())
}

fn build_uid_map() -> std::collections::HashMap<u32, String> {
    let mut map = std::collections::HashMap::new();
    if let Ok(content) = fs::read_to_string("/etc/passwd") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                if let Ok(uid) = parts[2].parse::<u32>() {
                    map.insert(uid, parts[0].to_string());
                }
            }
        }
    }
    map
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
    }
}

fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
