# SessionNexus

[中文](README.zh-CN.md)

SessionNexus is a Windows desktop app that puts local AI coding agents into one chat window. Each agent is a contact. A topic is a group. Sending a message in a group runs that contact's installed CLI and writes the result back into the thread.

The app does not proxy model API keys. Each CLI keeps its own login.

## What you can do today

- Open a three-pane window: contacts and groups, the thread, and the work started in that group.
- Create a group with a name, a working directory, a short brief, and one or more contacts.
- Open a direct chat with one contact. A direct chat is a group with a single member.
- Mention someone with `@Grok`, `@Qoder`, `@DSH`, or `@Codex`. A message without a mention goes to every member of the group.
- Run the installed CLI for each target. The prompt includes the group name, the brief, the working directory, and the task.
- Keep groups, members, messages, and work rows in a local SQLite database so they are still there after a restart.

## Contacts

| Contact | Command it runs | Notes |
|---------|-----------------|-------|
| Grok | `grok -p … --output-format plain --no-subagents --permission-mode dontAsk` | Grok Build. Adds `--cwd` when the group directory exists. |
| Qoder | `qodercli -p --permission-mode dont_ask …` | Uses `qodercli`, not the Qoder IDE launcher named `qoder`. Adds `-w` when the directory exists. |
| Codex | `codex exec --skip-git-repo-check --ephemeral --color never …` | ChatGPT Codex CLI. Adds `-C` when the directory exists. |
| DSH | `dsh headless …` | DeepSeek Harness one-shot task. The process working directory is the group directory when that directory exists. Official headless mode starts a new session each time. |

A run waits up to 180 seconds. Stdout is shown in the work bubble. If stdout is empty, stderr is shown instead. Output is truncated after 8000 characters.

The CLIs must already be on `PATH`. A missing command is marked unavailable and cannot be added to a group.

## Chat model

A group is one topic, such as "Project A". Members do not share one global master session, so a second project does not reuse the first project's context.

Two session roles are part of the product model. See [docs/30-design/02-decisions/01-group-chat.md](docs/30-design/02-decisions/01-group-chat.md). Current progress is in [docs/20-plans/01-backlog.md](docs/20-plans/01-backlog.md).

- A **room session** is the long-running conversation for one contact inside one group.
- A **work session** is a new CLI run for one task. SessionNexus starts it. The agent does not spawn its own subagent for that task.

A message is delivered to each target's room session. Grok and Qoder resume that session on the next message. Codex resumes when its JSON output contains a session id. DSH headless cannot resume, so each of its room turns is a new process with the same brief.

If the room reply ends with a `dispatch` block whose JSON has a `task` field, SessionNexus strips that block from the chat and starts a fresh work run. The work run does not resume the room session. A reply without that block stays in the room and does not start work.

## Browser bridge

Web pages are not driven with the Chrome DevTools Protocol. SessionNexus ships its own extension and a local MCP server.

- Extension: [bridge/extension](bridge/extension). Load it unpacked from `chrome://extensions` in the Chrome or Edge profile you are already signed in to.
- MCP: `http://127.0.0.1:19877/mcp` while the desktop app is running.
- Extension socket: `ws://127.0.0.1:19877/extension`.
- Tools: `list_tabs`, `find_tab`, `snapshot`, `fill`, `click`.

The extension uses `chrome.tabs` and `chrome.scripting`. If it is not connected, a tool call returns `扩展未连接` and the app does not launch another browser. The extension is not a chat contact. See [bridge/README.md](bridge/README.md).

## Requirements

- Windows 11 with WebView2
- [Rust](https://rustup.rs/) stable
- Node.js 20 or newer
- pnpm 9 or newer
- The agent CLIs you want to talk to, already logged in

## Run

From this repository root:

```powershell
pnpm install
pnpm tauri dev
```

`pnpm dev` only serves the React view. That view cannot see installed CLIs or the database. The window tells you to start `pnpm tauri dev` instead.

Production build:

```powershell
pnpm tauri build
```

Rust tests:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

## Data

SQLite database:

```text
%APPDATA%\SessionNexus\nexus.db
```

Tables are `groups`, `members`, `messages`, and `works`. The database is an index of this app's chats. Native agent transcripts stay in each CLI's own files.

An empty database does not insert a sample group. Create one with **新建群**.

## Repository

```text
src/                 React chat window
src-tauri/           Tauri shell, CLI runner, SQLite, browser MCP
bridge/extension/    Manifest V3 extension
docs/                Layered notes. Start at docs/README.md
```

## Not in this build

- Phone client and the loopback API on `127.0.0.1:19876`
- Embedding a full terminal
- UI Automation for other desktop apps
- WSL agents
- A global master session

Design notes and the progress backlog live under [docs/README.md](docs/README.md).
