use crate::agents::web::WebSiteConfig;
use crate::agents::AgentAdapter;
use crate::provider::{ProviderKind, ProviderProfile};

pub struct DshWebAgent;

impl AgentAdapter for DshWebAgent {
    fn id(&self) -> &'static str {
        "dsh-web"
    }

    fn profile(&self) -> ProviderProfile {
        ProviderProfile {
            id: "dsh-web".into(),
            display_name: "DeepSeek Harness Web".into(),
            kind: ProviderKind::BrowserMcp,
            executable: String::new(),
            session_dir: String::new(),
            exec_args: vec![],
            resume_args: vec![],
            interactive_args: vec![],
            mcp_url: Some("http://127.0.0.1:19877/mcp".into()),
            mcp_tools: vec![
                "find_tab".into(),
                "open_tab".into(),
                "fill".into(),
                "click".into(),
                "press_key".into(),
                "read_text".into(),
                "exists".into(),
            ],
        }
    }

    fn web_site(&self) -> Option<WebSiteConfig> {
        Some(WebSiteConfig {
            id: "dsh-web",
            url_matches: &["http://127.0.0.1:3080", "http://localhost:3080", "http://127.0.0.1:", "http://localhost:"],
            open_url: "http://127.0.0.1:3080/",
            new_chat_button_selectors: &[
                "button[aria-label*='未分组']",
                "button[title*='未分组']",
                "button[aria-label*='新建会话']",
            ],
            input_selectors: &[
                "div[contenteditable='true']",
                "textarea",
                "input[type='text']",
            ],
            submit_selectors: &[
                "button[aria-label*='发送']",
                "button[aria-label*='Send']",
                "button[type='submit']",
            ],
            stop_selectors: &[
                "button[aria-label*='停止']",
                "button[aria-label*='Stop']",
            ],
            reply_selectors: &[
                "[class*='markdown']",
                "[data-role='assistant']",
                "[data-message-author-role='assistant']",
                ".assistant-message",
            ],
        })
    }
}
