use crate::tools;
use anyhow::Result;
use rmcp::{
    model::{
        CallToolRequestParam, CallToolResult, Content, ListToolsResult, PaginatedRequestParamInner,
        ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    Error as McpError, RoleServer, ServerHandler, ServiceExt,
};

#[derive(Clone)]
pub struct McpWslServer;

impl ServerHandler for McpWslServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "MCP server providing WSL system information and command execution".into(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParamInner>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: tools::tool_list(),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let args: tools::ToolArgs = request
            .arguments
            .map(|m: serde_json::Map<String, serde_json::Value>| m.into_iter().collect())
            .unwrap_or_default();

        match tools::dispatch(&request.name, args).await {
            Ok(text) => Ok(CallToolResult {
                content: vec![Content::text(text)],
                is_error: Some(false),
            }),
            Err(msg) => Ok(CallToolResult {
                content: vec![Content::text(msg)],
                is_error: Some(true),
            }),
        }
    }
}

pub async fn run_stdio() -> Result<()> {
    let service = McpWslServer
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    service
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(())
}
