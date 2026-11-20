# SessionNexus 完整技术方案

## 1. 项目定位

**SessionNexus** 是一个运行在 Windows 11 上的本地桌面应用，用于统一发现、观察、调度和管理多种 AI Agent 的会话。

核心原则：
- 优先使用 Agent 自身公开能力（CLI 命令、浏览器插件的 MCP、Accessibility API）。
- CLI/TUI 类 Agent 允许自己启动并通过 PTY 控制。
- Web 与桌面端 Agent 优先尝试附着已有已登录实例。
- 提供桌面对话窗口形态，同时支持 Headless 运行。
- 内置本地 API，供移动端或外部客户端控制。

## 2. 技术栈

| 层级 | 选型 | 说明 |
|------|------|------|
| 语言 | Rust | 核心逻辑、PTY、系统集成 |
| 桌面框架 | Tauri 2.x | Rust 后端 + 系统 WebView |
| 前端 | React 18 + TypeScript + Tailwind CSS | 对话窗口 UI |
| 状态管理 | Zustand | 前端轻量状态 |
| PTY | `portable-pty` | 管理 CLI Agent 进程 |
| 数据库 | SQLite + `sqlx` | 会话注册表、历史 |
| HTTP/WS | `axum` + `tokio-tungstenite` | 本地 API 与实时推送 |
| Web Agent | 自有 MV3 扩展 + 本机 MCP | 操作用户已登录的浏览器。不使用 CDP，不依赖第三方扩展 |
| 桌面 Agent | `windows` crate（UIA） | Windows Accessibility |
| 构建 | Cargo + pnpm | 标准 Tauri 工程 |

## 3. 系统架构

```
┌─────────────────────────────────────────┐
│           Tauri 前端 (React)             │
│   对话窗口 / 会话列表 / 状态监控         │
└──────────────────┬──────────────────────┘
                   │ Tauri Commands
┌──────────────────▼──────────────────────┐
│           Rust Core (lib)                │
│  ┌─────────────┐  ┌──────────────────┐  │
│  │ Session     │  │  Scheduler /     │  │
│  │ Registry    │  │  Router          │  │
│  └──────┬──────┘  └────────┬─────────┘  │
│         │                  │            │
│  ┌──────▼──────────────────▼─────────┐  │
│  │        Connector Layer            │  │
│  │  CLI │ Browser MCP │ Desktop      │  │
│  └───────────────────────────────────┘  │
└──────────────────┬──────────────────────┘
                   │
        ┌──────────┼──────────┐
        ▼          ▼          ▼
   CLI Agents   Browser      Desktop
   (PTY)       (插件 MCP)  (UIA)
```

本地同时启动 `axum` 服务（默认 `127.0.0.1:19876`），提供 HTTP + WebSocket，供移动端调用。

## 4. 核心数据模型

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,                    // UUID
    pub agent_type: String,            // "claude-code", "codex", "grok-web", "cursor", ...
    pub title: String,
    pub status: SessionStatus,         // Idle, Working, WaitingInput, Error, Finished
    pub connector_id: String,
    pub process_id: Option<u32>,
    pub window_handle: Option<isize>,
    pub url: Option<String>,
    pub working_dir: Option<String>,
    pub last_user_message: Option<String>,
    pub last_reply_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Idle,
    Working,
    WaitingInput,
    Error,
    Finished,
}
```

## 5. Connector 抽象

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;

    async fn discover(&self) -> Result<Vec<Session>>;
    async fn read_state(&self, session: &Session) -> Result<Session>;
    async fn send_message(&self, session: &Session, message: &str) -> Result<()>;
    async fn wait_for_completion(&self, session: &Session, timeout: Duration) -> Result<Session>;
    async fn create_session(&self, opts: CreateSessionOptions) -> Result<Session>;
}
```

### 5.1 CLI Connector（优先实现）

第一批 Provider：

