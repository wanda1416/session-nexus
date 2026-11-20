# P0 计划：基线收敛与四 CLI 端到端实测

> 状态：已实现（2026-11-21，基线 v0.1.0）。本文是 [开发待办](01-backlog.md) 的 P0 执行方案，聚焦基线提交与桌面端四 CLI 的真实往返验收。

## 1. 目标与边界

本阶段目标是将当前工作区收敛为可稳定回归的干净基线，并在桌面应用（`dev.bat` / `pnpm tauri dev`）中完成四个一期 CLI Agent 的真实房间会话与工作会话链路验收。

**包含范围**：
- 工作区改动审查、前后端构建与单测通过、提交干净基线。
- 四个一期 CLI（Grok、`qodercli`、Codex、`dsh headless`）的本机可用性核对与会话目录确认。
- 桌面端真实往返测试：新建房间会话 → 恢复同一房间会话 → 触发 `dispatch` 代码块独立开启工作会话 → SQLite 持久化与重启恢复校验。
- 实测中暴露的参数拼装、输出解析或进程通信缺陷修复。

**不包含范围（留待 P1/P2）**：
- Chrome/Edge 浏览器扩展真机联调（`bridge/extension`）。
- 前端展示已有本地会话目录列表（`discover.rs` UI 暴露）。
- `Connector` trait 架构重构与本地 API `127.0.0.1:19876`。

---

## 2. 阶段拆解与执行步骤

### 阶段 A：基线审查与提交（预计 0.5h）

1. **静态检查与单元测试**
   - 运行 `dev.bat --check`（覆盖 `tsc`、`vite build` 与 `cargo check`）。
   - 运行 `dev.bat --test`（覆盖 Rust 单元测试）。
2. **工作区清理与核对**
   - 核对未提交改动（文档迁移、`dev.bat`/`dev.sh`、`runner.rs`、`store.rs`、`App.tsx`、`chat.ts`），确认无临时文件或冗余旧文件残留。
   - 检查 `session-nexus/` 残留子目录（如有）是否仍被引用或应清理。
3. **提交基线**
   - 按「文档与开发脚本」与「会话存储与运行器改动」拆分或整理为清晰提交，作为 P0 实测回归基准。

### 阶段 B：本机 CLI 环境与会话目录核对（预计 0.5h）

逐一确认本机 PATH 中四个 CLI 的可执行文件与实际参数兼容性，重点核对当前代码假设与真实版本的差异：

| Provider | 可执行文件 | 核对重点 |
|----------|------------|----------|
| Grok Build | `grok` | 确认 `-p`、`-s <uuid>`、`-r <uuid>`、`--cwd`、`--output-format plain`、`--no-subagents`、`--permission-mode dontAsk` 在当前版本均有效 |
| Qoder CLI | `qodercli` | 确认 `-p`、`--session-id <uuid>`、`-r <uuid>`、`-w <dir>`、`--permission-mode dont_ask` 有效；核对本机真实会话目录并修正 `provider.rs` 中的 `%USERPROFILE%\.qoder` |
| ChatGPT Codex CLI | `codex` | 确认 `exec --json` 输出中的会话 ID 字段名与格式；验证 `codex exec resume <session> -C <dir> <prompt>` 的参数顺序是否被 CLI 接受 |
| DeepSeek Harness | `dsh` | 确认 `dsh headless <prompt>` 能正常退出并返回文本；确认其不支持 resume 时的降级行为符合预期 |

### 阶段 C：桌面端端到端实测（核心验收，预计 1.5h）

在仓库根目录执行 `dev.bat` 启动桌面窗口，按以下顺序逐个验证（优先 Grok，再覆盖其余三个 CLI）：

#### 用例 1：首轮房间会话建立
- **操作**：新建或进入测试群，指定有效工作目录，向目标 Agent 发送带记忆锚点的消息（例如要求记住特定代号并简短回复）。
- **预期**：
  - 界面正常展示回复，无报错或超时。
  - SQLite（`%APPDATA%\SessionNexus\nexus.db`）中写入该群对应成员的 `native_session_id`（DSH 除外）。

#### 用例 2：房间会话上下文恢复
- **操作**：在同一群内发送追问（要求复述上一轮的记忆代号）。
- **预期**：
  - Grok、Qoder、Codex 均通过各自 resume 参数复用同一 `native_session_id`，能准确复述上一轮代号。
  - 关闭桌面窗口并重新运行 `dev.bat` 后，历史消息与 `native_session_id` 完整保留，再次追问仍可续接。
  - DSH 按预期以无状态新会话响应，不崩溃、不报错。

#### 用例 3：`dispatch` 触发独立工作会话
- **操作**：在房间对话中引导 Agent 输出标准 `dispatch` 代码块（包含 `{"task": "..."}` JSON）。
- **预期**：
  - 房间消息区自动剥离 `dispatch` 代码块，仅保留可见回复正文。
  - 右侧工作列表自动创建一条新工作记录，并以非 resume 的独立工作命令（`role = "work"`）启动子进程执行任务。
  - 工作完成后状态与输出正确写回 SQLite，重启应用后仍可见。
  - 房间会话本身的 `native_session_id` 不被工作会话覆盖。

### 阶段 D：实测问题修复与回归（预计 1h）

针对阶段 B/C 中可能暴露的高风险点预置排查清单，发现即修并补齐单元测试：

1. **Codex 参数与 JSON 解析**：
   - 若 `codex exec resume` 不接受 `-C` 在 `<session>` 之后，需调整 `with_cwd` 的插参位置。
   - 若 Codex JSON 输出存在空格或转义引号，将 `find_session_id` / `plain_text` 从手写字符串切片升级为 `serde_json` 逐行解析。
2. **子进程管道死锁风险**：
   - `runner.rs` 当前在单个后台线程中顺序读取 `stdout` 和 `stderr`，大输出或错误流阻塞时可能触发超时；如实测复现，改为双线程并发读取。
3. **Qoder 会话目录**：
   - 根据本机实测路径更新 `provider.rs` 及 `discover.rs` 的扫描深度（如有差异）。
4. **文档回写**：
   - 验收全部通过后，更新 [01-backlog.md](01-backlog.md) 对应条目状态，并在 [开发进度归档](../80-history/02-dev/README.md) 记录当日验收结果。

---

## 3. 验收标准（Definition of Done）

- [ ] `dev.bat --check` 与 `dev.bat --test` 全部通过，工作区提交干净。
- [ ] Grok 在桌面窗口中跑通「首轮建房 → 追问恢复 → 重启恢复 → `dispatch` 独立开工」全链路。
- [ ] Qoder、Codex、DSH 至少完成首轮对话与各自能力边界内的恢复/开工验证（不支持项有明确降级表现）。
- [ ] Qoder 本机真实会话目录已核对并同步至代码与文档。
- [ ] 实测中修复的逻辑均有对应 Rust 单元测试覆盖，且 [01-backlog.md](01-backlog.md) 已同步更新。
