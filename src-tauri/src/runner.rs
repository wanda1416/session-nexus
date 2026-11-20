use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::agents::{find_agent, AgentAdapter};
use crate::provider::{phase1_profiles, ProviderKind};
use crate::store::ContactProfile;

const MAX_CHARS: usize = 8000;

pub fn resolve_executable(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for ext in ["exe", "cmd"] {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn installed_providers() -> Vec<ProviderStatus> {
    phase1_profiles()
        .into_iter()
        .filter(|profile| profile.kind == ProviderKind::Cli)
        .map(|profile| ProviderStatus {
            id: profile.id.clone(),
            name: profile.display_name.clone(),
            installed: resolve_executable(&profile.executable).is_some(),
        })
        .collect()
}

#[derive(Debug, serde::Serialize)]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    pub installed: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct AgentRun {
    pub provider: String,
    pub ok: bool,
    pub text: String,
    pub native_session_id: Option<String>,
    pub dispatch_task: Option<String>,
}

pub fn run_agent(
    provider: &str,
    prompt: &str,
    cwd: Option<&str>,
    native_session_id: Option<&str>,
    role: &str,
) -> AgentRun {
    run_agent_with_profile(provider, prompt, cwd, native_session_id, role, None)
}

pub fn run_agent_with_profile(
    provider: &str,
    prompt: &str,
    cwd: Option<&str>,
    native_session_id: Option<&str>,
    role: &str,
    contact_profile: Option<&ContactProfile>,
) -> AgentRun {
    let Some(agent) = find_agent(provider) else {
        return failed(provider, format!("未知的联系人 {provider}"));
    };
    let profile = agent.profile();
    let Some(program) = resolve_executable(&profile.executable) else {
        return failed(
            provider,
            format!("未找到 {}（{}）", profile.display_name, profile.executable),
        );
    };
    let effective_prompt = apply_persona(prompt, contact_profile, native_session_id, role);
    let (mut args, assigned_id) = match agent.build_args(&effective_prompt, cwd, native_session_id, role) {
        Ok(value) => value,
        Err(err) => return failed(provider, err),
    };
    if let Some(model) = contact_profile.map(|p| p.model.trim()).filter(|m| !m.is_empty()) {
        agent.apply_model(&mut args, model);
    }
    let envs = build_envs(agent.as_ref(), contact_profile);
    let workdir = cwd.filter(|dir| Path::new(dir).is_dir());

    match spawn(&program, &args, workdir, &envs) {
        Ok((code, raw)) => {
            let parsed_id = assigned_id.or_else(|| agent.parse_session_id(&raw));
            let formatted = agent.format_output(&raw);
            let (visible, dispatch_task) = if role == "room" {
                split_dispatch(&formatted)
            } else {
                (formatted, None)
            };
            AgentRun {
                provider: provider.to_string(),
                ok: code == 0,
                text: if visible.trim().is_empty() {
                    format!("进程退出，代码 {code}，没有输出")
                } else {
                    visible
                },
                native_session_id: parsed_id,
                dispatch_task: if role == "room" { dispatch_task } else { None },
            }
        }
        Err(err) => failed(provider, err),
    }
}

fn failed(provider: &str, text: String) -> AgentRun {
    AgentRun {
        provider: provider.to_string(),
        ok: false,
        text,
        native_session_id: None,
        dispatch_task: None,
    }
}

pub(crate) fn new_session_id() -> String {
    let ticks = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);
    let pid = std::process::id() as u128;
    let a = splitmix64((ticks as u64) ^ ((pid as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)));
    let b = splitmix64(((ticks >> 64) as u64).wrapping_add(a).wrapping_add(0xbf58_476d_1ce4_e5b9));
    let mixed = ((a as u128) << 64) | (b as u128);
    format!(
        "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
        (mixed >> 96) as u32,
        (mixed >> 80) as u16,
        (mixed >> 68) as u16 & 0x0fff,
        (mixed >> 56) as u16 & 0x0fff,
        mixed & 0xffffffffffff
    )
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

pub fn split_dispatch(text: &str) -> (String, Option<String>) {
    let Some(start) = text.find("```dispatch") else {
        return (text.trim().to_string(), None);
    };
    let after = &text[start + "```dispatch".len()..];
    let Some(end) = after.find("```") else {
        return (text.trim().to_string(), None);
    };
    let body = after[..end].trim();
    let task = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("task").and_then(|item| item.as_str()).map(str::to_string))
        .filter(|task| !task.trim().is_empty());
    let mut visible = String::new();
    visible.push_str(text[..start].trim());
    let rest = after[end + 3..].trim();
    if !rest.is_empty() {
        if !visible.is_empty() {
            visible.push_str("\n");
        }
        visible.push_str(rest);
    }
    (visible, task)
}

