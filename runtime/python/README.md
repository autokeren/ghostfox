# ghostfox (Python)

Python surface for the [Ghostfox](https://github.com/autokeren/ghostfox)
agent-native stealth browser.

```python
import ghostfox

# one-time: download the prebuilt engine + runtime (Linux x86_64)
ghostfox.install_engine()
ghostfox.install_runtime()

fox = ghostfox.GhostfoxMCP()
sid  = fox.session_create(platform="android")   # coherent mobile persona
page = fox.page_open(sid, "https://example.com")
print(fox.page_snapshot(sid, page)["content"])
fox.page_screenshot(sid, page)                   # evidence + live view
fox.close()
```

Zero Python dependencies — it drives the Rust MCP runtime over stdio, the
same server AI agents use.
