use super::{ToolArgs, ToolResult};
use glob::glob;
use serde_json::{json, Value};
use std::fs::{self, Metadata};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

pub async fn list_dir(args: ToolArgs) -> ToolResult {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

    let show_perms = flag(&args, "show_permissions");
    let show_size = flag(&args, "show_size");
    let show_modified = flag(&args, "show_modified");
    let show_hidden = flag(&args, "show_hidden");

    let entries =
        fs::read_dir(&path).map_err(|e| format!("Cannot read directory {}: {e}", path.display()))?;

    let mut items: Vec<Value> = entries
        .filter_map(|e| e.ok())
        .filter(|e| show_hidden || !e.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().ok();
            let type_str = match &file_type {
                Some(ft) if ft.is_dir() => "directory",
                Some(ft) if ft.is_symlink() => "symlink",
                Some(ft) if ft.is_file() => "file",
                _ => "other",
            };

            let mut obj = json!({ "name": name, "type": type_str });

            if let Ok(meta) = entry.metadata() {
                if show_perms {
                    obj["permissions"] = json!(unix_perms(meta.permissions().mode()));
                }
                if show_size {
                    obj["size"] = json!(meta.len());
                }
                if show_modified {
                    obj["modified"] = json!(modified_str(&meta));
                }
            }
            obj
        })
        .collect();

    items.sort_by(|a, b| {
        a.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .cmp(b.get("name").and_then(|v| v.as_str()).unwrap_or(""))
    });

    serde_json::to_string_pretty(&items).map_err(|e| e.to_string())
}

pub async fn get_file(args: ToolArgs) -> ToolResult {
    let pattern = args
        .get("glob")
        .and_then(|v| v.as_str())
        .ok_or("glob parameter is required")?;

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(usize::MAX);

    let show_perms = flag(&args, "show_permissions");
    let show_size = flag(&args, "show_size");
    let show_modified = flag(&args, "show_modified");
    let content_mode = args
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("none");

    let paths: Vec<PathBuf> = glob(pattern)
        .map_err(|e| format!("Invalid glob pattern: {e}"))?
        .filter_map(|r| r.ok())
        .filter(|p| p.is_file())
        .take(limit)
        .collect();

    let mut results: Vec<Value> = Vec::new();

    for path in paths {
        let mut obj = json!({ "path": path.to_string_lossy() });

        if let Ok(meta) = path.metadata() {
            if show_perms {
                obj["permissions"] = json!(unix_perms(meta.permissions().mode()));
            }
            if show_size {
                obj["size"] = json!(meta.len());
            }
            if show_modified {
                obj["modified"] = json!(modified_str(&meta));
            }
        }

        match content_mode {
            "text" => match fs::read_to_string(&path) {
                Ok(c) => obj["content"] = json!(c),
                Err(e) => obj["content_error"] = json!(e.to_string()),
            },
            "hex" => match fs::read(&path) {
                Ok(b) => obj["content"] = json!(hex::encode(&b)),
                Err(e) => obj["content_error"] = json!(e.to_string()),
            },
            _ => {}
        }

        results.push(obj);
    }

    serde_json::to_string_pretty(&results).map_err(|e| e.to_string())
}

fn flag(args: &ToolArgs, key: &str) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn unix_perms(mode: u32) -> String {
    let bits = [
        (0o400, 'r'), (0o200, 'w'), (0o100, 'x'),
        (0o040, 'r'), (0o020, 'w'), (0o010, 'x'),
        (0o004, 'r'), (0o002, 'w'), (0o001, 'x'),
    ];
    bits.iter()
        .map(|(bit, c)| if mode & bit != 0 { *c } else { '-' })
        .collect()
}

fn modified_str(meta: &Metadata) -> String {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| {
            let secs = d.as_secs() as i64;
            chrono::DateTime::from_timestamp(secs, 0)
                .map(|dt: chrono::DateTime<chrono::Utc>| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| secs.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}
