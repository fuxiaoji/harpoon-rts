//! harpoon-mcp: a Model Context Protocol (MCP) stdio server for the harpoon
//! engine.
//!
//! The wire protocol is newline-delimited JSON-RPC 2.0, implemented by hand on
//! top of `serde_json` (no MCP SDK). stdout carries protocol messages and
//! nothing else; diagnostics go to stderr.

mod tools;

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn main() {
    eprintln!("harpoon-mcp 0.1.0: MCP stdio server (JSON-RPC 2.0)");

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut session = tools::Session::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                eprintln!("harpoon-mcp: stdin read error: {e}");
                break;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(line) {
            Ok(msg) => handle_message(&mut session, &msg),
            Err(e) => {
                eprintln!("harpoon-mcp: JSON parse error: {e}");
                Some(error_response(Value::Null, -32700, "Parse error"))
            }
        };

        if let Some(response) = response {
            let text = match serde_json::to_string(&response) {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("harpoon-mcp: failed to serialize response: {e}");
                    continue;
                }
            };
            if writeln!(out, "{text}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
}

/// Handle one decoded JSON value. `None` means "send nothing" (notifications).
fn handle_message(session: &mut tools::Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let Some(obj) = msg.as_object() else {
        return Some(error_response(Value::Null, -32600, "Invalid Request"));
    };

    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return match id {
            Some(id) => Some(error_response(
                id,
                -32600,
                "Invalid Request: missing method",
            )),
            None => None,
        };
    };

    // MCP lifecycle notifications never receive a response, even if a client
    // mistakenly attaches an id to one.
    if method.starts_with("notifications/") {
        return None;
    }

    // JSON-RPC notifications (requests without an id) get no response.
    let Some(id) = id else {
        return None;
    };

    match method {
        "initialize" => Some(success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "harpoon-mcp", "version": "0.1.0"}
            }),
        )),
        "ping" => Some(success(id, json!({}))),
        "tools/list" => Some(success(id, json!({"tools": tools::tool_definitions()}))),
        "tools/call" => Some(handle_tools_call(
            session,
            id,
            obj.get("params"),
        )),
        other => Some(error_response(
            id,
            -32601,
            &format!("Method not found: {other}"),
        )),
    }
}

fn handle_tools_call(session: &mut tools::Session, id: Value, params: Option<&Value>) -> Value {
    let Some(params) = params.filter(|p| p.is_object()) else {
        return error_response(id, -32602, "Invalid params: expected an object");
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error_response(id, -32602, "Invalid params: missing tool name");
    };
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);
    if !args.is_null() && !args.is_object() {
        return error_response(id, -32602, "Invalid params: arguments must be an object");
    }

    let (text, is_error) = match session.call(name, &args) {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(text) => (text, false),
            Err(e) => (
                json!({"error": format!("failed to serialize tool result: {e}")}).to_string(),
                true,
            ),
        },
        Err(e) => (json!({"error": e}).to_string(), true),
    };

    success(
        id,
        json!({
            "content": [{"type": "text", "text": text}],
            "isError": is_error
        }),
    )
}

fn success(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message}
    })
}
