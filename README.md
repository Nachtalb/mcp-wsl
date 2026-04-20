# mcp-wsl

A Rust-based [Model Context Protocol](https://modelcontextprotocol.io) (MCP) server that runs inside WSL and exposes system information and command execution capabilities to Windows-side MCP clients such as Claude Desktop.

Supports both **stdio** (invoked via `wsl.exe`) and **HTTP** (streamable HTTP transport) modes.

## Tools

### Read tools

| Tool | Description | Parameters |
|---|---|---|
| `read:get_system_info` | System information (`uname -a`) | — |
| `read:get_os_info` | OS distribution info from `/etc/os-release` and variants | — |
| `read:list_dir` | Directory listing with optional stat fields | `path`<br>`show_permissions`<br>`show_size`<br>`show_modified`<br>`show_hidden` |
| `read:get_mounts` | Mounted filesystems (`/proc/mounts`) | — |
| `read:get_wsl_config` | Contents of `/etc/wsl.conf` | — |
| `read:get_disk_usage` | Disk usage for a path (`df -h`) | `path` |
| `read:get_env` | Environment variables with optional substring filter | `filter` |
| `read:list_procs` | Running processes with selectable fields and optional filter | `filter`<br>`fields` |
| `read:get_file` | File metadata and optional content (text or hex) for glob-matched files | `glob` *(required)*<br>`limit`<br>`show_permissions`<br>`show_size`<br>`show_modified`<br>`content` |
| `read:get_package_manager` | Detects available package managers (pacman, apt, dnf, cargo, npm, uv, …) | — |
| `read:get_shells` | Available shells from `/etc/shells` | — |
| `read:get_default_shell` | Current user's default shell | — |

### Exec tools

| Tool | Description | Parameters |
|---|---|---|
| `exec:execute_command` | Run a binary with an explicit argument list | `command` *(required)*<br>`args`<br>`user`<br>`stdin`<br>`stdin_file`<br>`stdout_file`<br>`stderr_file`<br>`timeout_secs`<br>`working_dir` |
| `exec:execute_shell_command` | Run a full shell command string supporting pipes, redirects, and builtins | `command` *(required)*<br>`shell`<br>`user`<br>`stdin`<br>`stdout_file`<br>`stderr_file`<br>`timeout_secs`<br>`working_dir` |

## Installation

Requires Rust (install via [rustup](https://rustup.rs)).

### From source

```bash
git clone https://github.com/Nachtalb/mcp-wsl
cd mcp-wsl
cargo build --release

# Install with setuid root so exec tools can switch users
sudo install -o root -m 4755 target/release/mcp-wsl /usr/local/bin/mcp-wsl
```

### Via cargo install

```bash
cargo install --git https://github.com/Nachtalb/mcp-wsl

# Set setuid root so exec tools can switch users
sudo chown root:root ~/.cargo/bin/mcp-wsl
sudo chmod u+s ~/.cargo/bin/mcp-wsl
```

The setuid bit is what allows the server to switch to any user when the `user` parameter is passed to exec tools. Without it, `user` still works as long as you request the same user the server is already running as — any other user returns a clear error pointing to the fix.

## Usage

### Stdio mode (default)

Used by MCP clients that spawn the server as a subprocess. No flags needed — stdio is the default when no subcommand is given.

```bash
mcp-wsl
# or explicitly:
mcp-wsl stdio
```

### HTTP mode

Runs an HTTP server implementing the [MCP streamable HTTP transport](https://modelcontextprotocol.io/docs/concepts/transports). Useful for clients that connect over the network or for testing with plain HTTP tools.

```bash
mcp-wsl http                              # binds 127.0.0.1:3000
mcp-wsl http --host 0.0.0.0 --port 8080
```

Endpoint: `POST /mcp` with `Content-Type: application/json` (JSON-RPC 2.0).

## Connecting to Claude Desktop (Windows)

Add an entry to your Claude Desktop config file at  
`%APPDATA%\Claude\claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "wsl": {
      "command": "wsl.exe",
      "args": ["--", "mcp-wsl", "stdio"]
    }
  }
}
```

If `mcp-wsl` is not on your WSL `$PATH`, use the full Linux path:

```json
{
  "mcpServers": {
    "wsl": {
      "command": "wsl.exe",
      "args": ["--", "/home/youruser/.cargo/bin/mcp-wsl", "stdio"]
    }
  }
}
```

To target a specific WSL distro:

```json
{
  "mcpServers": {
    "wsl": {
      "command": "wsl.exe",
      "args": ["-d", "Ubuntu", "--", "mcp-wsl", "stdio"]
    }
  }
}
```

## Testing

`test_server.py` is a standalone Python 3 script (stdlib only) that exercises every tool with and without optional parameters against either transport. It can be run from Windows or Linux.

```bash
# Stdio — spawns the server automatically
python test_server.py stdio --binary ./target/release/mcp-wsl

# Stdio via wsl.exe from Windows (auto-detected on Windows)
python test_server.py stdio --binary mcp-wsl --wsl
python test_server.py stdio --binary mcp-wsl --wsl --distro Ubuntu

# HTTP — server must already be running
# (start it first: mcp-wsl http --host 0.0.0.0 --port 3000)
python test_server.py http --host 127.0.0.1 --port 3000

# Show full tool output for each test
python test_server.py stdio --binary ./target/release/mcp-wsl -v
```

Expected output (all 45 cases):

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  mcp-wsl test suite  •  transport: stdio
  binary: ./target/release/mcp-wsl
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

read:get_system_info
  ✓  basic call                               Linux DESKTOP 6.6.87.2 …

...

  45 passed
```

## License

Licensed under the GNU Lesser General Public License v3.0 — see [LICENSE](LICENSE).