pub(crate) fn clip(text: &str) -> String {
    if text.chars().count() > MAX_CHARS {
        let mut cut: String = text.chars().take(MAX_CHARS).collect();
        cut.push_str("\n…");
        cut
    } else {
        text.to_string()
    }
}

const ROOM_MASTER_PROMPT: &str = "\
[主对话身份与协作协议]
你现在是 SessionNexus 群聊中某个 Agent 的主对话（Room Session），负责与用户及群内其他成员沟通、拆解任务并引导推进。
1. 引导用户：当用户询问如何指挥你时，清晰说明你负责主对话沟通与方案拆解；需要真正动手执行时，你会通过创建独立的工作会话（Work Session）来完成，直到整个任务闭环。
2. 触发工作会话：不要在主对话里自己派生 subagent。如果只是讨论、规划或引导，直接用自然语言回复；只有需要真正动手执行独立任务（如写代码、读写文件、运行命令等）时，才在回复末尾单独附带一个 dispatch 代码块：
```dispatch
{\"task\":\"具体可执行的工作任务描述\"}
```
3. 多 Agent 协作：群内可能有多个 Agent（如 @Grok、@Qoder），请专注完成分配给你的部分，并给出清晰可交接的结论。";

pub(crate) fn apply_persona(
    prompt: &str,
    contact_profile: Option<&ContactProfile>,
    native_session_id: Option<&str>,
    role: &str,
) -> String {
    let is_fresh_room = role == "room" && native_session_id.map_or(true, |s| s.trim().is_empty());
    if !is_fresh_room {
        return prompt.to_string();
    }
    let mut sections = vec![ROOM_MASTER_PROMPT.to_string()];
    if let Some(persona) = contact_profile
        .map(|p| p.persona.trim())
        .filter(|p| !p.is_empty())
    {
        sections.push(format!("[人设与额外指令]\n{persona}"));
    }
    sections.push(prompt.to_string());
    sections.join("\n\n")
}

fn build_envs(agent: &dyn AgentAdapter, contact_profile: Option<&ContactProfile>) -> Vec<(String, String)> {
    let mut envs = Vec::new();
    let Some(profile) = contact_profile else {
        return envs;
    };
    if let Some(key_name) = agent.default_api_key_env() {
        let key = profile.api_key.trim();
        if !key.is_empty() {
            envs.push((key_name.to_string(), key.to_string()));
        }
    }
    for line in profile.env_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim();
            if !key.is_empty() {
                envs.push((key.to_string(), v.trim().to_string()));
            }
        }
    }
    envs
}

fn spawn(
    program: &Path,
    args: &[String],
    cwd: Option<&str>,
    envs: &[(String, String)],
) -> Result<(i32, String), String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .envs(envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let mut child = command.spawn().map_err(|err| format!("启动失败：{err}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut out = String::new();
        if let Some(mut pipe) = stdout {
            let _ = pipe.read_to_string(&mut out);
        }
        out
    });
    let err_reader = std::thread::spawn(move || {
        let mut err = String::new();
        if let Some(mut pipe) = stderr {
            let _ = pipe.read_to_string(&mut err);
        }
        err
    });
    let finished = wait_timeout(&mut child, Duration::from_secs(180));
    let code = match finished {
        Ok(code) => code,
        Err(message) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(message);
        }
    };
    let out = out_reader.join().unwrap_or_default();
    let err = err_reader.join().unwrap_or_default();
    let mut text = if out.trim().is_empty() { err } else { out };
    if text.chars().count() > MAX_CHARS {
        text = text.chars().take(MAX_CHARS).collect();
        text.push_str("\n…");
    }
    Ok((code.unwrap_or(-1), text))
}