| id | 产品 | 可执行文件 | 非交互 | 恢复 | 会话目录 |
|----|------|------------|--------|------|----------|
| `qoder` | Qoder CLI | `qoder` | `qoder -p "{prompt}"` | `-r {session}` | `%USERPROFILE%\.qoder` |
| `grok` | Grok Build | `grok` | `grok -p "{prompt}" --output-format json` | `-r {session}` | `%USERPROFILE%\.grok\sessions` |
| `codex` | ChatGPT Codex CLI | `codex` | `codex exec "{prompt}"` | `exec resume {session}` | `%USERPROFILE%\.codex\sessions` |
| `dsh` | DeepSeek Harness | `dsh` | `dsh --profile headless "{prompt}"` | 官方 headless 每次新会话，不恢复 | `%USERPROFILE%\.dsh\sessions` |

发消息走上表的非交互命令。PTY 只在用户要看原始 TUI 时使用。会话发现读会话目录，不刮屏。Qoder 的会话目录文档没有写死，当前按 `%USERPROFILE%\.qoder` 探测，对不上时再改 Profile。

### 5.2 浏览器 Connector：自有扩展 + MCP

CDP 不作为浏览器方案。调试端口通常要求单独启动浏览器，或连不上用户平时开着的窗口，因此带不走现有登录态。Playwright 也是同一条路。

也不依赖 Kimi WebBridge 或任何带厂商绑定的插件。SessionNexus 自己带两件东西：

1. **浏览器扩展**（Manifest V3）。用户把它装进日常使用的 Chrome 或 Edge。扩展用 `chrome.tabs` 和 `chrome.scripting` 操作已经打开的标签，Cookie 和登录态留在原配置里。不用 `chrome.debugger`，也不要求远程调试端口。
2. **本机 MCP 服务**。扩展连到 `127.0.0.1`。SessionNexus 只作为这个 MCP 的客户端。

工具面保持很小：`list_tabs`、`find_tab`、`snapshot`、`fill`、`click`。扩展没连上时，连接器报「扩展未连接」，不另开浏览器。

不引用现成项目当蓝本。BrowserMCP（`BrowserMCP/mcp`）仍在更新，但一条操作要经过「AI 进程里的 MCP → 每用户一个后台进程 → 扩展」，扩展再用 `chrome.debugger` 把命令交给 Playwright，读代码时不容易分清哪一层在做什么。`hangwin/mcp-chrome` 已经停止维护。两者都不进入实现，也不作为安装前提。

扩展和 MCP 服务都写在本仓库里，链路只有两段：SessionNexus 调用本机 MCP，MCP 把工具转给扩展。扩展只用 `chrome.tabs` 和 `chrome.scripting`。

| 字段 | 值 |
|------|-----|
| id | `browser-mcp` |
| 传输 | 我们自己的 MCP，默认 `http://127.0.0.1:19877/mcp` |
| 期望工具 | `list_tabs`、`find_tab`、`snapshot`、`fill`、`click` |
| 登录态 | 扩展所在浏览器的现有配置 |

DSH 网页如果用户已经在这个浏览器里打开，用 `find_tab` 复用那个标签。`dsh` 这个 CLI Provider 仍只负责 headless。

### 5.3 Desktop Connector
- 使用 Windows UI Automation 枚举窗口。
- 根据窗口标题、进程名匹配目标 Agent。
- 通过 UIA 查找输入框并发送文本，读取消息区域。
- 稳定性较低，作为补充能力。

## 6. 会话调度

不做全局 Master Session。一个群就是一个话题。群里每个 Agent 有自己的房间会话，干活时由我们的调度接口另开工作会话。见 [ui.md](ui.md)。

## 7. 前端界面

本地窗口按企业聊天软件做：联系人、群、群消息。见 [ui.md](ui.md)。

## 8. 本地 API（供移动端控制）

默认监听 `127.0.0.1:19876`。

主要接口：
- `GET  /api/sessions` — 列出所有会话
- `POST /api/sessions` — 创建新会话
- `POST /api/sessions/{id}/message` — 发送消息
- `GET  /api/sessions/{id}/state` — 获取当前状态
- `WS   /ws` — 实时状态与消息推送

安全：仅监听本地回环地址；可选简单 Token 认证。

## 9. 项目结构

