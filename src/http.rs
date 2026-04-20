use crate::tools;
use anyhow::Result;
use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use std::net::SocketAddr;

pub async fn run_http(host: String, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/mcp", post(handle_post))
        .layer(tower_http::cors::CorsLayer::permissive());

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    eprintln!("mcp-wsl HTTP server listening on http://{addr}/mcp");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_post(body: Bytes) -> Response {
    let val: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {e}")}
                })),
            )
                .into_response()
        }
    };

    if val.is_array() {
        let responses: Vec<Value> = val
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(|req| {
                // Spawn each in-place; batch items are handled sequentially here for simplicity.
                // For async we collect futures instead.
                req
            })
            .collect();

        // Process batch asynchronously
        let mut out = Vec::new();
        for req in responses {
            if let Some(resp) = dispatch_request(req).await {
                out.push(resp);
            }
        }
        (StatusCode::OK, Json(Value::Array(out))).into_response()
    } else {
        match dispatch_request(val).await {
            Some(resp) => (StatusCode::OK, Json(resp)).into_response(),
            None => StatusCode::NO_CONTENT.into_response(),
        }
    }
}

async fn dispatch_request(req: Value) -> Option<Value> {
    let id = req.get("id").cloned();
    let method = req.get("method")?.as_str()?.to_string();
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let is_notification = id.is_none();

    let result = handle_method(&method, params).await;

    if is_notification {
        return None;
    }

    let id = id.unwrap_or(Value::Null);
    Some(match result {
        Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
        Err(msg) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32603, "message": msg}
        }),
    })
}

async fn handle_method(method: &str, params: Value) -> Result<Value, String> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {"tools": {}},
            "serverInfo": {
                "name": env!("CARGO_PKG_NAME"),
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": "MCP server providing WSL system information and command execution"
        })),

        "ping" => Ok(json!({})),

        "notifications/initialized" | "notifications/cancelled" => Ok(Value::Null),

        "tools/list" => {
            let tools: Vec<Value> = tools::tool_list()
                .into_iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": *t.input_schema,
                    })
                })
                .collect();
            Ok(json!({"tools": tools}))
        }

        "tools/call" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing tool name")?
                .to_string();

            let args: tools::ToolArgs = params
                .get("arguments")
                .and_then(|v| v.as_object())
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            match tools::dispatch(&name, args).await {
                Ok(text) => Ok(json!({
                    "content": [{"type": "text", "text": text}],
                    "isError": false
                })),
                Err(msg) => Ok(json!({
                    "content": [{"type": "text", "text": msg}],
                    "isError": true
                })),
            }
        }

        _ => Err(format!("Method not found: {method}")),
    }
}
