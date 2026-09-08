# ghostfox (npm)

npm surface for the [Ghostfox](https://github.com/autokeren/ghostfox)
agent-native stealth browser. Zero npm dependencies.

```bash
npx ghostfox install     # one-time: engine (~650MB) + runtime into ~/.ghostfox
```

Then wire it into any MCP client (Claude Code, Cursor, ...):

```json
{
  "mcpServers": {
    "ghostcloak": { "command": "npx", "args": ["-y", "ghostfox", "mcp"] }
  }
}
```

Tools: `session_create` · `page_open` · `page_snapshot` · `page_screenshot` ·
`page_click` · `page_type` · `page_fill` · `page_press` ·
`identity_generate` · `identity_audit` · `session_evidence` · `captcha_solve`

Docs: https://autokeren.github.io/ghostfox/ ·
Docker: `ghcr.io/autokeren/ghostfox` · Python: `pip install ghostfox`
