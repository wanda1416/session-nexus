use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::provider::{command_for, phase1_profiles, ProviderKind};

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
}

pub fn run_agent(provider: &str, prompt: &str, cwd: Option<&str>) -> AgentRun {
    let Some(profile) = phase1_profiles().into_iter().find(|item| item.id == provider) else {
        return AgentRun {
            provider: provider.to_string(),
            ok: false,
            text: format!("未知的联系人 {provider}"),
        };
    };
    let Some(program) = resolve_executable(&profile.executable) else {
        return AgentRun {
            provider: provider.to_string(),
            ok: false,
            text: format!("未找到 {}（{}）", profile.display_name, profile.executable),
        };
    };
    let args = match command_for(&profile, prompt, None) {
        Ok(args) => args,
        Err(err) => {
            return AgentRun {
                provider: provider.to_string(),
                ok: false,
                text: err,
            };
        }
    };
    let args = with_cwd(&profile.id, args, cwd);
    let workdir = cwd.filter(|dir| Path::new(dir).is_dir());

    match spawn(&program, &args, workdir) {
        Ok((code, text)) => AgentRun {
            provider: provider.to_string(),
            ok: code == 0,
            text: if text.trim().is_empty() {
                format!("进程退出，代码 {code}，没有输出")
            } else {
                text
            },
        },
        Err(err) => AgentRun {
            provider: provider.to_string(),
            ok: false,
            text: err,
        },
    }
}

fn with_cwd(id: &str, mut args: Vec<String>, cwd: Option<&str>) -> Vec<String> {
    let Some(dir) = cwd.filter(|dir| Path::new(dir).is_dir()) else {
        return args;
    };
    match id {
        "grok" => args.extend(["--cwd".into(), dir.to_string()]),
        "qoder" => args.extend(["-w".into(), dir.to_string()]),
        "codex" => {
            let prompt = args.pop();
            args.extend(["-C".into(), dir.to_string()]);
            if let Some(prompt) = prompt {
                args.push(prompt);
            }
        }
        _ => {}
    }
    args
}

fn spawn(program: &Path, args: &[String], cwd: Option<&str>) -> Result<(i32, String), String> {
    let mut command = Command::new(program);
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let mut child = command.spawn().map_err(|err| format!("启动失败：{err}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let mut err = String::new();
        if let Some(mut pipe) = stdout {
            let _ = pipe.read_to_string(&mut out);
        }
        if let Some(mut pipe) = stderr {
            let _ = pipe.read_to_string(&mut err);
        }
        (out, err)
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
    let (out, err) = reader.join().unwrap_or_default();
    let mut text = if out.trim().is_empty() { err } else { out };
    if text.chars().count() > MAX_CHARS {
        text = text.chars().take(MAX_CHARS).collect();
        text.push_str("\n…");
    }
    Ok((code.unwrap_or(-1), text))
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
