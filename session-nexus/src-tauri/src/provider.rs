use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Non-interactive CLI. `{prompt}` and `{session}` are argument placeholders.
    Cli,
    /// Browser extension bridge. SessionNexus is an MCP client and does not speak CDP.
    BrowserMcp,
}

/// One agent surface. Argument vectors are templates, not scraped TUI keystrokes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: String,
    pub display_name: String,
    pub kind: ProviderKind,
    pub executable: String,
    /// Supports `%USERPROFILE%`.
    pub session_dir: String,
    /// New non-interactive turn. Empty for browser-only providers.
    pub exec_args: Vec<String>,
    /// Follow-up when `native_session_id` is known. Empty means resume is unsupported.
    pub resume_args: Vec<String>,
    pub interactive_args: Vec<String>,
    /// MCP endpoint of a browser-extension bridge. Empty for CLI providers.
    pub mcp_url: Option<String>,
    /// Tool names the bridge must expose.
    pub mcp_tools: Vec<String>,
}

pub fn phase1_profiles() -> Vec<ProviderProfile> {
    vec![
        ProviderProfile {
            id: "qoder".into(),
            display_name: "Qoder CLI".into(),
            kind: ProviderKind::Cli,
            executable: "qoder".into(),
            session_dir: r"%USERPROFILE%\.qoder".into(),
            exec_args: vec!["-p".into(), "{prompt}".into()],
            resume_args: vec!["-p".into(), "{prompt}".into(), "-r".into(), "{session}".into()],
            interactive_args: vec![],
            mcp_url: None,
            mcp_tools: vec![],
        },
        ProviderProfile {
            id: "grok".into(),
            display_name: "Grok Build".into(),
            kind: ProviderKind::Cli,
            executable: "grok".into(),
            session_dir: r"%USERPROFILE%\.grok\sessions".into(),
            exec_args: vec![
                "-p".into(),
                "{prompt}".into(),
                "--output-format".into(),
                "json".into(),
            ],
            resume_args: vec![
                "-p".into(),
                "{prompt}".into(),
                "-r".into(),
                "{session}".into(),
                "--output-format".into(),
                "json".into(),
            ],
            interactive_args: vec![],
            mcp_url: None,
            mcp_tools: vec![],
        },
        ProviderProfile {
            id: "codex".into(),
            display_name: "ChatGPT Codex CLI".into(),
            kind: ProviderKind::Cli,
            executable: "codex".into(),
            session_dir: r"%USERPROFILE%\.codex\sessions".into(),
            exec_args: vec!["exec".into(), "{prompt}".into()],
            resume_args: vec!["exec".into(), "resume".into(), "{session}".into(), "{prompt}".into()],
            interactive_args: vec![],
            mcp_url: None,
            mcp_tools: vec![],
        },
        ProviderProfile {
            id: "dsh".into(),
            display_name: "DeepSeek Harness".into(),
            kind: ProviderKind::Cli,
            executable: "dsh".into(),
            session_dir: r"%USERPROFILE%\.dsh\sessions".into(),
            exec_args: vec!["--profile".into(), "headless".into(), "{prompt}".into()],
            // Official headless starts a fresh persisted session. Resume is not offered here.
            resume_args: vec![],
            interactive_args: vec!["web".into()],
            mcp_url: None,
            mcp_tools: vec![],
        },
        ProviderProfile {
            id: "browser-mcp".into(),
            display_name: "SessionNexus browser extension".into(),
            kind: ProviderKind::BrowserMcp,
            executable: String::new(),
            session_dir: String::new(),
            exec_args: vec![],
            resume_args: vec![],
            interactive_args: vec![],
            mcp_url: Some("http://127.0.0.1:19877/mcp".into()),
            mcp_tools: vec![
                "list_tabs".into(),
                "find_tab".into(),
                "snapshot".into(),
                "fill".into(),
                "click".into(),
            ],
        },
    ]
}

pub fn expand_user_profile(path: &str) -> String {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    path.replace("%USERPROFILE%", &home)
}

/// Fill `{prompt}` and `{session}`. A missing session placeholder is left untouched
/// so a bad template fails visibly instead of dropping the id.
pub fn render_args(template: &[String], prompt: &str, session: Option<&str>) -> Vec<String> {
    template
        .iter()
        .map(|arg| {
            let mut out = arg.replace("{prompt}", prompt);
            if let Some(session) = session {
                out = out.replace("{session}", session);
            }
            out
        })
        .collect()
}

pub fn command_for(profile: &ProviderProfile, prompt: &str, native_session_id: Option<&str>) -> Result<Vec<String>, String> {
    if profile.kind == ProviderKind::BrowserMcp {
        return Err(format!(
            "{} is an MCP bridge at {}. Call its tools; do not spawn a browser.",
            profile.display_name,
            profile.mcp_url.as_deref().unwrap_or("(no endpoint)")
        ));
    }
    match native_session_id {
        Some(id) if !profile.resume_args.is_empty() => Ok(render_args(&profile.resume_args, prompt, Some(id))),
        Some(_) => Err(format!(
            "{} cannot resume a session from this connector yet",
            profile.display_name
        )),
        None => {
            if profile.exec_args.is_empty() {
                return Err(format!("{} has no non-interactive command", profile.display_name));
            }
            Ok(render_args(&profile.exec_args, prompt, None))
        }
    }
}