#[cfg(test)]
mod tests {
    use super::split_dispatch;
    use crate::agents::{codex::CodexAgent, AgentAdapter};

    #[test]
    fn dispatch_block_becomes_a_task() {
        let (visible, task) = split_dispatch("收到。\n```dispatch\n{\"task\":\"初始化仓库\"}\n```\n");
        assert_eq!(visible, "收到。");
        assert_eq!(task.as_deref(), Some("初始化仓库"));
    }

    #[test]
    fn codex_resume_omits_cd_flag_while_exec_keeps_it() {
        let cwd = env!("CARGO_MANIFEST_DIR");
        let agent = CodexAgent;
        let mut exec_args = vec!["exec".into(), "--json".into(), "hi".into()];
        agent.apply_cwd(&mut exec_args, cwd);
        assert_eq!(exec_args, vec!["exec", "--json", "-C", cwd, "hi"]);

        let mut resume_args = vec!["exec".into(), "resume".into(), "tid-1".into(), "hi".into()];
        agent.apply_cwd(&mut resume_args, cwd);
        assert_eq!(resume_args, vec!["exec", "resume", "tid-1", "hi"]);
    }

    #[test]
    fn codex_json_lines_extract_session_and_text() {
        let agent = CodexAgent;
        let raw = concat!(
            "{\"type\":\"thread.started\",\"thread_id\":\"019a-test\"}\n",
            "{\"type\":\"item.completed\",\"item\":{\"id\":\"1\",\"type\":\"agent_message\",\"text\":\"pong\\nline2\"}}\n"
        );
        assert_eq!(agent.parse_session_id(raw).as_deref(), Some("019a-test"));
        assert_eq!(agent.format_output(raw), "pong\nline2");
    }

    #[test]
    #[ignore = "calls live local CLIs"]
    fn live_cli_room_resume_and_dispatch() {
        let cwd = env!("CARGO_MANIFEST_DIR");
        for provider in ["codex", "qoder", "grok", "dsh"] {
            let first = super::run_agent(
                provider,
                "请记住测试代号：ALPHA-77。只回复：已记住 ALPHA-77",
                Some(cwd),
                None,
                "room",
            );
            println!("[{provider}][1] ok={} sid={:?} text={:?}", first.ok, first.native_session_id, first.text);
            if !first.ok {
                continue;
            }

            if provider != "dsh" {
                let sid = first.native_session_id.as_deref().expect("expected session id");
                let second = super::run_agent(
                    provider,
                    "我刚才让你记住的测试代号是什么？只回复代号。",
                    Some(cwd),
                    Some(sid),
                    "room",
                );
                println!("[{provider}][2] ok={} sid={:?} text={:?}", second.ok, second.native_session_id, second.text);
                assert!(second.ok, "{provider} resume turn failed: {}", second.text);
                assert!(
                    second.text.contains("ALPHA-77"),
                    "{provider} resume did not remember ALPHA-77: {}",
                    second.text
                );
            }
        }

        let dispatch_turn = super::run_agent(
            "qoder",
            "不要解释，只输出一行文字“收到。”，紧接着换行输出一个 ```dispatch 代码块，内容为 {\"task\":\"只回复 work-ok\"}",
            Some(cwd),
            None,
            "room",
        );
        println!(
            "[qoder][dispatch] ok={} visible={:?} task={:?}",
            dispatch_turn.ok, dispatch_turn.text, dispatch_turn.dispatch_task
        );
        assert!(dispatch_turn.ok, "dispatch turn failed: {}", dispatch_turn.text);
        let task = dispatch_turn.dispatch_task.expect("expected dispatch_task");
        let work = super::run_agent("qoder", &task, Some(cwd), None, "work");
        println!("[qoder][work] ok={} text={:?}", work.ok, work.text);
        assert!(work.ok, "work turn failed: {}", work.text);
    }

