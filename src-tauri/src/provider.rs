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
    crate::agents::all_profiles()
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
