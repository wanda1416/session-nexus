use crate::agents::AgentAdapter;
use crate::provider::{command_for, ProviderKind, ProviderProfile};
use crate::runner::new_session_id;

pub struct QoderAgent;

impl AgentAdapter for QoderAgent {
    fn id(&self) -> &'static str {
        "qoder"
    }

    fn profile(&self) -> ProviderProfile {
        ProviderProfile {
            id: "qoder".into(),
            display_name: "Qoder CLI".into(),
            kind: ProviderKind::Cli,
            executable: "qodercli".into(),
            session_dir: r"%USERPROFILE%\.qoder\projects".into(),
            exec_args: vec![
                "-p".into(),
                "--permission-mode".into(),
                "dont_ask".into(),
                "{prompt}".into(),
            ],
            resume_args: vec![
                "-p".into(),
                "--permission-mode".into(),
                "dont_ask".into(),
                "-r".into(),
                "{session}".into(),
                "{prompt}".into(),
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
                    "--permission-mode".into(),
                    "bypass_permissions".into(),
                    prompt.into(),
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
        args.extend(["--session-id".into(), id.clone()]);
        Ok((args, Some(id)))
    }

    fn apply_cwd(&self, args: &mut Vec<String>, dir: &str) {
        args.extend(["-w".into(), dir.to_string()]);
    }

    fn apply_model(&self, args: &mut Vec<String>, model: &str) {
        args.extend(["-m".into(), model.to_string()]);
    }
}