    #[test]
    #[ignore = "calls live grok and qoder CLIs"]
    fn live_grok_qoder_group_collab() {
        let dir = std::env::temp_dir().join(format!("nexus-collab-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.to_string_lossy().into_owned();

        // 1. Grok 主对话：引导用户如何指挥它，并设计协作任务方案（不要自己 dispatch）
        let grok_turn1 = super::run_agent(
            "grok",
            "我们正在一个包含 @Grok 和 @Qoder 的群聊里协作。\
            请简短回答两件事（不要输出 dispatch 代码块）：\
            1) 告诉用户你作为主对话是如何配合工作会话引导用户完成任务的；\
            2) 设计一个由 Qoder 落盘的任务：在当前工作目录创建文件 collab.txt，内容必须包含暗号 NEXUS-COLLAB-2026。",
            Some(&cwd),
            None,
            "room",
        );
        println!("[collab][grok-1] ok={} sid={:?}\n{}", grok_turn1.ok, grok_turn1.native_session_id, grok_turn1.text);
        assert!(grok_turn1.ok, "grok turn 1 failed: {}", grok_turn1.text);
        let grok_sid = grok_turn1.native_session_id.clone().expect("grok session id");

        // 2. Qoder 主对话：接收 Grok 的方案，通过 dispatch 派发独立工作会话
        let qoder_prompt = format!(
            "群内 @Grok 给出了如下方案：\n{}\n\n\
            现在请你作为 Qoder 主对话，不要自己改文件，直接输出简短确认并在末尾附带 ```dispatch 代码块，\
            把「在当前工作目录创建 collab.txt 且内容包含 NEXUS-COLLAB-2026」作为 task 派发给工作会话。",
            grok_turn1.text
        );
        let qoder_turn1 = super::run_agent("qoder", &qoder_prompt, Some(&cwd), None, "room");
        println!(
            "[collab][qoder-1] ok={} sid={:?} dispatch={:?}\n{}",
            qoder_turn1.ok, qoder_turn1.native_session_id, qoder_turn1.dispatch_task, qoder_turn1.text
        );
        assert!(qoder_turn1.ok, "qoder turn 1 failed: {}", qoder_turn1.text);
        let task = qoder_turn1.dispatch_task.expect("qoder should emit dispatch_task");

        // 3. 执行 Qoder 工作会话
        let work_prompt = format!(
            "工作目录：{cwd}\n任务：{task}\n请直接在工作目录完成文件写入，只用简短文字回复结果，不要派生子代理。"
        );
        let qoder_work = super::run_agent("qoder", &work_prompt, Some(&cwd), None, "work");
        println!("[collab][qoder-work] ok={}\n{}", qoder_work.ok, qoder_work.text);
        assert!(qoder_work.ok, "qoder work failed: {}", qoder_work.text);

        let target = dir.join("collab.txt");
        let content = std::fs::read_to_string(&target).expect("collab.txt should be created by qoder work");
        println!("[collab][file] {}", content);
        assert!(content.contains("NEXUS-COLLAB-2026"), "unexpected file content: {content}");

        // 4. 恢复 Grok 房间会话，验证上下文保持
        let grok_turn2 = super::run_agent(
            "grok",
            "Qoder 已经按你的方案创建好 collab.txt 了。请确认你刚才在方案里定的暗号是什么？只回复暗号。",
            Some(&cwd),
            Some(&grok_sid),
            "room",
        );
        println!("[collab][grok-2] ok={}\n{}", grok_turn2.ok, grok_turn2.text);
        assert!(grok_turn2.ok, "grok turn 2 failed: {}", grok_turn2.text);
        assert!(grok_turn2.text.contains("NEXUS-COLLAB-2026"), "grok forgot token: {}", grok_turn2.text);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn wait_timeout(child: &mut std::process::Child, limit: Duration) -> Result<Option<i32>, String> {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.code()),
            Ok(None) if start.elapsed() > limit => return Err("180 秒内没有返回".into()),
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(err) => return Err(format!("等待进程失败：{err}")),
        }
    }
}
