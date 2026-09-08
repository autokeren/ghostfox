"""Engine installer: fetch the prebuilt Ghostfox engine from GitHub Releases.

Layout after install: ``~/.ghostfox/engine`` (override the root with
``GHOSTFOX_HOME_ROOT``). Point ``GHOSTFOX_HOME`` at the returned path when
launching the runtime.
"""

from __future__ import annotations

import io
import json
import os
import shutil
import sys
import urllib.request
import zipfile
from pathlib import Path

REPO = "autokeren/ghostfox"


def _latest_release() -> dict:
    url = f"https://api.github.com/repos/{REPO}//releases/latest"
    req = urllib.request.Request(url, headers={"User-Agent": "ghostfox-py"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def engine_home() -> Path:
    """Where the engine is (or will be) installed."""
    root = Path(os.environ.get("GHOSTFOX_HOME_ROOT", Path.home() / ".ghostfox"))
    return root / "engine"


def install_engine(dest: Path | None = None, quiet: bool = False) -> Path:
    """Download and unpack the latest prebuilt engine. Returns its path."""
    dest = dest or engine_home()
    if (dest / "ghostfox-bin").exists():
        if not quiet:
            print(f"engine already installed at {dest}", file=sys.stderr)
        return dest

    rel = _latest_release()
    assets = {
        a["name"]: a["browser_download_url"] for a in rel.get("assets", [])
    }
    url = next(
        (u for n, u in assets.items() if "lin.x86_64" in n and n.endswith(".zip")),
        None,
    )
    if not url:
        raise RuntimeError(f"no Linux x86_64 engine asset in release {rel.get('tag_name')}")

    if not quiet:
        print(f"downloading {url.split('/')[-1]} ...", file=sys.stderr)
    req = urllib.request.Request(url, headers={"User-Agent": "ghostfox-py"})
    data = urllib.request.urlopen(req, timeout=600).read()

    tmp = dest.parent / ".engine-download"
    shutil.rmtree(tmp, ignore_errors=True)
    tmp.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        zf.extractall(tmp)

    src = tmp / "ghostfox" if (tmp / "ghostfox").is_dir() else tmp
    shutil.rmtree(dest, ignore_errors=True)
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(src), dest)
    shutil.rmtree(tmp, ignore_errors=True)
    if not quiet:
        print(f"engine installed at {dest}", file=sys.stderr)
    return dest


def install_runtime(dest: Path | None = None, quiet: bool = False) -> Path:
    """Download the prebuilt ``ghostcloak-mcp`` runtime binary (Linux x86_64)."""
    dest = dest or engine_home().parent / "mcp" / "ghostcloak-mcp"
    if dest.exists():
        return dest
    rel = _latest_release()
    url = next(
        (a["browser_download_url"] for a in rel.get("assets", []) if a["name"] == "ghostcloak-mcp"),
        None,
    )
    if not url:
        raise RuntimeError("no prebuilt runtime asset — build it with cargo (see README)")
    if not quiet:
        print("downloading ghostcloak-mcp ...", file=sys.stderr)
    req = urllib.request.Request(url, headers={"User-Agent": "ghostfox-py"})
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(urllib.request.urlopen(req, timeout=120).read())
    dest.chmod(0o755)
    return dest


def default_runtime() -> Path:
    """Find a runtime: $GHOSTFOX_MCP, the installed copy, or one on PATH."""
    if v := os.environ.get("GHOSTFOX_MCP"):
        p = Path(v)
        if p.exists():
            return p
    installed = engine_home().parent / "mcp" / "ghostcloak-mcp"
    if installed.exists():
        return installed
    if shutil.which("ghostcloak-mcp"):
        return Path(shutil.which("ghostcloak-mcp"))
    raise RuntimeError(
        "runtime not found: set GHOSTFOX_MCP, or call ghostfox.install_runtime()"
    )

