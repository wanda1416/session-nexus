use crate::agents::web::WebSiteConfig;
use crate::agents::AgentAdapter;
use crate::provider::{ProviderKind, ProviderProfile};

pub struct GeminiWebAgent;

impl AgentAdapter for GeminiWebAgent {
    fn id(&self) -> &'static str {
        "gemini-web"
    }

    fn profile(&self) -> ProviderProfile {
        ProviderProfile {
            id: "gemini-web".into(),
            display_name: "Gemini Web".into(),
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
            id: "gemini-web",
            url_matches: &["gemini.google.com/app", "gemini.google.com"],
            open_url: "https://gemini.google.com/app",
            new_chat_button_selectors: &[],
            input_selectors: &[
                "rich-textarea .ql-editor",
                "div.ql-editor[contenteditable='true']",
                "div[contenteditable='true']",
                "textarea",
            ],
            submit_selectors: &[
                "button.send-button",
                "button[aria-label*='发送']",
                "button[aria-label*='Send']",
            ],
            stop_selectors: &[
                "button[aria-label*='停止']",
                "button[aria-label*='Stop']",
            ],
            reply_selectors: &[
                "model-response message-content",
                "model-response .markdown",
                "model-response",
            ],
        })
    }
}
