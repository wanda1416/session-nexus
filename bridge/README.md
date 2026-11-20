# SessionNexus browser bridge

Two pieces, both ours:

- The desktop app serves MCP at `http://127.0.0.1:19877/mcp`.
- `extension/` is a Manifest V3 extension. Load it unpacked from `chrome://extensions` in the browser profile you are already signed into. It connects to `ws://127.0.0.1:19877/extension`.

The extension uses `chrome.tabs` and `chrome.scripting`. It does not open a debugging port.

Tools: `list_tabs`, `find_tab`, `snapshot`, `fill`, `click`. A call made while the extension is disconnected returns `扩展未连接`.
