// SPDX-License-Identifier: GPL-3.0-or-later

//! Minimal MCP stdio server for the ATVV daemon.
//!
//! Newline-delimited JSON-RPC 2.0, supporting the handshake revisions
//! `2024-11-05` through `2025-11-25`: `initialize`, `tools/list`, and
//! `tools/call`. The server controls the daemon; it never receives audio.
//! Transcripts belong to fcitx5-vinput and the agent's own text delivery.

use serde_json::{Value, json};

mod backend;

pub use backend::{Backend, DaemonBackend};

/// Newest handshake revision supported by this server.
pub const PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Tool names exposed to MCP clients.
pub const TOOLS: &[&str] = &[
    "status",
    "contract",
    "voice_start",
    "voice_stop",
    "voice_toggle",
    "list_devices",
    "reload",
];

/// JSON-RPC request dispatcher.
pub struct Server<B: Backend> {
    backend: B,
}

impl<B: Backend> Server<B> {
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Handle one protocol line. Returns a JSON response line, or `None` for
    /// notifications.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        match serde_json::from_str::<Value>(line) {
            Ok(message) => self.handle_message(message),
            Err(error) => Some(encode(error_response(
                Value::Null,
                PARSE_ERROR,
                format!("parse error: {error}"),
            ))),
        }
    }

    /// Handle one decoded JSON-RPC message.
    pub fn handle_message(&mut self, message: Value) -> Option<String> {
        let Some(object) = message.as_object() else {
            return Some(encode(error_response(
                Value::Null,
                INVALID_REQUEST,
                "expected a JSON-RPC object",
            )));
        };
        let id = object.get("id").cloned();
        let method = object
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if method.is_empty() {
            return id.map(|id| encode(error_response(id, INVALID_REQUEST, "missing method")));
        }

        let params = object.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => id.map(|id| encode(result_response(id, initialize(&params)))),
            "ping" => id.map(|id| encode(result_response(id, json!({})))),
            "tools/list" => id.map(|id| encode(result_response(id, json!({ "tools": tools() })))),
            "tools/call" => id.map(|id| self.call_tool(id, &params)),
            // Notifications (`notifications/initialized`, `notifications/cancelled`,
            // ...) are acknowledgements only.
            _ if id.is_none() => None,
            other => id.map(|id| {
                encode(error_response(
                    id,
                    METHOD_NOT_FOUND,
                    format!("method not found: {other}"),
                ))
            }),
        }
    }

    fn call_tool(&mut self, id: Value, params: &Value) -> String {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return encode(error_response(
                id,
                INVALID_PARAMS,
                "tools/call requires params.name",
            ));
        };
        let result = match name {
            "status" => self.backend.status().map(|status| pretty(&status)),
            "contract" => self.backend.contract().map(|contract| pretty(&contract)),
            "voice_start" => self
                .backend
                .voice_start()
                .map(|()| pretty(&json!({"ok": true, "tool": name}))),
            "voice_stop" => self
                .backend
                .voice_stop()
                .map(|()| pretty(&json!({"ok": true, "tool": name}))),
            "voice_toggle" => self
                .backend
                .voice_toggle()
                .map(|()| pretty(&json!({"ok": true, "tool": name}))),
            "list_devices" => self
                .backend
                .list_devices()
                .map(|devices| pretty(&json!({"devices": devices}))),
            "reload" => self
                .backend
                .reload()
                .map(|()| pretty(&json!({"ok": true, "tool": name}))),
            other => {
                return encode(error_response(
                    id,
                    INVALID_PARAMS,
                    format!("unknown tool: {other}"),
                ));
            }
        };
        match result {
            Ok(text) => encode(result_response(
                id,
                json!({
                    "content": [{"type": "text", "text": text}],
                    "isError": false,
                }),
            )),
            Err(error) => encode(result_response(
                id,
                json!({
                    "content": [{"type": "text", "text": error}],
                    "isError": true,
                }),
            )),
        }
    }
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);
    let version = if SUPPORTED_VERSIONS.contains(&requested) {
        requested
    } else {
        PROTOCOL_VERSION
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "atvv-mcp",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "Controls the paired ATVV remote through org.atvv.Daemon. \
                         Audio is captured by fcitx5-vinput from the resident PipeWire \
                         source; this server never receives audio and cannot deliver text.",
    })
}

