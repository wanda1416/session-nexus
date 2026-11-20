pub mod browser;
pub mod chatgpt_web;
pub mod codex;
pub mod dsh;
pub mod dsh_web;
pub mod gemini_web;
pub mod grok;
pub mod qoder;
pub mod web;

use std::path::Path;

use crate::provider::{command_for, ProviderProfile};
use crate::runner::clip;

/// Per-agent CLI adapter encapsulating profile, argument construction, and output parsing.
pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn profile(&self) -> ProviderProfile;

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
                Some(_id) if profile.resume_args.is_empty() => {
                    (command_for(&profile, prompt, None)?, None)
                }
                Some(id) => (command_for(&profile, prompt, Some(id))?, Some(id.to_string())),
                None => self.new_room_args(&profile, prompt)?,
            }
        } else {
            (command_for(&profile, prompt, None)?, None)
        };
        if let Some(dir) = cwd.filter(|dir| Path::new(dir).is_dir()) {
            self.apply_cwd(&mut args, dir);
        }
        Ok((args, assigned_id))
    }

    fn new_room_args(
        &self,
        profile: &ProviderProfile,
        prompt: &str,
    ) -> Result<(Vec<String>, Option<String>), String> {
        Ok((command_for(profile, prompt, None)?, None))
    }

    fn apply_cwd(&self, _args: &mut Vec<String>, _dir: &str) {}

    fn apply_model(&self, _args: &mut Vec<String>, _model: &str) {}

    fn default_api_key_env(&self) -> Option<&'static str> {
        None
    }

    fn parse_session_id(&self, _raw: &str) -> Option<String> {
        None
    }

    fn format_output(&self, raw: &str) -> String {
        clip(raw)
    }

    fn web_site(&self) -> Option<web::WebSiteConfig> {
        None
    }
}

pub fn all_agents() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(qoder::QoderAgent),
        Box::new(grok::GrokAgent),
        Box::new(codex::CodexAgent),
        Box::new(dsh::DshAgent),
        Box::new(dsh_web::DshWebAgent),
        Box::new(gemini_web::GeminiWebAgent),
        Box::new(chatgpt_web::ChatGptWebAgent),
        Box::new(browser::BrowserMcpAgent),
    ]
}

pub fn find_agent(id: &str) -> Option<Box<dyn AgentAdapter>> {
    all_agents().into_iter().find(|agent| agent.id() == id)
}

pub fn all_profiles() -> Vec<ProviderProfile> {
    all_agents().into_iter().map(|agent| agent.profile()).collect()
}
