# Ghostfox one-command installer (Windows PowerShell).
#
#   powershell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/autokeren/ghostfox/main/install.ps1 | iex"
#
# What it does:
#   1. Downloads the prebuilt Ghostfox engine (win.x86_64) from Releases
#   2. Installs the ghostfox-mcp.exe runtime
#   3. Prints the MCP client configuration
#
# Layout: ~\.ghostfox\{engine,mcp}
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$Repo = "autokeren/ghostfox"
$Root = if ($env:GHOSTFOX_HOME_ROOT) { $env:GHOSTFOX_HOME_ROOT } else { Join-Path $HOME ".ghostfox" }
$EngineDir = Join-Path $Root "engine"
$McpDir = Join-Path $Root "mcp"

function Get-Assets {
    $rel = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "ghostfox-ps" }
    return $rel
}

$rel = Get-Assets
$engine = $rel.assets | Where-Object { $_.name -like "*win.x86_64*.zip" } | Select-Object -First 1
if (-not $engine) { throw "no Windows x86_64 engine asset in latest release" }

if (-not (Test-Path (Join-Path $EngineDir "ghostfox.exe"))) {
    Write-Host "downloading $($engine.name) ..."
    $zip = Join-Path $env:TEMP $engine.name
    Invoke-WebRequest -Uri $engine.browser_download_url -OutFile $zip -Headers @{ "User-Agent" = "ghostfox-ps" }
    $tmp = Join-Path $env:TEMP "ghostfox-engine-unpack"
    if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
    Expand-Archive -Path $zip -DestinationPath $tmp
    New-Item -ItemType Directory -Force -Path $EngineDir | Out-Null
    if (Test-Path $EngineDir) { Remove-Item -Recurse -Force $EngineDir }
    $inner = Join-Path $tmp "ghostfox"
    if (Test-Path $inner) { Move-Item $inner $EngineDir } else { Move-Item $tmp $EngineDir }
    Remove-Item -Force $zip -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    Write-Host "engine installed at $EngineDir"
}

if (-not (Test-Path (Join-Path $McpDir "ghostfox-mcp.exe"))) {
    $rt = $rel.assets | Where-Object { $_.name -eq "ghostfox-mcp.exe" } | Select-Object -First 1
    if (-not $rt) { throw "no ghostfox-mcp.exe asset in latest release" }
    Write-Host "downloading ghostfox-mcp.exe ..."
    New-Item -ItemType Directory -Force -Path $McpDir | Out-Null
    Invoke-WebRequest -Uri $rt.browser_download_url -OutFile (Join-Path $McpDir "ghostfox-mcp.exe") -Headers @{ "User-Agent" = "ghostfox-ps" }
}

Write-Host ""
Write-Host "Ghostfox installed. MCP client config:"
Write-Host ""
Write-Host "  {"
Write-Host "    \"mcpServers\": {"
Write-Host "      \"ghostfox\": {"
Write-Host "        \"type\": \"local\","
Write-Host "        \"command\": [\"$(Join-Path $McpDir 'ghostfox-mcp.exe')\"],"
Write-Host "        \"environment\": {"
Write-Host "          \"GHOSTFOX_HOME\": \"$(Join-Path $EngineDir '')/\""
Write-Host "        }"
Write-Host "      }"
Write-Host "    }"
Write-Host "  }"
