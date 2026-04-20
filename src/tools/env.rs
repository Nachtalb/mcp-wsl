use super::{ToolArgs, ToolResult};

pub async fn get_env(args: ToolArgs) -> ToolResult {
    let filter = args
        .get("filter")
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase());

    let mut vars: Vec<(String, String)> = std::env::vars().collect();
    vars.sort_by(|a, b| a.0.cmp(&b.0));

    if let Some(f) = &filter {
        vars.retain(|(k, v)| {
            k.to_lowercase().contains(f.as_str()) || v.to_lowercase().contains(f.as_str())
        });
    }

    Ok(vars
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n"))
}
