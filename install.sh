#!/usr/bin/env bash
# Ghostfox one-command installer.
#
#   curl -fsSL https://raw.githubusercontent.com/autokeren/ghostfox/main/install.sh | bash
#
# What it does:
#   1. Downloads the prebuilt Ghostfox engine (Linux x86_64) from Releases
#   2. Installs the ghostcloak-mcp runtime (prebuilt binary, or builds from
#      source with cargo when the prebuilt one can't run)
#   3. Prints the MCP client configuration
#
# Layout: ~/.ghostfox/{engine,mcp}
set -euo pipefail

REPO="autokeren/ghostfox"
DEST="${GHOSTFOX_HOME_ROOT:-$HOME/.ghostfox}"
OS="$(uname -s)"
ARCH="$(uname -m)"

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$OS" = "Linux" ] && [ "$ARCH" = "x86_64" ] || die "prebuilt engine currently Linux x86_64 only (from-source builds for other platforms: see the repo README)"

mkdir -p "$DEST"

# --- 1) Engine -----------------------------------------------------------------
if [ -x "$DEST/engine/ghostfox-bin" ]; then
    say "engine already installed at $DEST/engine — skipping (delete it to force reinstall)"
else
    say "fetching latest release info"
    ASSET_URL="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep -o '"browser_download_url": *"[^"]*lin\.x86_64\.zip"' \
        | head -1 | sed 's/.*"\(https[^"]*\)"/\1/')" \
        || die "could not resolve the latest release (network?)"
    [ -n "${ASSET_URL:-}" ] || die "no Linux x86_64 asset on the latest release"
    say "downloading engine ($(basename "$ASSET_URL")) — ~650MB"
    curl -fL --retry 3 -o /tmp/ghostfox-engine.zip "$ASSET_URL"
    say "unpacking to $DEST/engine"
    rm -rf "$DEST/engine.tmp"
    mkdir -p "$DEST/engine.tmp"
    if command -v unzip >/dev/null 2>&1; then
        unzip -q /tmp/ghostfox-engine.zip -d "$DEST/engine.tmp"
    else
        7z x -y -o"$DEST/engine.tmp" /tmp/ghostfox-engine.zip >/dev/null
    fi
    rm -f /tmp/ghostfox-engine.zip
    # The zip unpacks as ghostfox/ — flatten it into engine/
    if [ -d "$DEST/engine.tmp/ghostfox" ]; then
        mv "$DEST/engine.tmp/ghostfox" "$DEST/engine"
        rmdir "$DEST/engine.tmp"
    else
        mv "$DEST/engine.tmp" "$DEST/engine"
    fi
    [ -x "$DEST/engine/ghostfox-bin" ] || die "engine unpacked but ghostfox-bin missing — report this"
    say "engine installed: $($DEST/engine/ghostfox-bin --version 2>/dev/null | tail -1 || echo 'unknown version')"
fi

# --- 2) Runtime (MCP server) ---------------------------------------------------
MCP_BIN="$DEST/mcp/ghostcloak-mcp"
if [ ! -x "$MCP_BIN" ]; then
    say "fetching runtime release info"
    RUNTIME_URL="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep -o '"browser_download_url": *"[^"]*ghostcloak-mcp[^"]*"' \
        | head -1 | sed 's/.*"\(https[^"]*\)"/\1/')" || true
    if [ -n "${RUNTIME_URL:-}" ] && curl -fsSL --retry 2 -o /tmp/ghostcloak-mcp "$RUNTIME_URL"; then
        mkdir -p "$DEST/mcp"
        mv /tmp/ghostcloak-mcp "$MCP_BIN"
        chmod +x "$MCP_BIN"
    elif command -v cargo >/dev/null 2>&1; then
        say "no prebuilt runtime — building from source (needs ~3 min)"
        mkdir -p "$DEST/src"
        if [ ! -d "$DEST/src/runtime" ]; then
            curl -fsSL --retry 3 "https://github.com/$REPO/archive/refs/heads/main.tar.gz" | tar -xz -C "$DEST/src"
            mv "$DEST/src/ghostfox-main" "$DEST/src/ghostfox" 2>/dev/null || true
        fi
        (cd "$DEST/src/ghostfox/runtime" && cargo build --release -p ghostcloak-mcp)
        mkdir -p "$DEST/mcp"
        cp "$DEST/src/ghostfox/runtime/target/release/ghostcloak-mcp" "$MCP_BIN"
    else
        die "no prebuilt runtime for this platform and no cargo to build one — see the repo README"
    fi
fi

# --- 3) Print the MCP config -----------------------------------------------------
cat <<EOF

\033[1;32mGhostfox installed.\033[0m

Engine:   $DEST/engine
Runtime:  $MCP_BIN

Add to your MCP client (e.g. ~/.claude.json, .mcp.json, Cursor config):

{
  "mcpServers": {
    "ghostcloak": {
      "command": "$MCP_BIN",
      "env": { "GHOSTFOX_HOME": "$DEST/engine" }
    }
  }
}

Evidence of every session is recorded under ~/.ghostfox/recordings/.
EOF
