pub mod browser_mcp;
pub mod connector;
pub mod discover;
pub mod models;
pub mod provider;

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::discover::discover_sessions;
    use crate::provider::{command_for, phase1_profiles, ProviderKind};

    #[test]
    fn phase1_provider_ids_are_unique() {
        let profiles = phase1_profiles();
        let mut ids: Vec<_> = profiles.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), profiles.len());
    }

    #[test]
    fn cli_commands_include_prompt_and_resume() {
        let profiles = phase1_profiles();
        let grok = profiles.iter().find(|p| p.id == "grok").unwrap();
        let fresh = command_for(grok, "hello", None).unwrap();
        assert_eq!(fresh, vec!["-p", "hello", "--output-format", "json"]);
        let again = command_for(grok, "next", Some("abc")).unwrap();
        assert!(again.iter().any(|a| a == "abc"));

        let dsh = profiles.iter().find(|p| p.id == "dsh").unwrap();
        assert!(command_for(dsh, "task", Some("old")).is_err());

        let web = profiles.iter().find(|p| p.id == "browser-mcp").unwrap();
        assert_eq!(web.kind, ProviderKind::BrowserMcp);
        assert_eq!(web.mcp_url.as_deref(), Some("http://127.0.0.1:19877/mcp"));
        assert!(web.mcp_tools.contains(&"find_tab".to_string()));
        assert!(command_for(web, "task", None).unwrap_err().contains("MCP"));
    }

    #[test]
    fn discover_indexes_nested_session_dirs() {
        let root = std::env::temp_dir().join(format!("nexus-discover-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("proj").join("session-a")).unwrap();
        fs::create_dir_all(root.join("proj").join("session-b")).unwrap();
        let mut profile = phase1_profiles().into_iter().find(|p| p.id == "grok").unwrap();
        profile.session_dir = root.to_string_lossy().into_owned();
        let found = discover_sessions(&profile);
        let _ = fs::remove_dir_all(&root);
        let mut ids: Vec<_> = found.iter().filter_map(|s| s.native_session_id.clone()).collect();
        ids.sort();
        assert_eq!(ids, vec!["session-a", "session-b"]);
    }
}

use provider::phase1_profiles;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct CoreStatus {
    phase: &'static str,
    providers: Vec<String>,
}

#[tauri::command]
fn core_status() -> CoreStatus {
    CoreStatus {
        phase: "1",
        providers: phase1_profiles().into_iter().map(|p| p.id).collect(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    browser_mcp::spawn(browser_mcp::DEFAULT_ADDR);
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![core_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
