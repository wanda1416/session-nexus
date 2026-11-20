use crate::agents::AgentAdapter;
use crate::provider::{ProviderKind, ProviderProfile};

pub struct DshAgent;

impl AgentAdapter for DshAgent {
    fn id(&self) -> &'static str {
        "dsh"
    }

    fn profile(&self) -> ProviderProfile {
        ProviderProfile {
            id: "dsh".into(),
            display_name: "DeepSeek Harness".into(),
            kind: ProviderKind::Cli,
            executable: "dsh".into(),
            session_dir: r"%USERPROFILE%\.dsh\sessions".into(),
            exec_args: vec!["headless".into(), "{prompt}".into()],
            // Official headless starts a fresh persisted session. Resume is not offered here.
            resume_args: vec![],
            interactive_args: vec!["web".into()],
            mcp_url: None,
            mcp_tools: vec![],
        }
    }

    fn default_api_key_env(&self) -> Option<&'static str> {
        Some("DEEPSEEK_API_KEY")
    }
}
