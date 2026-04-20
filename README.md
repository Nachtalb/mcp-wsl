# mcp-wsl

[![CI](https://github.com/Nachtalb/mcp-wsl/actions/workflows/ci.yml/badge.svg)](https://github.com/Nachtalb/mcp-wsl/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Nachtalb/mcp-wsl)](https://github.com/Nachtalb/mcp-wsl/releases/latest)
[![AUR](https://img.shields.io/aur/version/mcp-wsl)](https://aur.archlinux.org/packages/mcp-wsl)
[![AUR bin](https://img.shields.io/aur/version/mcp-wsl-bin)](https://aur.archlinux.org/packages/mcp-wsl-bin)
[![License](https://img.shields.io/github/license/Nachtalb/mcp-wsl)](LICENSE)

A Rust-based [Model Context Protocol](https://modelcontextprotocol.io) (MCP) server that runs inside WSL and exposes system information and command execution capabilities to Windows-side MCP clients such as Claude Desktop.

Supports both **stdio** (invoked via `wsl.exe`) and **HTTP** (streamable HTTP transport) modes.

## Tools

### Read tools

| Tool | Description | Parameters |
|---|---|---|
| `read:get_system_info` | System information (`uname -a`) | — |
| `read:get_os_info` | OS distribution info from `/etc/os-release` and variants | — |
| `read:list_dir` | Directory listing with optional stat fields | `path: str = cwd`<br>`show_permissions: bool = false`<br>`show_size: bool = false`<br>`show_modified: bool = false`<br>`show_hidden: bool = false` |
| `read:get_mounts` | Mounted filesystems (`/proc/mounts`) | — |
| `read:get_wsl_config` | Contents of `/etc/wsl.conf` | — |
| `read:get_disk_usage` | Disk usage for a path (`df -h`) | `path: str = /` |
| `read:get_env` | Environment variables with optional substring filter | `filter: str` |
| `read:list_procs` | Running processes with selectable fields and optional filter | `filter: str`<br>`fields: [pid\|user\|cpu\|memory\|virtual_memory\|time\|status\|name\|command] = all` |
| `read:get_file` | File metadata and optional content (text or hex) for glob-matched files | `glob: str` *(required)*<br>`limit: int`<br>`show_permissions: bool = false`<br>`show_size: bool = false`<br>`show_modified: bool = false`<br>`content: none\|text\|hex = none` |
| `read:get_package_manager` | Detects available package managers (pacman, apt, dnf, cargo, npm, uv, …) | — |
| `read:get_shells` | Available shells from `/etc/shells` | — |
| `read:get_default_shell` | Current user's default shell | — |

### Exec tools

| Tool | Description | Parameters |
|---|---|---|
| `exec:execute_command` | Run a binary with an explicit argument list | `command: str` *(required)*<br>`args: str[]`<br>`user: str`<br>`stdin: str`<br>`stdin_file: str`<br>`stdout_file: str`<br>`stderr_file: str`<br>`timeout_secs: int = 30`<br>`working_dir: str` |
| `exec:execute_shell_command` | Run a full shell command string supporting pipes, redirects, and builtins | `command: str` *(required)*<br>`shell: str = $SHELL`<br>`user: str`<br>`stdin: str`<br>`stdout_file: str`<br>`stderr_file: str`<br>`timeout_secs: int = 30`<br>`working_dir: str` |

## Installation

### Arch Linux (AUR)

```bash
# Pre-built binary (fastest)
paru -S mcp-wsl-bin

# Build from source
paru -S mcp-wsl
```

Any AUR helper works (`yay`, `paru`, etc.). Both packages set the setuid bit automatically via pacman.

### Pre-built binary (recommended)

Auto-detects your architecture and installs the latest release:

```bash
ARCH=$(uname -m)
case $ARCH in
  x86_64)        FILE=mcp-wsl-linux-amd64 ;;
  i686|i386)     FILE=mcp-wsl-linux-i386  ;;
  aarch64|arm64) FILE=mcp-wsl-linux-arm64 ;;
  armv7l)        FILE=mcp-wsl-linux-arm32 ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

curl -fsSL "https://github.com/Nachtalb/mcp-wsl/releases/latest/download/$FILE" -o /tmp/mcp-wsl
sudo install -o root -m 4755 /tmp/mcp-wsl /usr/local/bin/mcp-wsl
```

### From source

Requires Rust (install via [rustup](https://rustup.rs)).

```bash
git clone https://github.com/Nachtalb/mcp-wsl
cd mcp-wsl
cargo build --release

# Install with setuid root so exec tools can switch users
sudo install -o root -m 4755 target/release/mcp-wsl /usr/local/bin/mcp-wsl
```

### Via cargo install

Requires Rust (install via [rustup](https://rustup.rs)).

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

## MCP Client Setup

### Claude Desktop (Windows)

Open **Settings → Developer → Edit Config** and add an entry to your `mcpServers` object:

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

### Claude Code

```bash
claude mcp add wsl -- wsl.exe -- mcp-wsl stdio

# Target a specific distro
claude mcp add wsl -- wsl.exe -d Ubuntu -- mcp-wsl stdio

# Full Linux path if mcp-wsl is not on $PATH
claude mcp add wsl -- wsl.exe -- /home/youruser/.cargo/bin/mcp-wsl stdio
```

The server is added to your local project scope by default. Use `--scope user` to make it available across all projects.

## Remote Access via Reverse Proxy

Run `mcp-wsl http` on any Linux host and put it behind a reverse proxy that handles TLS and authentication. mcp-wsl itself does no authentication — delegate that entirely to the proxy layer.

### nginx example with HTTP basic auth

```bash
# Create a password entry
htpasswd -c /etc/nginx/.htpasswd myuser
```

```nginx
server {
    listen 443 ssl;
    server_name mcp.example.com;

    ssl_certificate     /etc/ssl/certs/mcp.crt;
    ssl_certificate_key /etc/ssl/private/mcp.key;

    location /mcp {
        auth_basic "MCP Server";
        auth_basic_user_file /etc/nginx/.htpasswd;

        proxy_pass         http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header   Host $host;
        proxy_read_timeout 300s;
    }
}
```

### Connecting clients

Generate the base64 credential once:

```bash
echo -n "myuser:mypassword" | base64
```

**Claude Desktop** (`Settings → Developer → Edit Config`):

```json
{
  "mcpServers": {
    "linux": {
      "url": "https://mcp.example.com/mcp",
      "headers": {
        "Authorization": "Basic <base64-credential>"
      }
    }
  }
}
```

**Claude Code**:

```bash
claude mcp add --transport http \
  --header "Authorization: Basic <base64-credential>" \
  linux https://mcp.example.com/mcp
```

Any other header scheme your proxy requires (API key, bearer token, etc.) works the same way — just change the header name and value.

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
