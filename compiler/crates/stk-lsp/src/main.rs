//! Minimal Steampunk LSP (diagnostics via compile).
//!
//! Run: `cargo run -p stk-lsp --manifest-path compiler/Cargo.toml`
//! Configure the editor to use stdio LSP.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use serde_json::{json, Value};

fn main() {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout();
    loop {
        let Some(msg) = read_message(&mut reader) else {
            break;
        };
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = msg.get("id").cloned();
        match method {
            "initialize" => {
                respond(
                    &mut stdout,
                    id,
                    json!({
                        "capabilities": {
                            "textDocumentSync": 1,
                            "hoverProvider": true
                        },
                        "serverInfo": { "name": "steampunk-lsp", "version": "0.1.0" }
                    }),
                );
            }
            "initialized" | "shutdown" => {
                if method == "shutdown" {
                    respond(&mut stdout, id, json!(null));
                }
            }
            "exit" => break,
            "textDocument/didOpen" | "textDocument/didChange" => {
                if let Some(params) = msg.get("params") {
                    publish_diagnostics(&mut stdout, params);
                }
            }
            "textDocument/hover" => {
                respond(
                    &mut stdout,
                    id,
                    json!({
                        "contents": {
                            "kind": "markdown",
                            "value": "Steampunk LSP — see SPEC.md"
                        }
                    }),
                );
            }
            _ => {
                if id.is_some() {
                    respond(&mut stdout, id, json!(null));
                }
            }
        }
    }
}

fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        headers.push_str(&line);
    }
    let mut content_length = 0usize;
    for line in headers.lines() {
        if let Some(v) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("Content-Length: "))
        {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }
    let mut buf = vec![0u8; content_length];
    reader.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn respond(out: &mut impl Write, id: Option<Value>, result: Value) {
    let msg = json!({ "jsonrpc": "2.0", "id": id, "result": result });
    write_message(out, &msg);
}

fn write_message(out: &mut impl Write, msg: &Value) {
    let body = msg.to_string();
    let _ = write!(
        out,
        "Content-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = out.flush();
}

fn publish_diagnostics(out: &mut impl Write, params: &Value) {
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(|u| u.as_str())
        .unwrap_or("");
    let text = params
        .pointer("/textDocument/text")
        .or_else(|| params.pointer("/contentChanges/0/text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let diags = diagnose(uri, text);
    let msg = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": diags }
    });
    write_message(out, &msg);
}

fn diagnose(uri: &str, text: &str) -> Vec<Value> {
    if text.is_empty() {
        return vec![];
    }
    let path = uri_to_path(uri);
    let tmp = std::env::temp_dir().join(format!(
        "stk-lsp-{}.stk",
        std::process::id()
    ));
    if std::fs::write(&tmp, text).is_err() {
        return vec![];
    }
    let path = path.unwrap_or(tmp.clone());
    let _ = path;
    // Prefer parsing the temp buffer
    let diags = match stk_parser::parse(text) {
        Ok(program) => match stk_types::typecheck(&program, "lsp") {
            Ok(_) => vec![],
            Err(d) => vec![json!({
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 1 }
                },
                "severity": 1,
                "source": "steampunk",
                "message": d.message
            })],
        },
        Err(d) => vec![json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 1 }
            },
            "severity": 1,
            "source": "steampunk",
            "message": d.message
        })],
    };
    let _ = std::fs::remove_file(&tmp);
    diags
}

fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let u = uri.strip_prefix("file://")?;
    Some(PathBuf::from(u))
}
