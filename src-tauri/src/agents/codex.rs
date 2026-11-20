use crate::agents::AgentAdapter;
use crate::provider::{ProviderKind, ProviderProfile};
use crate::runner::clip;

pub struct CodexAgent;

impl AgentAdapter for CodexAgent {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn profile(&self) -> ProviderProfile {
        ProviderProfile {
            id: "codex".into(),
            display_name: "ChatGPT Codex CLI".into(),
            kind: ProviderKind::Cli,
            executable: "codex".into(),
            session_dir: r"%USERPROFILE%\.codex\sessions".into(),
            exec_args: vec![
                "exec".into(),
                "--skip-git-repo-check".into(),
                "--ephemeral".into(),
                "--color".into(),
                "never".into(),
                "{prompt}".into(),
            ],
            resume_args: vec![
                "exec".into(),
                "resume".into(),
                "--skip-git-repo-check".into(),
                "--json".into(),
                "{session}".into(),
                "{prompt}".into(),
            ],
            interactive_args: vec![],
            mcp_url: None,
            mcp_tools: vec![],
        }
    }

    fn new_room_args(
        &self,
        _profile: &ProviderProfile,
        prompt: &str,
    ) -> Result<(Vec<String>, Option<String>), String> {
        // Codex rooms omit `--ephemeral` and emit JSONL so `thread_id` can be captured and resumed.
        Ok((
            vec![
                "exec".into(),
                "--skip-git-repo-check".into(),
                "--color".into(),
                "never".into(),
                "--json".into(),
                prompt.into(),
            ],
            None,
        ))
    }

    fn apply_cwd(&self, args: &mut Vec<String>, dir: &str) {
        // `codex exec resume` does not accept `-C`; it relies on the child process current_dir.
        if args.get(1).map(String::as_str) != Some("resume") {
            let prompt = args.pop();
            args.extend(["-C".into(), dir.to_string()]);
            if let Some(prompt) = prompt {
                args.push(prompt);
            }
        }
    }

    fn apply_model(&self, args: &mut Vec<String>, model: &str) {
        let prompt = args.pop();
        args.extend(["-m".into(), model.to_string()]);
        if let Some(prompt) = prompt {
            args.push(prompt);
        }
    }

    fn default_api_key_env(&self) -> Option<&'static str> {
        Some("OPENAI_API_KEY")
    }

    fn parse_session_id(&self, raw: &str) -> Option<String> {
        for line in raw.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with('{') {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                for key in ["thread_id", "session_id", "sessionId"] {
                    if let Some(id) = value.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        return Some(id.to_string());
                    }
                }
            }
        }
        None
    }

    fn format_output(&self, raw: &str) -> String {
        let mut parts = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with('{') {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                continue;
            };
            let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if kind == "item.completed" {
                if let Some(item) = value.get("item") {
                    if item.get("type").and_then(|v| v.as_str()) == Some("agent_message") {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            parts.push(text.to_string());
                            continue;
                        }
                    }
                }
            }
            if kind == "message"
                || value.get("item_type").and_then(|v| v.as_str()) == Some("agent_message")
            {
                if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                    parts.push(text.to_string());
                }
            }
        }
        if parts.is_empty() {
            clip(raw)
        } else {
            clip(&parts.join("\n"))
        }
    }
}
