#!/usr/bin/env python3
"""
mcp-wsl test suite — exercises every tool via stdio or HTTP transport.

Usage
-----
Stdio (spawns the server directly or via wsl.exe):
  python test_server.py stdio
  python test_server.py stdio --binary /home/user/.cargo/bin/mcp-wsl
  python test_server.py stdio --binary mcp-wsl --wsl
  python test_server.py stdio --binary mcp-wsl --wsl --distro Ubuntu

HTTP (server must already be running):
  # In WSL first: mcp-wsl http --host 0.0.0.0 --port 3000
  python test_server.py http
  python test_server.py http --host 127.0.0.1 --port 3000

Options:
  -v, --verbose    Print full tool output for every test
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Callable, Dict, List, Optional

# ── ANSI colours (Windows needs virtual-terminal processing enabled) ──────────

def _enable_ansi_windows() -> bool:
    try:
        import ctypes
        kernel32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
        kernel32.SetConsoleMode(kernel32.GetStdHandle(-11), 7)
        return True
    except Exception:
        return False

if sys.stdout.isatty():
    if os.name == "nt":
        _ok = _enable_ansi_windows()
    else:
        _ok = True
else:
    _ok = False

G  = "\033[92m" if _ok else ""   # green
R  = "\033[91m" if _ok else ""   # red
Y  = "\033[93m" if _ok else ""   # yellow
C  = "\033[96m" if _ok else ""   # cyan
DIM= "\033[2m"  if _ok else ""   # dim
B  = "\033[1m"  if _ok else ""   # bold
X  = "\033[0m"  if _ok else ""   # reset


# ── Transport layer ───────────────────────────────────────────────────────────

class Transport(ABC):
    @abstractmethod
    def call_tool(self, name: str, args: Optional[dict] = None) -> dict:
        ...

    @abstractmethod
    def close(self) -> None:
        ...


class StdioTransport(Transport):
    """Communicates with mcp-wsl over stdin/stdout (newline-delimited JSON-RPC)."""

    def __init__(self, binary: str, wsl: bool = False, distro: Optional[str] = None):
        if wsl:
            cmd = ["wsl.exe"]
            if distro:
                cmd += ["-d", distro]
            cmd += ["--", binary, "stdio"]
        else:
            cmd = [binary, "stdio"]

        self._proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self._next_id = 1
        self._responses: Dict[int, dict] = {}
        self._lock = threading.Lock()
        self._events: Dict[int, threading.Event] = {}

        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()

        # MCP handshake
        init_resp = self._rpc(0, "initialize", {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "mcp-wsl-test", "version": "1.0"},
        })
        if "error" in init_resp:
            raise RuntimeError(f"initialize failed: {init_resp['error']}")
        self._send_notify("notifications/initialized", {})

    # ── internal ──────────────────────────────────────────────────────────────

    def _read_loop(self) -> None:
        assert self._proc.stdout
        for raw in self._proc.stdout:
            raw = raw.strip()
            if not raw:
                continue
            try:
                msg = json.loads(raw)
                mid = msg.get("id")
                if mid is not None:
                    ev: Optional[threading.Event]
                    with self._lock:
                        self._responses[mid] = msg
                        ev = self._events.get(mid)
                    if ev:
                        ev.set()
            except json.JSONDecodeError:
                pass

    def _send(self, msg: dict) -> None:
        assert self._proc.stdin
        self._proc.stdin.write(json.dumps(msg) + "\n")
        self._proc.stdin.flush()

    def _send_notify(self, method: str, params: dict) -> None:
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def _wait_for(self, mid: int, timeout: float = 60.0) -> dict:
        ev = threading.Event()
        with self._lock:
            if mid in self._responses:
                return self._responses.pop(mid)
            self._events[mid] = ev
        if not ev.wait(timeout):
            raise TimeoutError(f"No response for id={mid} after {timeout}s")
        with self._lock:
            self._events.pop(mid, None)
            return self._responses.pop(mid)

    def _rpc(self, mid: int, method: str, params: dict) -> dict:
        self._send({"jsonrpc": "2.0", "id": mid, "method": method, "params": params})
        return self._wait_for(mid)

    # ── public ────────────────────────────────────────────────────────────────

    def call_tool(self, name: str, args: Optional[dict] = None) -> dict:
        mid = self._next_id
        self._next_id += 1
        return self._rpc(mid, "tools/call", {"name": name, "arguments": args or {}})

    def close(self) -> None:
        try:
            assert self._proc.stdin
            self._proc.stdin.close()
            self._proc.wait(timeout=5)
        except Exception:
            self._proc.kill()


class HttpTransport(Transport):
    """Communicates with mcp-wsl over HTTP (streamable HTTP MCP transport)."""

    def __init__(self, host: str = "127.0.0.1", port: int = 3000):
        self._url = f"http://{host}:{port}/mcp"
        self._next_id = 1

        init_resp = self._post(0, "initialize", {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "mcp-wsl-test", "version": "1.0"},
        })
        if "error" in init_resp:
            raise RuntimeError(f"initialize failed: {init_resp['error']}")

        self._post_notify("notifications/initialized", {})

    def _post(self, mid: int, method: str, params: dict) -> dict:
        payload = json.dumps({"jsonrpc": "2.0", "id": mid, "method": method, "params": params}).encode()
        req = urllib.request.Request(
            self._url,
            data=payload,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                body = r.read()
                return json.loads(body) if body else {}
        except urllib.error.HTTPError as e:
            body = e.read()
            return json.loads(body) if body else {"error": str(e)}

    def _post_notify(self, method: str, params: dict) -> None:
        payload = json.dumps({"jsonrpc": "2.0", "method": method, "params": params}).encode()
        req = urllib.request.Request(
            self._url,
            data=payload,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=10) as r:
                r.read()
        except Exception:
            pass

    def call_tool(self, name: str, args: Optional[dict] = None) -> dict:
        mid = self._next_id
        self._next_id += 1
        return self._post(mid, "tools/call", {"name": name, "arguments": args or {}})

    def close(self) -> None:
        pass


# ── Test runner ───────────────────────────────────────────────────────────────

@dataclass
class Case:
    label: str
    tool: str
    args: dict = field(default_factory=dict)
    check: Optional[Callable[[str], None]] = None
    soft: bool = False   # if True, a tool-level error is reported as SKIP not FAIL

@dataclass
class Result:
    label: str
    tool: str
    status: str          # PASS / FAIL / SKIP
    snippet: str = ""
    reason: str = ""


def _snippet(text: str, width: int = 72) -> str:
    s = text.replace("\n", " ").strip()
    return s[:width] + "…" if len(s) > width else s


def run_case(transport: Transport, case: Case, verbose: bool) -> Result:
    try:
        resp = transport.call_tool(case.tool, case.args)
    except TimeoutError as e:
        return Result(case.label, case.tool, "FAIL", reason=str(e))
    except Exception as e:
        return Result(case.label, case.tool, "FAIL", reason=f"{type(e).__name__}: {e}")

    if "error" in resp:
        err = resp["error"]
        msg = err.get("message", str(err)) if isinstance(err, dict) else str(err)
        return Result(case.label, case.tool, "FAIL", reason=f"JSON-RPC error: {msg}")

    result_obj = resp.get("result", {})
    is_error = result_obj.get("isError", False)
    content = result_obj.get("content", [])
    text = content[0].get("text", "") if content else ""

    if verbose:
        print(f"\n{DIM}{'─'*60}{X}")
        print(text)

    if is_error:
        status = "SKIP" if case.soft else "FAIL"
        return Result(case.label, case.tool, status, snippet=_snippet(text), reason=text[:200])

    if case.check:
        try:
            case.check(text)
        except AssertionError as e:
            return Result(case.label, case.tool, "FAIL", snippet=_snippet(text), reason=str(e))

    return Result(case.label, case.tool, "PASS", snippet=_snippet(text))


# ── Test definitions ──────────────────────────────────────────────────────────

def contains(*needles: str) -> Callable[[str], None]:
    def _check(text: str) -> None:
        for n in needles:
            assert n in text, f"Expected {n!r} in output"
    return _check


def is_json(text: str) -> None:
    try:
        json.loads(text)
    except json.JSONDecodeError as e:
        raise AssertionError(f"Output is not valid JSON: {e}")


def is_json_array(text: str) -> None:
    is_json(text)
    assert text.strip().startswith("["), "Expected a JSON array"


def status_zero(text: str) -> None:
    is_json(text)
    d = json.loads(text)
    assert d.get("status") == 0, f"Expected status 0, got {d.get('status')}"


def has_stdout(text: str) -> None:
    is_json(text)
    d = json.loads(text)
    assert d.get("stdout", "").strip(), "Expected non-empty stdout"


ALL_CASES: List[Case] = [
    # ── read:get_system_info ─────────────────────────────────────────────────
    Case("basic call", "read:get_system_info",
         check=contains("Linux")),

    # ── read:get_os_info ─────────────────────────────────────────────────────
    Case("basic call", "read:get_os_info",
         check=contains("os-release")),

    # ── read:list_dir ────────────────────────────────────────────────────────
    Case("default path (cwd)",   "read:list_dir",
         check=is_json_array),
    Case("path=/tmp",            "read:list_dir", {"path": "/tmp"},
         check=is_json_array),
    Case("show_size",            "read:list_dir", {"path": "/", "show_size": True},
         check=contains('"size"')),
    Case("show_permissions",     "read:list_dir", {"path": "/", "show_permissions": True},
         check=contains('"permissions"')),
    Case("show_modified",        "read:list_dir", {"path": "/", "show_modified": True},
         check=contains('"modified"')),
    Case("show_hidden",          "read:list_dir", {"path": "/etc", "show_hidden": True},
         check=is_json_array),
    Case("all flags combined",   "read:list_dir", {
         "path": "/etc", "show_size": True, "show_permissions": True,
         "show_modified": True, "show_hidden": True},
         check=lambda t: (contains('"size"')(t), contains('"permissions"')(t), contains('"modified"')(t))),

    # ── read:get_mounts ──────────────────────────────────────────────────────
    Case("basic call", "read:get_mounts",
         check=contains("/")),

    # ── read:get_wsl_config ──────────────────────────────────────────────────
    Case("basic call (may not exist)", "read:get_wsl_config",
         soft=True),   # /etc/wsl.conf is optional

    # ── read:get_disk_usage ──────────────────────────────────────────────────
    Case("default path (/)",     "read:get_disk_usage",
         check=contains("Filesystem")),
    Case("explicit path=/",      "read:get_disk_usage", {"path": "/"},
         check=contains("Filesystem")),
    Case("path=/tmp",            "read:get_disk_usage", {"path": "/tmp"},
         check=contains("Filesystem")),

    # ── read:get_env ─────────────────────────────────────────────────────────
    Case("all vars (no filter)",     "read:get_env",
         check=contains("=")),
    Case("filter=PATH",              "read:get_env", {"filter": "PATH"},
         check=contains("PATH")),
    Case("filter=NONEXISTENT_XYZ",   "read:get_env", {"filter": "NONEXISTENT_XYZ"}),

    # ── read:list_procs ──────────────────────────────────────────────────────
    Case("all fields (default)",     "read:list_procs",
         check=lambda t: (is_json_array(t), contains('"pid"')(t))),
    Case("filter by name",           "read:list_procs", {"filter": "mcp-wsl"},
         check=is_json_array),
    Case("fields=[pid,name]",        "read:list_procs", {"fields": ["pid", "name"]},
         check=lambda t: (is_json_array(t), contains('"pid"')(t), contains('"name"')(t))),
    Case("fields=[pid,cpu,memory]",  "read:list_procs", {"fields": ["pid", "cpu", "memory"]},
         check=lambda t: (is_json_array(t), contains('"cpu"')(t), contains('"memory"')(t))),
    Case("fields=[pid,user,command]","read:list_procs", {"fields": ["pid", "user", "command"]},
         check=lambda t: (is_json_array(t), contains('"user"')(t))),

    # ── read:get_file ────────────────────────────────────────────────────────
    Case("single path (/etc/hostname)",          "read:get_file", {"glob": "/etc/hostname"},
         check=lambda t: (is_json_array(t), contains('"path"')(t))),
    Case("with content=text",                    "read:get_file",
         {"glob": "/etc/hostname", "content": "text"},
         check=contains('"content"')),
    Case("with content=hex",                     "read:get_file",
         {"glob": "/etc/hostname", "content": "hex"},
         check=contains('"content"')),
    Case("wildcard glob (/etc/*.conf)",          "read:get_file", {"glob": "/etc/*.conf"},
         check=is_json_array),
    Case("glob + limit=2",                       "read:get_file",
         {"glob": "/etc/*.conf", "limit": 2},
         check=lambda t: (is_json_array(t), assert_len_le(t, 2))),
    Case("show_size + show_modified + show_permissions", "read:get_file", {
         "glob": "/etc/hostname", "show_size": True,
         "show_modified": True, "show_permissions": True},
         check=lambda t: (contains('"size"')(t), contains('"modified"')(t), contains('"permissions"')(t))),

    # ── read:get_package_manager ─────────────────────────────────────────────
    Case("basic call", "read:get_package_manager"),

    # ── read:get_shells ──────────────────────────────────────────────────────
    Case("basic call", "read:get_shells",
         check=contains("/bin/")),

    # ── read:get_default_shell ───────────────────────────────────────────────
    Case("basic call", "read:get_default_shell",
         check=contains("/")),

    # ── exec:execute_command ─────────────────────────────────────────────────
    Case("basic echo",           "exec:execute_command",
         {"command": "echo", "args": ["hello world"]},
         check=lambda t: (status_zero(t), contains("hello world")(json.loads(t)["stdout"]))),
    Case("multiple args",        "exec:execute_command",
         {"command": "printf", "args": ["%s-%s", "foo", "bar"]},
         check=lambda t: contains("foo-bar")(json.loads(t)["stdout"])),
    Case("with stdin text",      "exec:execute_command",
         {"command": "cat", "stdin": "hello from stdin"},
         check=lambda t: contains("hello from stdin")(json.loads(t)["stdout"])),
    Case("with working_dir",     "exec:execute_command",
         {"command": "pwd", "working_dir": "/tmp"},
         check=lambda t: contains("/tmp")(json.loads(t)["stdout"])),
    Case("non-zero exit code",   "exec:execute_command",
         {"command": "false"},
         check=lambda t: (is_json(t), assert_status_nonzero(t))),
    Case("capture stderr",       "exec:execute_command",
         {"command": "ls", "args": ["/nonexistent_path_xyz"]},
         check=lambda t: json.loads(t)["stderr"] != ""),
    Case("with timeout_secs",    "exec:execute_command",
         {"command": "echo", "args": ["quick"], "timeout_secs": 5},
         check=status_zero),

    # ── exec:execute_shell_command ───────────────────────────────────────────
    Case("basic command",        "exec:execute_shell_command",
         {"command": "echo hello"},
         check=lambda t: contains("hello")(json.loads(t)["stdout"])),
    Case("with pipe",            "exec:execute_shell_command",
         {"command": "echo hello | tr a-z A-Z"},
         check=lambda t: contains("HELLO")(json.loads(t)["stdout"])),
    Case("with stdin text",      "exec:execute_shell_command",
         {"command": "cat", "stdin": "piped in"},
         check=lambda t: contains("piped in")(json.loads(t)["stdout"])),
    Case("with working_dir",     "exec:execute_shell_command",
         {"command": "pwd", "working_dir": "/var"},
         check=lambda t: contains("/var")(json.loads(t)["stdout"])),
    Case("explicit shell=/bin/bash", "exec:execute_shell_command",
         {"command": "echo $BASH_VERSION", "shell": "/bin/bash"},
         check=status_zero),
    Case("multi-word pipeline",  "exec:execute_shell_command",
         {"command": "seq 5 | paste -s -d+"},
         check=lambda t: contains("1+2+3+4+5")(json.loads(t)["stdout"])),
    Case("with timeout_secs",    "exec:execute_shell_command",
         {"command": "sleep 0.1 && echo done", "timeout_secs": 10},
         check=lambda t: contains("done")(json.loads(t)["stdout"])),
]


# ── Helpers for complex validators ────────────────────────────────────────────

def assert_len_le(text: str, max_len: int) -> None:
    arr = json.loads(text)
    assert isinstance(arr, list), "Not a list"
    assert len(arr) <= max_len, f"Expected ≤{max_len} items, got {len(arr)}"


def assert_status_nonzero(text: str) -> None:
    d = json.loads(text)
    assert d.get("status", 0) != 0, "Expected non-zero exit status"


# ── Presentation ──────────────────────────────────────────────────────────────

TOOL_ORDER = [
    "read:get_system_info",
    "read:get_os_info",
    "read:list_dir",
    "read:get_mounts",
    "read:get_wsl_config",
    "read:get_disk_usage",
    "read:get_env",
    "read:list_procs",
    "read:get_file",
    "read:get_package_manager",
    "read:get_shells",
    "read:get_default_shell",
    "exec:execute_command",
    "exec:execute_shell_command",
]

STATUS_ICON = {"PASS": f"{G}✓{X}", "FAIL": f"{R}✗{X}", "SKIP": f"{Y}○{X}"}
STATUS_LABEL = {"PASS": f"{G}PASS{X}", "FAIL": f"{R}FAIL{X}", "SKIP": f"{Y}SKIP{X}"}


def print_results(results: List[Result], verbose: bool) -> int:
    by_tool: Dict[str, List[Result]] = {}
    for r in results:
        by_tool.setdefault(r.tool, []).append(r)

    print()
    for tool in TOOL_ORDER:
        cases = by_tool.get(tool, [])
        if not cases:
            continue
        print(f"{C}{B}{tool}{X}")
        for r in cases:
            icon = STATUS_ICON[r.status]
            label = f"{r.label:<40}"
            if r.status == "PASS":
                info = f"{DIM}{r.snippet}{X}"
            elif r.status == "SKIP":
                info = f"{Y}{r.reason[:60] if r.reason else r.snippet}{X}"
            else:
                info = f"{R}{r.reason[:70]}{X}"
            print(f"  {icon}  {label} {info}")
        print()

    total = len(results)
    passed = sum(1 for r in results if r.status == "PASS")
    failed = sum(1 for r in results if r.status == "FAIL")
    skipped = sum(1 for r in results if r.status == "SKIP")

    bar = "━" * 60
    print(f"{DIM}{bar}{X}")
    parts = [f"{G}{passed} passed{X}"]
    if failed:
        parts.append(f"{R}{failed} failed{X}")
    if skipped:
        parts.append(f"{Y}{skipped} skipped{X}")
    print("  " + ", ".join(parts))
    print(f"{DIM}{bar}{X}")

    return failed


# ── CLI ───────────────────────────────────────────────────────────────────────

def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="mcp-wsl test suite",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    sub = parser.add_subparsers(dest="transport", required=True)

    stdio_p = sub.add_parser("stdio", help="Test via stdio transport")
    stdio_p.add_argument(
        "--binary",
        default="mcp-wsl",
        help="Path to mcp-wsl binary inside WSL (default: mcp-wsl)",
    )
    stdio_p.add_argument(
        "--wsl",
        action="store_true",
        default=(os.name == "nt"),
        help="Invoke via wsl.exe (auto-enabled on Windows)",
    )
    stdio_p.add_argument("--distro", help="WSL distro name (-d flag for wsl.exe)")

    http_p = sub.add_parser("http", help="Test via HTTP transport")
    http_p.add_argument("--host", default="127.0.0.1")
    http_p.add_argument("--port", type=int, default=3000)

    parser.add_argument("-v", "--verbose", action="store_true", help="Print full tool output")

    return parser.parse_args()


def main() -> None:
    args = parse_args()

    bar = "━" * 60
    print(f"\n{B}{bar}{X}")
    if args.transport == "stdio":
        wsl_info = " (via wsl.exe)" if args.wsl else ""
        print(f"  {B}mcp-wsl test suite{X}  •  transport: {C}stdio{X}{wsl_info}")
        print(f"  binary: {args.binary}")
    else:
        print(f"  {B}mcp-wsl test suite{X}  •  transport: {C}http{X}  {args.host}:{args.port}")
    print(f"{B}{bar}{X}\n")

    # Build transport
    try:
        if args.transport == "stdio":
            transport: Transport = StdioTransport(
                binary=args.binary,
                wsl=args.wsl,
                distro=getattr(args, "distro", None),
            )
        else:
            transport = HttpTransport(host=args.host, port=args.port)
    except Exception as e:
        print(f"{R}Failed to connect: {e}{X}")
        sys.exit(1)

    # Run all test cases
    results: List[Result] = []
    try:
        for case in ALL_CASES:
            r = run_case(transport, case, args.verbose)
            results.append(r)
            icon = STATUS_ICON[r.status]
            # Inline progress
            print(f"  {icon}  {case.tool:<40} {DIM}{case.label}{X}", flush=True)
    finally:
        transport.close()

    failed = print_results(results, args.verbose)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
