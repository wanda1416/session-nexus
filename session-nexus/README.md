# SessionNexus

Windows desktop hub for AI agent sessions. The product design lives in [docs/plan.md](docs/plan.md).

Phase 1 connectors: Qoder CLI, Grok Build, ChatGPT Codex CLI, and DeepSeek Harness headless. Browser sessions go through the extension in `bridge/extension` and the MCP server on `127.0.0.1:19877`. See [bridge/README.md](bridge/README.md).

```
pnpm install
pnpm tauri dev
```
