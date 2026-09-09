"""Synchronous Python client for the Ghostfox MCP runtime (ghostcloak-mcp).

The runtime is the same MCP server AI agents use; this client gives plain
Python the same surface:

    from ghostfox import GhostfoxMCP

    fox = GhostfoxMCP()                      # finds/downloads nothing; requires runtime+engine
    sid = fox.session_create(platform="android")
    page = fox.page_open(sid, "https://example.com")
    print(fox.page_snapshot(sid, page)["content"])
    fox.close()
"""

from __future__ import annotations

import json
import os
import subprocess
import time
from pathlib import Path
from typing import Any

from .engine import default_runtime


class McpError(RuntimeError):
    pass


class GhostfoxMCP:
    """Drive the Ghostfox runtime over MCP stdio from Python."""

    def __init__(
        self,
        runtime: Path | str | None = None,
        engine_home: Path | str | None = None,
        env: dict[str, str] | None = None,
    ) -> None:
        self.runtime = Path(runtime) if runtime else default_runtime()
        self.env = dict(os.environ)
        self.env["GHOSTFOX_HOME"] = str(engine_home or os.environ.get("GHOSTFOX_HOME") or Path.home() / ".ghostfox" / "engine")
        if env:
            self.env.update(env)
        self.env.setdefault("RUST_LOG", "warn")

        self.proc = subprocess.Popen(
            [str(self.runtime)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
            env=self.env,
        )
        self._id = 0
        self._call("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "ghostfox-py", "version": "0.1.0"},
        })
        self._notify("notifications/initialized")

    # -- protocol plumbing ---------------------------------------------------

    def _send(self, method: str, params: dict | None = None, notify: bool = False) -> None:
        msg: dict[str, Any] = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            msg["params"] = params
        if not notify:
            self._id += 1
            msg["id"] = self._id
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()

    def _recv(self, want: int, timeout: float = 120.0) -> dict:
        deadline = time.time() + timeout
        while time.time() < deadline:
            line = self.proc.stdout.readline()
            if not line:
                raise McpError("runtime closed the connection")
            line = line.strip()
            if not line:
                continue
            try:
                resp = json.loads(line)
            except json.JSONDecodeError:
                continue
            if resp.get("id") == want:
                return resp
        raise McpError(f"timeout waiting for response {want}")

    def _call(self, method: str, params: dict | None = None, timeout: float = 120.0) -> Any:
        self._send(method, params)
        resp = self._recv(self._id, timeout)
        if "error" in resp:
            raise McpError(f"{method}: {resp['error'].get('message')}")
        return resp.get("result")

    def _notify(self, method: str) -> None:
        self._send(method, notify=True)

    def _tool(self, name: str, args: dict, timeout: float = 180.0) -> str:
        result = self._call("tools/call", {"name": name, "arguments": args}, timeout)
        if result.get("isError"):
            texts = [c.get("text", "") for c in result.get("content", [])]
            raise McpError(f"{name}: {' '.join(texts)}")
        return result["content"][0]["text"]

    # -- the browser surface ---------------------------------------------------

    def session_create(self, platform: str | None = None, profile_dir: str | None = None, proxy: str | None = None, headful: bool = False) -> str:
        args: dict[str, Any] = {}
        if platform:
            args["platform"] = platform
        if profile_dir:
            args["profile_dir"] = profile_dir
        if proxy:
            args["proxy"] = proxy
        if headful:
            args["headful"] = True
        return self._tool("session_create", args)

    def page_open(self, session_id: str, url: str) -> str:
        return self._tool("page_open", {"session_id": session_id, "url": url})

    def page_snapshot(self, session_id: str, page_id: str) -> dict:
        return json.loads(self._tool("page_snapshot", {"session_id": session_id, "page_id": page_id}))

    def page_a11y(self, session_id: str, page_id: str) -> list:
        return json.loads(self._tool("page_a11y", {"session_id": session_id, "page_id": page_id}))

    def page_click_ref(self, session_id: str, page_id: str, ref: str):
        return self._tool("page_click_ref", {"session_id": session_id, "page_id": page_id, "ref": ref})

    def page_type_ref(self, session_id: str, page_id: str, ref: str, text: str):
        return self._tool("page_type_ref", {"session_id": session_id, "page_id": page_id, "ref": ref, "text": text})

    def page_eval(self, session_id: str, page_id: str, expression: str):
        return json.loads(self._tool("page_eval", {"session_id": session_id, "page_id": page_id, "expression": expression}))

    def page_screenshot(self, session_id: str, page_id: str, full_page: bool = False) -> dict:
        return json.loads(self._tool("page_screenshot", {"session_id": session_id, "page_id": page_id, "full_page": full_page}))

    def page_click(self, session_id: str, page_id: str, selector: str) -> None:
        self._tool("page_click", {"session_id": session_id, "page_id": page_id, "selector": selector})

    def page_type(self, session_id: str, page_id: str, selector: str, text: str) -> None:
        self._tool("page_type", {"session_id": session_id, "page_id": page_id, "selector": selector, "text": text})

    def page_fill(self, session_id: str, page_id: str, selector: str, text: str):
        return json.loads(self._tool("page_fill", {"session_id": session_id, "page_id": page_id, "selector": selector, "text": text}))

    def page_press(self, session_id: str, page_id: str, key: str) -> None:
        self._tool("page_press", {"session_id": session_id, "page_id": page_id, "key": key})

    def identity_generate(self) -> str:
        return self._tool("identity_generate", {})

    def identity_audit(self, identity_toml: str) -> str:
        return self._tool("identity_audit", {"identity_toml": identity_toml})

    def session_evidence(self, session_id: str) -> dict:
        return json.loads(self._tool("session_evidence", {"session_id": session_id}))

    def close(self) -> None:
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
