use crate::agents::AgentAdapter;
use crate::provider::{command_for, ProviderKind, ProviderProfile};
use crate::runner::new_session_id;

pub struct GrokAgent;

impl AgentAdapter for GrokAgent {
    fn id(&self) -> &'static str {
        "grok"
    }

    fn profile(&self) -> ProviderProfile {
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
                "plain".into(),
                "--no-subagents".into(),
                "--permission-mode".into(),
                "dontAsk".into(),
            ],
            resume_args: vec![
                "-p".into(),
                "{prompt}".into(),
                "-r".into(),
                "{session}".into(),
                "--output-format".into(),
                "plain".into(),
                "--no-subagents".into(),
                "--permission-mode".into(),
                "dontAsk".into(),
            ],
            interactive_args: vec![],
            mcp_url: None,
            mcp_tools: vec![],
        }
    }

    fn build_args(
        &self,
        prompt: &str,
        cwd: Option<&str>,
        native_session_id: Option<&str>,
        role: &str,
    ) -> Result<(Vec<String>, Option<String>), String> {
        let profile = self.profile();
        let (mut args, assigned_id) = if role == "room" {
            match native_session_id.filter(|id| !id.is_empty()) {
                Some(id) => (command_for(&profile, prompt, Some(id))?, Some(id.to_string())),
                None => self.new_room_args(&profile, prompt)?,
            }
        } else {
            // Work sessions are dispatched to execute file edits/commands non-interactively.
            (
                vec![
                    "-p".into(),
                    prompt.into(),
                    "--output-format".into(),
                    "plain".into(),
                    "--no-subagents".into(),
                    "--permission-mode".into(),
                    "bypassPermissions".into(),
                ],
                None,
            )
        };
        if let Some(dir) = cwd.filter(|dir| std::path::Path::new(dir).is_dir()) {
            self.apply_cwd(&mut args, dir);
        }
        Ok((args, assigned_id))
    }

    fn new_room_args(
        &self,
        profile: &ProviderProfile,
        prompt: &str,
    ) -> Result<(Vec<String>, Option<String>), String> {
        let mut args = command_for(profile, prompt, None)?;
        let id = new_session_id();
        args.extend(["-s".into(), id.clone()]);
        Ok((args, Some(id)))
    }

    fn apply_cwd(&self, args: &mut Vec<String>, dir: &str) {
        args.extend(["--cwd".into(), dir.to_string()]);
    }

    fn apply_model(&self, args: &mut Vec<String>, model: &str) {
        args.extend(["-m".into(), model.to_string()]);
    }

    fn default_api_key_env(&self) -> Option<&'static str> {
        Some("XAI_API_KEY")
    }
}
