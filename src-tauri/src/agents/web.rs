use std::time::{Duration, Instant};

use serde_json::json;

use crate::browser_mcp::Bridge;
use crate::runner::{clip, split_dispatch, AgentRun};

pub struct WebSiteConfig {
    pub id: &'static str,
    pub url_matches: &'static [&'static str],
    pub open_url: &'static str,
    pub new_chat_button_selectors: &'static [&'static str],
    pub input_selectors: &'static [&'static str],
    pub submit_selectors: &'static [&'static str],
    pub stop_selectors: &'static [&'static str],
    pub reply_selectors: &'static [&'static str],
}

pub async fn run_web_agent(
    bridge: &Bridge,
    site: &WebSiteConfig,
    prompt: &str,
    native_session_id: Option<&str>,
    role: &str,
) -> AgentRun {
    match execute(bridge, site, prompt, native_session_id, role).await {
        Ok(run) => run,
        Err(err) => AgentRun {
            provider: site.id.to_string(),
            ok: false,
            text: err,
            native_session_id: None,
            dispatch_task: None,
        },
    }
}

async fn execute(
    bridge: &Bridge,
    site: &WebSiteConfig,
    prompt: &str,
    native_session_id: Option<&str>,
    role: &str,
) -> Result<AgentRun, String> {
    let bound = if role == "room" {
        native_session_id.filter(|id| !id.trim().is_empty())
    } else {
        None
    };

    let (tab_id, is_fresh) = open_or_attach_session(bridge, site, bound).await?;
    let mut input_selector = wait_for_input(bridge, tab_id, site.input_selectors).await?;

    if is_fresh && !site.new_chat_button_selectors.is_empty() {
        for selector in site.new_chat_button_selectors {
            if bridge
                .call_tool("click", json!({ "tabId": tab_id, "selector": selector }))
                .await
                .is_ok()
            {
                tokio::time::sleep(Duration::from_millis(400)).await;
                input_selector = wait_for_input(bridge, tab_id, site.input_selectors).await?;
                break;
            }
        }
    }

    let (prev_count, previous) = read_latest_reply(bridge, tab_id, site.reply_selectors).await;

    bridge
        .call_tool(
            "fill",
            json!({ "tabId": tab_id, "selector": input_selector, "value": prompt }),
        )
        .await?;

    submit(bridge, tab_id, &input_selector, site.submit_selectors).await?;

    let raw_reply = wait_for_reply(bridge, tab_id, site, prev_count, &previous).await?;
    let session_id = resolve_session_id(bridge, site, tab_id, bound).await;
    let formatted = clip(&raw_reply);
    let (visible, dispatch_task) = if role == "room" {
        split_dispatch(&formatted)
    } else {
        (formatted, None)
    };

    Ok(AgentRun {
        provider: site.id.to_string(),
        ok: true,
        text: visible,
        native_session_id: Some(session_id),
        dispatch_task: if role == "room" { dispatch_task } else { None },
    })
}

async fn open_or_attach_session(
    bridge: &Bridge,
    site: &WebSiteConfig,
    bound: Option<&str>,
) -> Result<(i64, bool), String> {
    if let Some(session_ref) = bound {
        if session_ref.starts_with("http://") || session_ref.starts_with("https://") {
            if let Ok(tab) = bridge
                .call_tool("find_tab", json!({ "url": session_ref, "nexusOnly": true }))
                .await
            {
                if let Some(id) = tab.get("tabId").and_then(|v| v.as_i64()) {
                    return Ok((id, false));
                }
            }
            let opened = bridge
                .call_tool("open_tab", json!({ "url": session_ref, "isolated": true }))
                .await?;
            let id = opened
                .get("tabId")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "未能获取已绑定会话的 tabId".to_string())?;
            return Ok((id, false));
        }

        if let Some(raw_id) = session_ref.strip_prefix("tab:") {
            if let Ok(tab_id) = raw_id.parse::<i64>() {
                if let Ok(tab) = bridge.call_tool("get_tab", json!({ "tabId": tab_id })).await {
                    let url = tab.get("url").and_then(|v| v.as_str()).unwrap_or("");
                    if site.url_matches.iter().any(|needle| url.contains(needle)) {
                        return Ok((tab_id, false));
                    }
                }
            }
        }
    }

    // No bound session (or work call): always open a brand-new tab inside the isolated Nexus window.
    let opened = bridge
        .call_tool("open_tab", json!({ "url": site.open_url, "isolated": true }))
        .await?;
    let id = opened
        .get("tabId")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "未能获取新标签页 tabId".to_string())?;
    Ok((id, true))
}

