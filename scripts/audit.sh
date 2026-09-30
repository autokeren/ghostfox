#!/usr/bin/env bash
# Ghostfox release-consistency audit.
# CI enforces this so NOBODY has to remember to cross-check.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0
note() { echo "  ✓ $1"; }
warn() { echo "  ✗ $1"; fail=1; }

echo "== 1. no stale ghostcloak names (outside changelog history) =="
if grep -rni "ghostcloak" . \
    --exclude-dir=.git --exclude-dir=target --exclude-dir=node_modules \
    --exclude-dir=__pycache__ --exclude=CHANGELOG.md --exclude=JOURNEY.md \
    --exclude=audit.sh --exclude='*.pyc' 2>/dev/null | head -3; then
  warn "stale ghostcloak references found"
else
  note "clean"
fi

echo "== 2. version consistency (Cargo workspace / server.json / npm / PyPI) =="
ver_cargo=$(grep -m1 '^version = ' runtime/Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
ver_server=$(grep -m1 '"version"' server.json | sed 's/.*"\([0-9.]*\)".*/\1/')
ver_npm=$(grep -m1 '"version"' runtime/node/package.json | sed 's/.*"\([0-9.]*\)".*/\1/')
ver_pypi=$(grep -m1 '^version = ' runtime/python/pyproject.toml | sed 's/.*"\(.*\)".*/\1/')
echo "  cargo=$ver_cargo server=$ver_server npm=$ver_npm pypi=$ver_pypi"
if [ "$ver_cargo" = "$ver_server" ] && [ "$ver_cargo" = "$ver_npm" ] && [ "$ver_cargo" = "$ver_pypi" ]; then
  note "consistent ($ver_cargo)"
else
  warn "version mismatch"
fi

echo "== 3. runtime asset-name contract (workflow vs installers) =="
for name in ghostfox-mcp ghostfox-mcp-arm64 ghostfox-mcp.exe ghostfox-mcp-darwin; do
  in_workflow=$(grep -c "asset: $name" .github/workflows/runtime-build.yml || true)
  in_installers=$(grep -rl "$name" install.sh runtime/python runtime/node 2>/dev/null | wc -l)
  if [ "$in_workflow" -ge 1 ] && [ "$in_installers" -ge 1 ]; then
    note "$name in workflow + installers"
  else
    warn "$name missing (workflow=$in_workflow installers=$in_installers)"
  fi
done

echo "== 4. Dockerfile asset names =="
if grep -q "ghostfox-mcp" Dockerfile && ! grep -q "ghostcloak" Dockerfile; then
  note "docker clean"
else
  warn "docker references stale names"
fi

echo "== 5. engine zip naming contract =="
if grep -q "package_pattern.*target\[:3\]" engine/multibuild.py 2>/dev/null \
   || grep -q "lin.x86_64\|win.x86_64\|mac.x86_64" runtime/python/ghostfox/engine.py; then
  note "zip contract present"
else
  warn "engine zip contract unclear"
fi

echo "== 6. OUR patches: every hunk must touch files we own =="
# Only the patches WE authored (m2.9-*) get the hunk audit — the rest
# are inherited from upstream Camoufox and trusted as-is.
for p in engine/patches/m2.9-*.patch; do
  [ -e "$p" ] || continue
  for f in $(grep '^diff --git' "$p" | sed 's|diff --git a/||; s| b/.*||'); do
    # juggler/* additions + the known DOM/layout touchpoints are ours;
    # anything else in a patch is a foreign-hunk red flag.
    case "$f" in
      juggler/*|dom/html/HTMLCanvasElement.*|dom/canvas/OffscreenCanvasDisplayHelper.*|dom/canvas/OffscreenCanvas.*) ;;
      layout/base/PresShellForwards.h|layout/base/PresShell.cpp|layout/generic/nsIFrame.*|gfx/ipc/CrossProcessPaint.*|dom/ipc/WindowGlobalParent.*|dom/media/systemservices/video_engine/tab_capturer.cc|dom/chrome-webidl/WindowGlobalActors.webidl) ;;
      *) warn "$(basename "$p") touches unexpected file: $f" ;;
    esac
  done
done
note "patch file-scan done"

echo "== 7. README tool count vs contract test =="
n_readme=$(grep -oE 'Full tool surface \([0-9]+ tools\)' README.md | grep -oE '[0-9]+' | head -1)
n_test=$(grep -oE 'names\.len\(\), [0-9]+' runtime/crates/ghostfox-mcp/tests/mcp_stdio_contract.rs | grep -oE '[0-9]+' | head -1)
echo "  readme=$n_readme test=$n_test"
if [ "$n_readme" = "$n_test" ] && [ -n "$n_readme" ]; then
  note "tool counts agree ($n_readme)"
else
  warn "tool count mismatch"
fi

if [ "$fail" -ne 0 ]; then
  echo "AUDIT FAILED"
  exit 1
fi
echo "AUDIT PASSED"