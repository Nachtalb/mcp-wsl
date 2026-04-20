mod http;
mod server;
mod tools;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mcp-wsl", about = "MCP server for WSL system interaction")]
struct Cli {
    #[command(subcommand)]
    command: Option<Mode>,
}

#[derive(Subcommand)]
enum Mode {
    /// Run as an MCP stdio server (default when no subcommand is given)
    Stdio,
    /// Run as an MCP HTTP server using streamable HTTP transport
    Http {
        /// Host to bind
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Mode::Stdio) {
        Mode::Stdio => server::run_stdio().await,
        Mode::Http { host, port } => http::run_http(host, port).await,
    }
}
