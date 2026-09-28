mcp-name: io.github.autokeren/ghostfox

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

Tools: the full 43-tool surface — sessions (`session_create`, `session_pages`),
sight (`page_a11y`, `page_snapshot`, `page_screenshot`), action
(`page_click_ref`, `page_type_ref`, `page_fill`, `page_drag`, ...), the
native captcha suite (`page_geetest_click`, `page_geetest_slide`,
`page_captcha_rotate`, `page_captcha_ocr`, `page_hcaptcha`, `captcha_solve`),
the debug cortex (`page_console`, `page_errors`, `page_network_*`) and
multi-model vision helpers. Full list: see the
[main README](https://github.com/autokeren/ghostfox#full-tool-surface-43-tools).

Docs: https://autokeren.github.io/ghostfox/ ·
Docker: `ghcr.io/autokeren/ghostfox` · Python: `pip install ghostfox`
