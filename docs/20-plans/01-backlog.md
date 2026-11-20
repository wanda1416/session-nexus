# 开发待办

> 状态：部分实现（2026-11-21，基线 v0.1.0）。状态只取已实现、部分实现、规划。

验收以桌面窗口 `pnpm tauri dev` 为准。只开 `pnpm dev` 连不上本机 CLI 和数据库。

## 已实现

| 项 | 验收 |
|----|------|
| 仓库根目录就是应用根目录 | `package.json` 与 `src-tauri/` 在 Git 根上 |
| 三栏群聊界面 | 联系人、群、消息、工作列表能在窗口里看到 |
| 四个 CLI | Grok、`qodercli`、Codex、`dsh headless` 可由发送动作启动。Grok 用「只回复 pong」验证过 |
| SQLite | `%APPDATA%\SessionNexus\nexus.db` 保存群、成员、消息、工作和房间会话 id |
| 房间会话与工作会话分开 | 消息先进入房间会话。回复里的 `dispatch` 代码块才会新开工作会话。Grok 与 Qoder 用我们指定的 id 恢复。Codex 在输出含会话 id 时恢复。DSH headless 不能恢复 |
| 浏览器 MCP 进程 | 应用启动后监听 `127.0.0.1:19877`。扩展未连接时工具返回「扩展未连接」 |
| 英文和中文 README | 英文版是主文档 |

## 部分实现

| 项 | 还差什么 |
|----|----------|
| 房间会话往返 | 解析、数据库与 Qoder 真实端到端（建房→恢复→`dispatch` 独立开工）已通过。Grok（本机余额耗尽 402）、Codex（本机 `localhost:7863` 代理 404）、DSH（缺 `DEEPSEEK_API_KEY`）均已验证错误透传与会话 id 提取，待本机凭据/余额就绪后补全成功态回归 |
| 浏览器扩展 | 源码在 `bridge/extension`。还没有装进已登录的 Chrome 或 Edge 做点击和填写 |
| 会话目录发现 | Rust 能索引 Qoder、Grok、Codex、DSH 的会话目录。界面还没有列出这些已有会话 |

## 规划

| 项 | 边界 |
|----|------|
| 本地 API `127.0.0.1:19876` 和手机客户端 | 复用同一套群和会话，不另做模型 |
| 嵌入完整终端 | 对话窗继续展示我们记录的消息 |
| UI Automation | 不作为第一条桌面接入路径 |
| WSL 里的 Agent | 先只支持 Windows 原生可执行文件 |
| 全局 Master Session | 不做。话题隔离在群上 |

下一步：进入 P1（浏览器扩展真机联调 & 界面展示已发现会话目录）；待 Grok/Codex/DSH 本机凭据就绪后补跑一次 `live_cli_room_resume_and_dispatch`。
