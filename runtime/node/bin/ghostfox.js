#!/usr/bin/env node
// ghostfox — CLI + MCP launcher for the Ghostfox agent browser.
//
// Commands:
//   ghostfox install    download the engine + runtime into ~/.ghostfox
//   ghostfox mcp        run the MCP server (stdio) — for MCP client configs
//   ghostfox config     print the MCP client JSON snippet
//   ghostfox version    print versions
//
// Zero npm dependencies; only Node stdlib.

const { spawn, execFileSync } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const https = require("https");
const zlib = require("zlib");

const REPO = "autokeren/ghostfox";
const ROOT = process.env.GHOSTFOX_ROOT || path.join(os.homedir(), ".ghostfox");
const ENGINE = path.join(ROOT, "engine");
const MCP_BIN = path.join(ROOT, "mcp", "ghostcloak-mcp");

function log(msg) { process.stderr.write(`ghostfox: ${msg}\n`); }

function fetchJson(url) {
  return new Promise((resolve, reject) => {
    https.get(url, { headers: { "User-Agent": "ghostfox-npm" } }, (res) => {
      if (res.statusCode >= 300 && res.headers.location) {
        return fetchJson(res.headers.location).then(resolve, reject);
      }
      if (res.statusCode !== 200) return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      let data = "";
      res.on("data", (c) => (data += c));
      res.on("end", () => { try { resolve(JSON.parse(data)); } catch (e) { reject(e); } });
    }).on("error", reject);
  });
}

function download(url, dest, redirectCount = 0) {
  return new Promise((resolve, reject) => {
    if (redirectCount > 5) return reject(new Error("too many redirects"));
    https.get(url, { headers: { "User-Agent": "ghostfox-npm" } }, (res) => {
      if (res.statusCode >= 300 && res.headers.location) {
        return download(res.headers.location, dest, redirectCount + 1).then(resolve, reject);
      }
      if (res.statusCode !== 200) return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      const file = fs.createWriteStream(dest);
      res.pipe(file);
      file.on("finish", () => file.close(resolve));
      file.on("error", reject);
    }).on("error", reject);
  });
}

function unzip(zipPath, dest) {
  fs.mkdirSync(dest, { recursive: true });
  try {
    execFileSync("unzip", ["-q", "-o", zipPath, "-d", dest], { stdio: "inherit" });
  } catch {
    execFileSync("7z", ["x", "-y", `-o${dest}`, zipPath], { stdio: "inherit" });
  }
}

async function install() {
  const rel = await fetchJson(`https://api.github.com/repos/${REPO}/releases/latest`);
  const assets = Object.fromEntries(rel.assets.map((a) => [a.name, a.browser_download_url]));

  if (!fs.existsSync(MCP_BIN)) {
    const url = assets["ghostcloak-mcp"];
    if (!url) throw new Error("no prebuilt runtime asset on the latest release — build it from source (see the repo README)");
    log("downloading ghostcloak-mcp runtime...");
    fs.mkdirSync(path.dirname(MCP_BIN), { recursive: true });
    await download(url, MCP_BIN);
    fs.chmodSync(MCP_BIN, 0o755);
  }

  if (!fs.existsSync(path.join(ENGINE, "ghostfox-bin"))) {
    const platform = os.platform() === "linux" ? "lin" : os.platform() === "darwin" ? "mac" : "win";
    const arch = os.arch() === "arm64" ? "arm64" : "x86_64";
    const asset = Object.keys(assets).find((n) => n.includes(`${platform}.${arch}`) && n.endsWith(".zip"));
    if (!asset) throw new Error(`no prebuilt engine for ${platform}-${arch} on the latest release`);
    log(`downloading engine (${asset}) — this is ~650MB...`);
    const tmp = path.join(os.tmpdir(), "ghostfox-engine.zip");
    await download(assets[asset], tmp);
    const stage = path.join(ROOT, ".engine-unpack");
    fs.rmSync(stage, { recursive: true, force: true });
    unzip(tmp, stage);
    fs.rmSync(ENGINE, { recursive: true, force: true });
    const inner = fs.existsSync(path.join(stage, "ghostfox")) ? path.join(stage, "ghostfox") : stage;
    fs.mkdirSync(ROOT, { recursive: true });
    fs.renameSync(inner, ENGINE);
    fs.rmSync(stage, { recursive: true, force: true });
    fs.rmSync(tmp, { force: true });
  }
  log(`installed: engine ${ENGINE}`);
}

function mcp() {
  if (!fs.existsSync(MCP_BIN)) {
    log("runtime not installed — run `ghostfox install` first");
    process.exit(1);
  }
  const child = spawn(MCP_BIN, [], {
    env: {
      ...process.env,
      GHOSTFOX_HOME: process.env.GHOSTFOX_HOME || ENGINE,
    },
    stdio: "inherit",
  });
  child.on("exit", (code) => process.exit(code || 0));
}

function config() {
  const home = process.env.GHOSTFOX_HOME || ENGINE;
  process.stdout.write(
    JSON.stringify(
      {
        mcpServers: {
          ghostcloak: { command: "npx", args: ["-y", "ghostfox", "mcp"], env: { GHOSTFOX_HOME: home } },
        },
      },
      null,
      2
    ) + "\n"
  );
}

async function main() {
  const cmd = process.argv[2] || "help";
  switch (cmd) {
    case "install":
      await install();
      break;
    case "mcp":
      mcp();
      break;
    case "config":
      config();
      break;
    case "version":
      process.stdout.write(`ghostfox npm wrapper 0.2.0\n`);
      try {
        const out = execFileSync(MCP_BIN, ["--version"], { encoding: "utf8" });
        process.stdout.write(out);
      } catch {}
      break;
    default:
      process.stdout.write(`ghostfox — the agent-native stealth browser you can own

usage:
  ghostfox install    download engine + runtime into ~/.ghostfox
  ghostfox mcp        run the MCP server (stdio)
  ghostfox config     print the MCP client JSON snippet
  ghostfox version    print versions

repo: https://github.com/${REPO}
docs: https://autokeren.github.io/ghostfox/
`);
      process.exit(cmd === "help" ? 0 : 1);
  }
}

main().catch((e) => {
  log(e.message);
  process.exit(1);
});