```
./
├── docs/
│   └── plan.md                 # 本方案
├── src-tauri/                  # Rust 后端
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── models/
│   │   ├── connectors/
│   │   │   ├── mod.rs
│   │   │   ├── cli.rs
│   │   │   ├── web.rs
│   │   │   └── desktop.rs
│   │   ├── registry.rs
│   │   ├── api.rs
│   │   └── commands.rs         # Tauri commands
│   └── Cargo.toml
├── src/                        # React 前端
│   ├── components/
│   ├── pages/
│   ├── stores/
│   └── App.tsx
├── package.json
└── README.md
```

## 10. 实施阶段

**Phase 1（MVP）**
- Tauri + React 骨架
- CLI Connector：Qoder、Grok Build、ChatGPT Codex
- DSH headless 发送
- 自有浏览器扩展和它的 MCP 服务。不使用 CDP，不依赖第三方扩展
- SQLite 会话注册表
- 基本对话窗口
- 本地 API + WebSocket

**Phase 2**
- 用 `browser-mcp` 对已打开的网页会话发消息（含 DSH Web）
- 扩展的安装与连接状态写进设置页
- 会话状态更精确的解析
- 移动端 Web 界面（响应式）

**Phase 3**
- Desktop UIA Connector
- Master Session 调度
- 通知、历史搜索、多工作区

## 11. 关键约束与注意事项

- 所有对 Agent 的操作必须通过公开接口完成，禁止注入与逆向。
- CLI Agent 的会话命令需按具体 Agent 适配（做成可配置的 Provider Profile）。
- Windows 上 PTY 使用 `portable-pty` 的 ConPTY 后端。
- Web 与 Desktop 附着失败时应有清晰降级提示，不阻塞 CLI 功能。
- 本地 API 默认只绑定回环地址。


## 12. 参考项目

以下项目与 SessionNexus 的目标高度相关，建议在实现时参考其架构、PTY 管理方式、会话模型或状态解析逻辑。

### 核心参考（CLI 多会话管理）

