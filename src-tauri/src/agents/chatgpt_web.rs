use crate::agents::web::WebSiteConfig;
use crate::agents::AgentAdapter;
use crate::provider::{ProviderKind, ProviderProfile};

pub struct ChatGptWebAgent;

impl AgentAdapter for ChatGptWebAgent {
    fn id(&self) -> &'static str {
        "chatgpt-web"
    }

    fn profile(&self) -> ProviderProfile {
        ProviderProfile {
            id: "chatgpt-web".into(),
            display_name: "ChatGPT Web".into(),
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
            id: "chatgpt-web",
            url_matches: &["chatgpt.com", "chat.openai.com"],
            open_url: "https://chatgpt.com/",
            new_chat_button_selectors: &[],
            input_selectors: &[
                "#prompt-textarea",
                "div.ProseMirror[contenteditable='true']",
                "div[contenteditable='true']",
                "textarea",
            ],
            submit_selectors: &[
                "button[data-testid='send-button']",
                "button[aria-label*='发送']",
                "button[aria-label*='Send']",
            ],
            stop_selectors: &[
                "button[data-testid='stop-button']",
                "button[aria-label*='停止']",
                "button[aria-label*='Stop']",
            ],
            reply_selectors: &[
                "[data-message-author-role='assistant'] .markdown",
                "[data-message-author-role='assistant']",
                "[class*='prose']",
            ],
        })
    }
}
