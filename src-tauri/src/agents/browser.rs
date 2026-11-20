use crate::agents::AgentAdapter;
use crate::provider::{ProviderKind, ProviderProfile};

pub struct BrowserMcpAgent;

impl AgentAdapter for BrowserMcpAgent {
    fn id(&self) -> &'static str {
        "browser-mcp"
    }

    fn profile(&self) -> ProviderProfile {
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
        }
    }
}