| 项目 | 地址 | 可参考点 | 语言 | 备注 |
|------|------|----------|------|------|
| **CLI Agent Orchestrator** | [awslabs/cli-agent-orchestrator](https://github.com/awslabs/cli-agent-orchestrator) | 多 Agent 通过 tmux 隔离会话、Supervisor 委派模式、Provider 抽象 | Python | AWS 出品，1.6k+ stars，最接近「自己启动 + 统一调度」 |
| **agent-manager** | [wainmann/agent-manager](https://github.com/wainmann/agent-manager) | 不 attach 也能发送 prompt、实时状态着色、worktree 集成、MCP 工具暴露 | Go | 目前最成熟的终端多路复用器，支持 13+ Agent |
| **ReevesAgents** | [mertkayacs/reevesagents](https://github.com/mertkayacs/reevesagents) | 一个 Agent 通过 MCP 控制其他 Agent、支持 25 个 CLI、原生 Windows 路径 | - | 有 MCP 控制能力，Windows 支持较好 |
| **Fleet** | [brizzai/fleet](https://github.com/brizzai/fleet) | 轻量 TUI 并行管理多个 Claude/Codex/OpenCode 会话 | - | 结构简单，适合参考会话列表与切换逻辑 |
| **orchestmux** | [layl-labs/orchestmux](https://github.com/layl-labs/orchestmux) | 以 Claude Code 作为协调者，通过 tmux 调度其他 Agent | TypeScript | 适合参考「Master 会话」调度模式 |
| **ae (agentic engineering)** | [clemens33/ae](https://github.com/clemens33/ae) | Agent 间互相发消息、共享状态、会话持久化与恢复 | - | 适合参考跨会话通信与状态同步 |

### 会话发现与历史管理

| 项目 | 地址 | 可参考点 | 语言 | 备注 |
|------|------|----------|------|------|
| **Agent Session Hub** | [vinzify/Agent-Session-Hub](https://github.com/vinzify/Agent-Session-Hub) | 读取本地会话文件（Claude/Codex/OpenCode）、fzf 式选择与恢复 | Rust | 与我们技术栈一致，可直接参考会话文件解析 |
| **agent_hub** | [someonegg/agent_hub](https://github.com/someonegg/agent_hub) | 读取 `~/.claude` / `~/.codex` 会话历史 + tmux 实时交互、Web 界面 | TypeScript | 适合参考会话历史解析与 Web 控制台设计 |
| **AgentHub (macOS)** | [jamesrochabrun/AgentHub](https://github.com/jamesrochabrun/AgentHub) | 嵌入式终端、文件监听实时更新、worktree 管理、diff 审查 | Swift | 495 stars，桌面端体验参考价值高 |

### 其他值得关注的项目

| 项目 | 地址 | 可参考点 | 备注 |
|------|------|----------|------|
| **Maniple** | [Martian-Engineering/maniple](https://github.com/Martian-Engineering/maniple) | 通过 MCP + tmux/iTerm2 编排 Claude/Codex 会话 | 适合参考 MCP 工具设计 |
| **AgentMux** | [markuswondrak/AgentMux](https://github.com/markuswondrak/AgentMux) | 确定性多角色流水线，通过 tmux 驱动 | 适合参考工作流编排 |
| **claude-session-driver** | [obra/claude-session-driver](https://github.com/obra/claude-session-driver) | 将其他会话作为 Worker 的驱动模式 | 适合参考 Worker 生命周期管理 |
| **gztxt/agenthub** | [gztxt/agenthub](https://github.com/gztxt/agenthub) | FastAPI + PTY + xterm.js 的本机控制台 | 适合参考 Web 终端嵌入方式 |
| **AgentCommons** | [DrishtantKaushal/AgentCommons](https://github.com/DrishtantKaushal/AgentCommons) | PTY 层拦截审批提示、跨终端任务转发 | 适合参考权限弹窗处理 |

### 使用建议

1. **优先阅读**：`awslabs/cli-agent-orchestrator` 和 `wainmann/agent-manager` 的架构与 Provider 实现。
2. **会话文件解析**：参考 `vinzify/Agent-Session-Hub`（Rust）和 `someonegg/agent_hub`。
3. **Master 调度**：参考 `orchestmux` 和 `cli-agent-orchestrator` 的 Supervisor 模式。
4. **桌面体验**：参考 `jamesrochabrun/AgentHub` 的会话卡片与嵌入终端设计。
5. **Windows 兼容**：重点关注 `reevesagents` 和 `agent-manager` 的 Windows/WSL 处理方式。

这些项目证明了「自己启动 CLI + PTY/tmux 控制」路线的可行性，SessionNexus 可在此基础上扩展 Web/Desktop 附着能力与对话窗口形态。

## 13. 可行性判断

结论：Phase 1 可以做，而且应该先做。整份方案里，真正会卡住的不是 Tauri、SQLite 或本地 API，而是「把交互式 TUI 的屏幕输出解析成可靠状态机」。

| 能力 | 判断 | 原因 |
|------|------|------|
| Tauri 2 + React 桌面壳 | 高 | 成熟，Windows WebView2 可用 |
| SQLite 注册表、axum + WebSocket | 高 | 无平台障碍；注意与 Tauri 共用 tokio |
| 读 Grok / Codex / DSH 本地会话目录 | 高 | 目录结构稳定；比刮屏稳。Qoder 目录仍需对照本机安装确认 |
| `portable-pty` + ConPTY 启动 CLI | 中高 | 能跑；要自己管进程树、尺寸、退出和孤儿进程 |
| 向交互式 TUI 注入按键并判断「说完了」 | 中低 | 备用屏幕、审批框、多行输入会让文本解析经常误判 |
| 用 CDP 附着普通已打开的 Chrome | 不做 | 没有调试端口就连不上，单独启动又丢掉现有登录态 |
| UIA 读写聊天内容 | 低 | 自绘或 Electron 界面经常没有可用文本控件 |
| Master 用自然语言做调度 | 低（作为控制面） | 能演示，不能当可靠路由 |

Phase 1 的 Provider 是 Qoder CLI、Grok Build、ChatGPT Codex CLI、DeepSeek Harness headless，以及一个浏览器 MCP 桥。Claude Code 不再是第一批。浏览器不走 CDP。

因此 Phase 1 的控制面分成三层，而不是只靠 PTY 刮屏：

1. **发现（L1）**：读 Provider 的会话文件，加上我们启动的进程表。
2. **发消息（L2）**：优先走该 Agent 的非交互入口（如 `claude -p`、`codex exec`），等待进程结束或会话文件追加。
3. **交互（L3）**：需要看原始 TUI、处理审批按键时，才附着 PTY。状态以进程退出码和会话文件为准，屏幕文本只做辅助。

Web / UIA / Master 维持原阶段，不进入 MVP。

## 14. 方案里缺的部分

### 14.1 身份与所有权

Agent 自己的会话 ID 和我们的 ID 不是同一个东西。

- `nexus_id`：我们的 UUID，API 和 UI 使用。
- `provider` + `native_session_id`：唯一键，用来对齐磁盘上的会话。
- `ownership`：`Owned`（我们启动，退出时结束整个进程树）、`Attached`（只观察，不杀用户进程）、`Indexed`（只索引历史文件）。

### 14.2 Provider Profile

每种 CLI 一份可配置档案，而不是在代码里写死斜杠命令。至少包括：

- 可执行文件名与探测命令（`claude --version`）
- 会话目录（Windows 上展开 `%USERPROFILE%`）
- 新建、恢复、非交互执行的参数模板
- 如何从会话文件判断 native id、标题、更新时间
- 审批提示的已知特征（可选，L3 用）

档案加载失败或二进制不存在时，该 Provider 显示为未安装，不阻塞其他 Provider。

### 14.3 事件，而不只是快照

`Session` 结构适合列表。实时推送用事件：

- `session.upsert`
- `session.status`
- `transcript.append`
- `approval.required`
- `process.exit`

增加状态 `WaitingApproval`。审批不是 `Error`。用户确认前，`send_message` 返回明确错误，不把下一条消息写进审批框。

### 14.4 并发与生命周期

- 一个 Owned 会话对应一个进程，不用单个 PTY 里反复 `/resume`，避免串会话。
- 同一会话的写操作串行。
- Windows 上用 Job Object 收进程树，应用退出时只结束 `Owned`。
- ConPTY 固定一个初始尺寸；Phase 1 不承诺嵌入完整终端，只保证能启动、写入和读输出。

### 14.5 本地 API

- 只绑定 `127.0.0.1`。
- 首次启动生成 Token，放在 `%APPDATA%\SessionNexus\token`。除 `GET /health` 外都要带 `Authorization: Bearer`。
- 不开放任意 CORS。
- 端口占用时换下一个端口并写进配置，UI 显示实际端口。
- 单实例：第二个进程把请求转给已有 API，而不是再开一个库。

### 14.6 存储位置

- 配置：`%APPDATA%\SessionNexus\config.json`
- 数据库：`%APPDATA%\SessionNexus\nexus.db`
- 日志：`%APPDATA%\SessionNexus\logs\`

注册表是索引。会话原文以 Agent 自己的文件为准，我们存摘要、状态、所有权和最近一次错误。

### 14.7 Phase 1 明确不做

- 浏览器只调用我们自己的扩展 MCP。不实现 CDP 或 Playwright，也不把 Kimi WebBridge 或其他厂商扩展当作依赖。
- 不做 UIA，不做 Master 调度。桥未连接时失败要可见，并留下 CLI 路径可用。
- 不嵌入 xterm。对话窗展示我们记录的消息和状态。
- 不代理模型 API Key，认证仍由各 Agent 自己完成。
- 不支持 WSL 里的 Agent。先只支持 Windows 原生可执行文件。

### 14.8 验收

不依赖本机真的安装 Claude 或 Codex 也能测核心路径：用一个固定的假 CLI（读 stdin、写几行、退出 0）跑通「创建 → 发送 → 状态变为 Finished → WebSocket 收到事件 → SQLite 留有记录」。真 Agent 作为手动验收，并在 README 里写清版本。

