# SessionNexus

[English](README.md)

SessionNexus 是一个 Windows 桌面应用，把本机的 AI 编程 Agent 放进同一个聊天窗口。每个 Agent 是一个联系人，每个话题是一个群。在群里发消息会调用该联系人已经安装的 CLI，并把结果写回对话。

应用不代理模型 API Key。登录态留在各个 CLI 自己的配置里。

## 现在能做什么

- 打开三栏窗口：联系人和群、对话、这个群里已经开始的工作。
- 新建群，填写名称、工作目录、项目说明，并拉入一个或多个联系人。
- 点一个联系人开单聊。单聊就是只有一个成员的群。
- 用 `@Grok`、`@Qoder`、`@DSH`、`@Codex` 点名。不带 `@` 的消息会发给本群每一个成员。
- 为每个目标运行已安装的 CLI。提示里包含群名、项目说明、工作目录和这次任务。
- 把群、成员、消息和工作记录放进本地 SQLite。重启桌面应用后还在。

## 联系人

| 联系人 | 实际命令 | 说明 |
|--------|----------|------|
| Grok | `grok -p … --output-format plain --no-subagents --permission-mode dontAsk` | Grok Build。群的工作目录存在时加上 `--cwd`。 |
| Qoder | `qodercli -p --permission-mode dont_ask …` | 用的是 `qodercli`，不是名为 `qoder` 的 IDE 启动器。目录存在时加上 `-w`。 |
| Codex | `codex exec --skip-git-repo-check --ephemeral --color never …` | ChatGPT 的 Codex CLI。目录存在时加上 `-C`。 |
| DSH | `dsh headless …` | DeepSeek Harness 的一次性任务。目录存在时，进程的工作目录设为该目录。官方 headless 每次都是新会话。 |

一次调用最多等 180 秒。工作气泡优先显示标准输出；标准输出为空时显示标准错误。超过 8000 个字符会截断。

CLI 必须已经在 `PATH` 里。找不到命令的联系人标为不可用，不能拉进群。

## 聊天模型

一个群是一个话题，例如「项目 A」。成员不共用一个全局主会话，所以另一个项目不会接着用这个项目的上下文。

产品模型里有两种会话，详见 [docs/ui.md](docs/ui.md)。

- **房间会话**：一个联系人在一个群里的长期对话。
- **工作会话**：一次任务对应一次新的 CLI 运行。由 SessionNexus 启动，不是 Agent 自己去拉子代理。

窗口里这两种记录已经分开存放。当前点击发送后，实际执行的仍是一次新的非交互命令。还不会恢复已有的房间会话，也不会先等房间会话给出调度块再开工。

## 浏览器桥

网页不走 Chrome DevTools Protocol。SessionNexus 自带扩展和本机 MCP。

- 扩展在 [bridge/extension](bridge/extension)。在已经登录的 Chrome 或 Edge 里打开 `chrome://extensions`，以未打包方式加载。
- 桌面应用运行时，MCP 在 `http://127.0.0.1:19877/mcp`。
- 扩展连接 `ws://127.0.0.1:19877/extension`。
- 工具：`list_tabs`、`find_tab`、`snapshot`、`fill`、`click`。

扩展只用 `chrome.tabs` 和 `chrome.scripting`。扩展没连上时，工具调用返回「扩展未连接」，应用不会另开浏览器。扩展不是聊天联系人。详见 [bridge/README.md](bridge/README.md)。

## 环境

- 带 WebView2 的 Windows 11
- [Rust](https://rustup.rs/) stable
- Node.js 20 或更新版本
- pnpm 9 或更新版本
- 要对话的 Agent CLI，并且已经登录

## 运行

在仓库根目录：

```powershell
pnpm install
pnpm tauri dev
```

`pnpm dev` 只打开 React 页面。这个页面看不到已安装的 CLI，也不会读写数据库。窗口会提示改用 `pnpm tauri dev`。

打包：

```powershell
pnpm tauri build
```

Rust 测试：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

## 数据

SQLite 数据库：

```text
%APPDATA%\SessionNexus\nexus.db
```

表是 `groups`、`members`、`messages`、`works`。这里只存本应用的聊天索引。Agent 自己的会话原文仍在各自的目录里。

空数据库不会自动插入示例群。用左上角的「新建群」创建。

## 目录

```text
src/                 React 聊天窗口
src-tauri/           Tauri 壳、CLI 运行、SQLite、浏览器 MCP
bridge/extension/    Manifest V3 扩展
docs/plan.md         产品与技术方案
docs/ui.md           群聊和会话模型
```

## 这一版没有

- 手机客户端，以及 `127.0.0.1:19876` 上的本地 API
- 恢复房间会话，或等房间会话提出要求后再开工作会话
- 嵌入完整终端
- 用 UI Automation 操作其他桌面程序
- WSL 里的 Agent
- 全局唯一的主会话

设计和阶段说明在 [docs/plan.md](docs/plan.md) 与 [docs/ui.md](docs/ui.md)。
