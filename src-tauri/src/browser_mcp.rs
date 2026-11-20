//! Local MCP server for the SessionNexus browser extension.
//!
//! Clients call `POST /mcp`. The extension connects to `GET /extension` as a
//! WebSocket. Page work happens in the extension (`chrome.tabs` / `chrome.scripting`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex};

pub const DEFAULT_ADDR: &str = "127.0.0.1:19877";

const TOOLS: &[&str] = &[
    "list_tabs",
    "find_tab",
    "get_tab",
    "open_tab",
    "snapshot",
    "fill",
    "click",
    "press_key",
    "read_text",
    "exists",
];

struct Pending(oneshot::Sender<Result<Value, String>>);

#[derive(Clone)]
pub struct Bridge {
    inner: Arc<Inner>,
}

struct Inner {
    extension: Mutex<Option<mpsc::UnboundedSender<String>>>,
    pending: Mutex<HashMap<String, Pending>>,
}

impl Bridge {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                extension: Mutex::new(None),
                pending: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn extension_connected(&self) -> bool {
        self.inner.extension.lock().await.is_some()
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, String> {
        if !TOOLS.contains(&name) {
            return Err(format!("unknown tool {name}"));
        }
        let tx = self
            .inner
            .extension
            .lock()
            .await
            .clone()
            .ok_or_else(|| "扩展未连接".to_string())?;
        let id = format!("{:016x}", rand_id());
        let (done_tx, done_rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id.clone(), Pending(done_tx));
        let outbound = json!({ "id": id, "method": name, "params": arguments }).to_string();
        if tx.send(outbound).is_err() {
            self.inner.pending.lock().await.remove(&id);
            *self.inner.extension.lock().await = None;
            return Err("扩展未连接".into());
        }
        match tokio::time::timeout(Duration::from_secs(8), done_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("扩展已断开".into()),
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                Err("扩展没有在 8 秒内响应".into())
            }
        }
    }

    async fn resolve(&self, id: &str, result: Result<Value, String>) {
        if let Some(Pending(tx)) = self.inner.pending.lock().await.remove(id) {
            let _ = tx.send(result);
        }
    }
}

pub async fn handle_rpc(bridge: &Bridge, request: Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    if id.is_none() {
        return None;
    }
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "session-nexus-browser", "version": "0.1.0" }
        })),
        "tools/list" => Ok(json!({ "tools": tool_defs() })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            match bridge.call_tool(name, arguments).await {
                Ok(value) => Ok(json!({
                    "content": [{ "type": "text", "text": value.to_string() }],
                    "isError": false
                })),
                Err(err) => Ok(json!({
                    "content": [{ "type": "text", "text": err }],
                    "isError": true
                })),
            }
        }
        _ => Err(format!("unknown method {method}")),
    };
    Some(match result {
        Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
        Err(message) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": message }
        }),
    })
}

fn tool_defs() -> Vec<Value> {
    vec![
        tool("list_tabs", "List open browser tabs.", json!({ "type": "object", "properties": {} })),
        tool(
            "find_tab",
            "Find an open tab whose URL contains the given string.",
            json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }),
        ),
        tool(
            "get_tab",
            "Get the current URL and title of a tab by tabId.",
            json!({
                "type": "object",
                "properties": { "tabId": { "type": "number" } },
                "required": ["tabId"]
            }),
        ),
        tool(
            "open_tab",
            "Open a new browser tab at the given URL.",
            json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }),
        ),
        tool(
            "snapshot",
            "Read the page text and mark interactive elements with @e refs.",
            json!({
                "type": "object",
                "properties": { "tabId": { "type": "number" } },
                "required": ["tabId"]
            }),
        ),
        tool(
            "fill",
            "Replace the text of an input or contenteditable element.",
            json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "selector": { "type": "string" },
                    "value": { "type": "string" }
                },
                "required": ["tabId", "selector", "value"]
            }),
        ),
        tool(
            "click",
            "Click an element by CSS selector or @e ref.",
            json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "selector": { "type": "string" }
                },
                "required": ["tabId", "selector"]
            }),
        ),
        tool(
            "press_key",
            "Dispatch a keyboard key event on an element.",
            json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "selector": { "type": "string" },
                    "key": { "type": "string" },
                    "ctrlKey": { "type": "boolean" },
                    "shiftKey": { "type": "boolean" }
                },
                "required": ["tabId", "key"]
            }),
        ),
        tool(
            "read_text",
            "Read innerText from elements matching a CSS selector.",
            json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "selector": { "type": "string" },
                    "lastOnly": { "type": "boolean" }
                },
                "required": ["tabId", "selector"]
            }),
        ),
        tool(
            "exists",
            "Check whether any element matches a CSS selector.",
            json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "selector": { "type": "string" }
                },
                "required": ["tabId", "selector"]
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

