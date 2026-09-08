"""Ghostfox — the agent-native stealth browser you can own.

Python surface for the Ghostfox stack:
- ``ghostfox.install_engine()`` — download + unpack the prebuilt engine
- ``ghostfox.GhostfoxMCP`` — drive the runtime (the same MCP server agents
  use) from plain Python

The runtime is a Rust binary (``ghostcloak-mcp``); this package wraps it so
the Python world gets the same one-command story as ``pip install browser-use``.
"""

from .mcp import GhostfoxMCP, McpError
from .engine import install_engine, engine_home

__version__ = "0.2.0"
__all__ = ["GhostfoxMCP", "McpError", "install_engine", "engine_home"]
