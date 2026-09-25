# Ghostfox runtime image: engine + MCP server, engine-first entrypoint.
#
# Build (from repo root):
#   docker build -t ghcr.io/autokeren/ghostfox:latest -f Dockerfile .
# Run:
#   docker run -p 7900:7900 -e GHOSTFOX_LIVE_VIEW_PORT=7900 ghcr.io/autokeren/ghostfox
#
# The MCP server speaks stdio; for containerized use, mount a socket or run
# it under an MCP gateway. For quick inspection, GHOSTFOX_LIVE_VIEW_PORT
# serves the live view on the mapped port.
#
# Base is ubuntu:24.04 (noble): the prebuilt ghostcloak-mcp binary links
# against the onnxruntime static libs from `ort` download-binaries, which
# require glibc >= 2.38 / libstdc++ from GCC 13 — bookworm (glibc 2.36)
# cannot satisfy them.

FROM ubuntu:24.04 AS runtime

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl unzip \
        libgtk-3-0t64 libdbus-glib-1-2 libxt6 libasound2t64 \
        libx11-xcb1 libxcomposite1 libxdamage1 libxrandr2 \
        libxss1 libxcursor1 libxinerama1 \
        libstdc++6 libssl3t64 \
    && rm -rf /var/lib/apt/lists/*

# Engine + runtime from the matching GitHub release (pinned by build arg).
ARG GHOSTFOX_VERSION=v0.1.0
ARG GHOSTFOX_REPO=autokeren/ghostfox
RUN curl -fsSL "https://api.github.com/repos/${GHOSTFOX_REPO}/releases/tags/${GHOSTFOX_VERSION}" \
        -o /tmp/rel.json \
 && ENGINE_URL=$(grep -o '"browser_download_url": *"[^"]*lin\.x86_64\.zip"' /tmp/rel.json | head -1 | sed 's/.*"\(https[^"]*\)"/\1/') \
 && RUNTIME_URL=$(grep -o '"browser_download_url": *"[^"]*ghostcloak-mcp"' /tmp/rel.json | head -1 | sed 's/.*"\(https[^"]*\)"/\1/') \
 && test -n "${ENGINE_URL:-}" && test -n "${RUNTIME_URL:-}" \
 && curl -fL --retry 3 "$ENGINE_URL" -o /tmp/engine.zip \
 && mkdir -p /opt/ghostfox/engine \
 && unzip -q /tmp/engine.zip -d /tmp/engine-unpack \
 \
# The engine zip has no top-level dir: its files land directly in the
# unpack dir. Support both layouts (flat, or wrapped in ghostfox/).
 && if [ -d /tmp/engine-unpack/ghostfox ]; then \
        mv /tmp/engine-unpack/ghostfox/* /opt/ghostfox/engine/; \
    else \
        mv /tmp/engine-unpack/* /opt/ghostfox/engine/; \
    fi \
 && chmod +x /opt/ghostfox/engine/ghostfox-bin \
 && mkdir -p /opt/ghostfox/mcp \
 && curl -fL --retry 3 "$RUNTIME_URL" -o /opt/ghostfox/mcp/ghostcloak-mcp \
 && chmod +x /opt/ghostfox/mcp/ghostcloak-mcp \
 && rm -rf /tmp/*

ENV GHOSTFOX_HOME=/opt/ghostfox/engine \
    GHOSTFOX_RECORDINGS=/data/recordings \
    RUST_LOG=warn

VOLUME /data
WORKDIR /data
EXPOSE 7900

ENTRYPOINT ["/opt/ghostfox/mcp/ghostcloak-mcp"]