async fn resolve_session_id(
    bridge: &Bridge,
    site: &WebSiteConfig,
    tab_id: i64,
    bound: Option<&str>,
) -> String {
    if let Ok(tab) = bridge.call_tool("get_tab", json!({ "tabId": tab_id })).await {
        let url = tab.get("url").and_then(|v| v.as_str()).unwrap_or("").trim();
        let root_trimmed = site.open_url.trim_end_matches('/');
        let url_trimmed = url.trim_end_matches('/');
        if !url.is_empty() && url_trimmed != root_trimmed {
            return url.to_string();
        }
    }
    if let Some(existing) = bound.filter(|s| s.starts_with("http")) {
        return existing.to_string();
    }
    format!("tab:{tab_id}")
}

async fn wait_for_input(
    bridge: &Bridge,
    tab_id: i64,
    selectors: &[&str],
) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        for selector in selectors {
            if let Ok(res) = bridge
                .call_tool("exists", json!({ "tabId": tab_id, "selector": selector }))
                .await
            {
                if res.get("exists").and_then(|v| v.as_bool()) == Some(true) {
                    return Ok((*selector).to_string());
                }
            }
        }
        if Instant::now() >= deadline {
            return Err("未在目标网页找到输入框（请确认已登录并打开页面）".into());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn submit(
    bridge: &Bridge,
    tab_id: i64,
    input_selector: &str,
    submit_selectors: &[&str],
) -> Result<(), String> {
    tokio::time::sleep(Duration::from_millis(150)).await;
    for selector in submit_selectors {
        if bridge
            .call_tool("click", json!({ "tabId": tab_id, "selector": selector }))
            .await
            .is_ok()
        {
            return Ok(());
        }
    }
    bridge
        .call_tool(
            "press_key",
            json!({ "tabId": tab_id, "selector": input_selector, "key": "Enter" }),
        )
        .await?;
    Ok(())
}

async fn read_latest_reply(bridge: &Bridge, tab_id: i64, selectors: &[&str]) -> (u64, String) {
    for selector in selectors {
        if let Ok(res) = bridge
            .call_tool(
                "read_text",
                json!({ "tabId": tab_id, "selector": selector, "lastOnly": true }),
            )
            .await
        {
            let count = res.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            let text = res.get("text").and_then(|v| v.as_str()).unwrap_or("").trim();
            if count > 0 || !text.is_empty() {
                return (count, text.to_string());
            }
        }
    }
    (0, String::new())
}

async fn is_generating(bridge: &Bridge, tab_id: i64, stop_selectors: &[&str]) -> bool {
    for selector in stop_selectors {
        if let Ok(res) = bridge
            .call_tool("exists", json!({ "tabId": tab_id, "selector": selector }))
            .await
        {
            if res.get("exists").and_then(|v| v.as_bool()) == Some(true) {
                return true;
            }
        }
    }
    false
}

async fn wait_for_reply(
    bridge: &Bridge,
    tab_id: i64,
    site: &WebSiteConfig,
    prev_count: u64,
    previous: &str,
) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut last_seen = String::new();
    let mut stable_ticks = 0u8;

    loop {
        tokio::time::sleep(Duration::from_millis(600)).await;
        let generating = is_generating(bridge, tab_id, site.stop_selectors).await;
        let (count, current) = read_latest_reply(bridge, tab_id, site.reply_selectors).await;

        let is_new = !current.is_empty() && (count > prev_count || current != previous);
        if is_new {
            if current == last_seen {
                stable_ticks = stable_ticks.saturating_add(1);
            } else {
                last_seen = current.clone();
                stable_ticks = 0;
            }
            if !generating && stable_ticks >= 2 {
                return Ok(current);
            }
        }

        if Instant::now() >= deadline {
            if !last_seen.is_empty() {
                return Ok(last_seen);
            }
            return Err("等待网页回复超时（90 秒）".into());
        }
    }
}