fn tools() -> Value {
    let schema = json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    });
    let tool = |name: &str, description: &str| {
        json!({
            "name": name,
            "description": description,
            "inputSchema": schema.clone(),
        })
    };
    json!([
        tool(
            "status",
            "Daemon status as JSON: session state, device, battery and hardware \
             facts, caps, audio counters, input, vinput mode, contract, config, \
             and the last error."
        ),
        tool(
            "contract",
            "Provider contract as JSON: profile, source node, media class, sample \
             format, declared recording keys, active key, input/keyboard state, \
             D-Bus/MCP endpoints, and a consumer recipe."
        ),
        tool(
            "voice_start",
            "Open the remote microphone (MIC_OPEN). On the RC003 audio frames \
             additionally require the physical voice key to be held."
        ),
        tool("voice_stop", "Close the remote microphone (MIC_CLOSE)."),
        tool(
            "voice_toggle",
            "Start voice when idle; stop it while opening or streaming."
        ),
        tool("list_devices", "List configured ATVV device addresses."),
        tool(
            "reload",
            "Re-read $XDG_CONFIG_HOME/atvv/daemon.toml (gain, input, vinput)."
        ),
    ])
}

fn result_response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message.into()},
    })
}

fn encode(value: Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|error| {
        format!(
            r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32603,"message":"serialization failed: {error}"}}}}"#
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeBackend {
        calls: Vec<&'static str>,
        fail: bool,
    }

    impl Backend for FakeBackend {
        fn status(&mut self) -> Result<Value, String> {
            self.calls.push("status");
            if self.fail {
                return Err("daemon is not running".to_string());
            }
            Ok(json!({"state": "ready", "connected": true}))
        }

        fn contract(&mut self) -> Result<Value, String> {
            self.calls.push("contract");
            if self.fail {
                return Err("daemon is not running".to_string());
            }
            Ok(json!({
                "version": "1",
                "profile": "miremote-rc003",
                "source": "atvv-miremote-rc003",
                "media_class": "Audio/Source",
                "recording_keys": ["DICTATE", "VOICECOMMAND", "ASSISTANT"],
                "active_key": "DICTATE"
            }))
        }

        fn voice_start(&mut self) -> Result<(), String> {
            self.calls.push("voice_start");
            Ok(())
        }

        fn voice_stop(&mut self) -> Result<(), String> {
            self.calls.push("voice_stop");
            Ok(())
        }

        fn voice_toggle(&mut self) -> Result<(), String> {
            self.calls.push("voice_toggle");
            Ok(())
        }

        fn list_devices(&mut self) -> Result<Vec<String>, String> {
            self.calls.push("list_devices");
            Ok(vec!["C0:5D:39:F8:2D:4C".to_string()])
        }

        fn reload(&mut self) -> Result<(), String> {
            self.calls.push("reload");
            Ok(())
        }
    }

    fn request(server: &mut Server<FakeBackend>, method: &str, params: Value) -> Value {
        let line = json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params});
        let encoded = server.handle_message(line).expect("response");
        serde_json::from_str(&encoded).expect("valid JSON")
    }

    #[test]
    fn initialize_negotiates_a_supported_version() {
        let mut server = Server::new(FakeBackend::default());
        let response = request(
            &mut server,
            "initialize",
            json!({"protocolVersion": "2025-11-25"}),
        );
        assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(response["result"]["serverInfo"]["name"], "atvv-mcp");
        assert!(response["result"]["capabilities"]["tools"].is_object());

        let old = request(
            &mut server,
            "initialize",
            json!({"protocolVersion": "2024-11-05"}),
        );
        assert_eq!(old["result"]["protocolVersion"], "2024-11-05");

        let future = request(
            &mut server,
            "initialize",
            json!({"protocolVersion": "9999-01-01"}),
        );
        assert_eq!(future["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn tools_list_has_the_documented_tools() {
        let mut server = Server::new(FakeBackend::default());
        let response = request(&mut server, "tools/list", Value::Null);
        let listed: Vec<&str> = response["result"]["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect();
        assert_eq!(listed, TOOLS);
        for tool in response["result"]["tools"].as_array().expect("tools array") {
            assert!(
                tool["description"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty())
            );
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn tools_call_dispatches_and_returns_text_content() {
        let mut server = Server::new(FakeBackend::default());
        let response = request(
            &mut server,
            "tools/call",
            json!({"name": "status", "arguments": {}}),
        );
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text content");
        let status: Value = serde_json::from_str(text).expect("status JSON");
        assert_eq!(status["state"], "ready");
        assert_eq!(response["result"]["isError"], false);

        let response = request(
            &mut server,
            "tools/call",
            json!({"name": "voice_toggle", "arguments": {}}),
        );
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(server.backend.calls, ["status", "voice_toggle"]);
    }

    #[test]
    fn contract_tool_returns_the_provider_contract() {
        let mut server = Server::new(FakeBackend::default());
        let response = request(
            &mut server,
            "tools/call",
            json!({"name": "contract", "arguments": {}}),
        );
        assert_eq!(response["result"]["isError"], false);
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("contract text");
        let contract: Value = serde_json::from_str(text).expect("contract JSON");
        assert_eq!(contract["version"], "1");
        assert_eq!(contract["source"], "atvv-miremote-rc003");
        assert_eq!(contract["media_class"], "Audio/Source");
        assert_eq!(
            contract["recording_keys"],
            json!(["DICTATE", "VOICECOMMAND", "ASSISTANT"])
        );
        assert_eq!(server.backend.calls, ["contract"]);
    }

    #[test]
    fn contract_tool_reports_backend_failure() {
        let mut server = Server::new(FakeBackend {
            fail: true,
            ..FakeBackend::default()
        });
        let response = request(
            &mut server,
            "tools/call",
            json!({"name": "contract", "arguments": {}}),
        );
        assert_eq!(response["result"]["isError"], true);
        assert!(response["error"].is_null());
    }

    #[test]
    fn backend_errors_are_tool_results_not_protocol_errors() {
        let mut server = Server::new(FakeBackend {
            fail: true,
            ..FakeBackend::default()
        });
        let response = request(
            &mut server,
            "tools/call",
            json!({"name": "status", "arguments": {}}),
        );
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .is_some_and(|text| text.contains("not running"))
        );
        assert!(response["error"].is_null());
    }

    #[test]
    fn unknown_tool_and_method_are_protocol_errors() {
        let mut server = Server::new(FakeBackend::default());
        let response = request(
            &mut server,
            "tools/call",
            json!({"name": "teleport", "arguments": {}}),
        );
        assert_eq!(response["error"]["code"], INVALID_PARAMS);

        let response = request(&mut server, "resources/list", Value::Null);
        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn notifications_have_no_response_and_parse_errors_are_reported() {
        let mut server = Server::new(FakeBackend::default());
        let notification = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert!(server.handle_message(notification).is_none());

        let response = server
            .handle_line("{not json")
            .expect("parse error response");
        let parsed: Value = serde_json::from_str(&response).expect("valid JSON");
        assert_eq!(parsed["error"]["code"], PARSE_ERROR);
        assert!(parsed["id"].is_null());
    }

    #[test]
    fn tools_call_requires_a_name() {
        let mut server = Server::new(FakeBackend::default());
        let response = request(&mut server, "tools/call", json!({}));
        assert_eq!(response["error"]["code"], INVALID_PARAMS);
    }
}