pub fn router(bridge: Bridge) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/mcp", post(mcp))
        .route("/extension", get(extension_ws))
        .with_state(bridge)
}

async fn health(State(bridge): State<Bridge>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "extension": bridge.extension_connected().await
    }))
}

async fn mcp(
    State(bridge): State<Bridge>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    match handle_rpc(&bridge, request).await {
        Some(response) => Ok(Json(response)),
        None => Err(StatusCode::ACCEPTED),
    }
}

async fn extension_ws(State(bridge): State<Bridge>, upgrade: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    upgrade.on_upgrade(move |socket| hold_extension(bridge, socket))
}

async fn hold_extension(bridge: Bridge, socket: WebSocket) {
    let (mut write, mut read) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    *bridge.inner.extension.lock().await = Some(tx);
    loop {
        tokio::select! {
            outgoing = rx.recv() => {
                let Some(text) = outgoing else { break };
                if write.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = read.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                            if value.get("type").and_then(Value::as_str) == Some("ping") {
                                let _ = write.send(Message::Text("{\"type\":\"pong\"}".into())).await;
                                continue;
                            }
                            if let Some(id) = value.get("id").and_then(Value::as_str) {
                                let result = if let Some(err) = value.get("error").and_then(Value::as_str) {
                                    Err(err.to_string())
                                } else {
                                    Ok(value.get("result").cloned().unwrap_or(Value::Null))
                                };
                                bridge.resolve(id, result).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    *bridge.inner.extension.lock().await = None;
    let leftover: Vec<_> = bridge.inner.pending.lock().await.drain().collect();
    for (_, Pending(tx)) in leftover {
        let _ = tx.send(Err("扩展已断开".into()));
    }
}

pub fn spawn(addr: &str) -> Bridge {
    let addr = addr.to_string();
    let bridge = Bridge::new();
    let server_bridge = bridge.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("browser mcp runtime");
        runtime.block_on(async move {
            if let Err(err) = listen_with_bridge(&addr, server_bridge).await {
                eprintln!("browser mcp: {err}");
            }
        });
    });
    bridge
}

pub async fn listen(addr: &str) -> Result<(), String> {
    listen_with_bridge(addr, Bridge::new()).await
}

pub async fn listen_with_bridge(addr: &str, bridge: Bridge) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| format!("bind {addr}: {err}"))?;
    axum::serve(listener, router(bridge))
        .await
        .map_err(|err| err.to_string())
}

fn rand_id() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    nanos ^ SEQ.fetch_add(1, Ordering::Relaxed).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_tools_without_an_extension() {
        let bridge = Bridge::new();
        let response = handle_rpc(
            &bridge,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        )
        .await
        .unwrap();
        let names: Vec<_> = response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, TOOLS);
    }

    #[tokio::test]
    async fn call_fails_when_the_extension_is_absent() {
        let bridge = Bridge::new();
        let response = handle_rpc(
            &bridge,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": { "name": "list_tabs", "arguments": {} }
            }),
        )
        .await
        .unwrap();
        assert_eq!(response["result"]["isError"], true);
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("扩展未连接"));
    }

    #[tokio::test]
    async fn call_returns_the_extension_result() {
        let bridge = Bridge::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        *bridge.inner.extension.lock().await = Some(tx);
        let bridge2 = bridge.clone();
        let caller = tokio::spawn(async move {
            handle_rpc(
                &bridge2,
                json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": { "name": "find_tab", "arguments": { "url": "https://example.com" } }
                }),
            )
            .await
        });
        let raw = rx.recv().await.unwrap();
        let outbound: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(outbound["method"], "find_tab");
        let id = outbound["id"].as_str().unwrap().to_string();
        bridge
            .resolve(&id, Ok(json!({ "tabId": 7, "url": "https://example.com" })))
            .await;
        let response = caller.await.unwrap().unwrap();
        assert_eq!(response["result"]["isError"], false);
        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("\"tabId\":7"));
    }
}
