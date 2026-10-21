//! `mcp`: Codigest's tools for AI agents, as an MCP server over newline-delimited stdio.
//!
//! The protocol surface used here (initialize, tools/list, tools/call, ping) is small
//! enough that an SDK would only add weight. Project tools run this same executable with
//! `-s --no-copy`, so an agent sees exactly what a person would paste.

use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::redact::redact;
use crate::{shellhist, term};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

fn tools() -> Value {
    let path = json!({"type": "string", "description": "Project directory (default: server cwd)"});
    let message = json!({"type": "string", "description": "Instruction to embed in the output"});
    json!([
        {
            "name": "terminal_history",
            "description": "Recent terminal commands with their full output, exit code and cwd, as captured by Codigest. Use this to see what the user just ran and why it failed, instead of asking them to paste it. If nothing was captured, falls back to the native shell history: commands only (redacted), with no output, exit code or cwd, so only when here=false and failed=false.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "count": {"type": "integer", "minimum": 1, "default": 1, "description": "How many recent commands"},
                    "failed": {"type": "boolean", "default": false, "description": "Only commands that exited non-zero"},
                    "here": {
                        "type": "boolean",
                        "default": true,
                        "description": "Only commands run in the server's working directory (the project) or below"
                    }
                }
            }
        },
        {
            "name": "pack",
            "description": "Start here when fixing a failure or reviewing work in progress: the last failed command run in the project, the files its output points at, uncommitted changes and the local modules those files import, cut to a token budget. Lists what was left out.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "message": message,
                    "targets": {"type": "array", "items": {"type": "string"}, "description": "Files/dirs to include first"},
                    "budget": {"type": "integer", "minimum": 1000, "default": 30000, "description": "Estimated token budget"},
                    "with": {
                        "type": "array",
                        "items": {"enum": ["failure", "changes", "tests", "imports"]},
                        "description": "Automatic sources to include. Default: all of them, or none when targets are given"
                    }
                }
            }
        },
        {
            "name": "scan",
            "description": "Full project snapshot: tree plus file contents as XML, filtered by the project's Codigest config.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "message": message,
                    "targets": {"type": "array", "items": {"type": "string"}, "description": "Limit to these files/dirs"},
                    "resolve": {"type": "boolean", "default": false, "description": "Follow imports of the targets"}
                }
            }
        },
        {
            "name": "diff",
            "description": "Code changes since the last `cdg mark` baseline (requires one in the project).",
            "inputSchema": {"type": "object", "properties": {"path": path, "message": message}}
        },
        {
            "name": "semdiff",
            "description": "Structural (function/class level) changes since the last `cdg mark` baseline.",
            "inputSchema": {"type": "object", "properties": {"path": path, "message": message}}
        },
        {
            "name": "digest",
            "description": "Architecture digest: signatures and docstrings without bodies.",
            "inputSchema": {"type": "object", "properties": {"path": path, "message": message}}
        },
        {
            "name": "tree",
            "description": "Project directory tree.",
            "inputSchema": {"type": "object", "properties": {"path": path}}
        }
    ])
}

fn terminal_history(args: &Value) -> anyhow::Result<String> {
    let count = args
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    let here = args.get("here").and_then(Value::as_bool).unwrap_or(true);
    let failed = args.get("failed").and_then(Value::as_bool).unwrap_or(false);
    let entries = term::load_entries();
    if entries.is_empty() {
        // Native shell history has no cwd or exit code to filter on, so it is only returned
        // when no filter was asked for.
        if here {
            return Ok("No captured commands. Native shell history has no working directory, so it may hold other projects' commands; pass here=false to read it.".into());
        }
        if failed {
            return Ok("No captured commands. Native shell history has no exit codes, so failed commands cannot be picked out; pass failed=false to read it.".into());
        }
        let (mut fallback, _) = shellhist::read_recent(count);
        for entry in &mut fallback {
            entry.command = redact(&entry.command);
        }
        return Ok(term::render(&fallback, false));
    }
    let cwd = env::current_dir()?;
    let selected = term::select(entries, failed, here.then_some(cwd.as_path()));
    let start = selected.len().saturating_sub(count);
    Ok(term::render(&selected[start..], false))
}

/// Absolute form of an agent-supplied path; refuses anything outside the server's project.
fn in_project(raw: &str) -> anyhow::Result<String> {
    let root = resolve(&env::current_dir()?);
    // Resolving follows symlinks and junctions, so a link pointing outside is caught too.
    let path = resolve(&root.join(raw));
    anyhow::ensure!(
        path.starts_with(&root),
        "{raw} is outside the project ({})",
        root.display()
    );
    Ok(path.display().to_string())
}

fn resolve(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(real) => crate::paths::strip_verbatim(real),
        Err(_) => std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
    }
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn cli(command: &str, args: &Value) -> anyhow::Result<String> {
    let mut argv: Vec<String> = vec![command.into(), "-s".into(), "--no-copy".into()];
    if command == "scan" {
        argv.push("-y".into());
    }
    if args
        .get("resolve")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        argv.push("-r".into());
    }
    if let Some(message) = args
        .get("message")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
    {
        argv.extend(["-m".into(), message.into()]);
    }
    if args.get("with").is_some_and(Value::is_array) {
        let sources = strings(args.get("with"));
        argv.extend([
            "--with".into(),
            if sources.is_empty() {
                "none".into()
            } else {
                sources.join(",")
            },
        ]);
    }
    if let Some(budget) = args
        .get("budget")
        .and_then(Value::as_i64)
        .filter(|b| *b != 0)
    {
        argv.extend(["-b".into(), budget.to_string()]);
    }
    let path = args.get("path").and_then(Value::as_str);
    match command {
        "scan" | "pack" => {
            let targets = strings(args.get("targets"));
            if targets.is_empty() {
                if let Some(path) = path {
                    argv.push(in_project(path)?);
                }
            } else {
                for target in targets {
                    argv.push(in_project(&target)?);
                }
            }
        }
        _ => {
            if let Some(path) = path {
                argv.push(in_project(path)?);
            }
        }
    }
    let result = Command::new(env::current_exe()?).args(&argv).output()?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
        let code = result.status.code().unwrap_or(1);
        anyhow::bail!(
            "{}",
            if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit {code}")
            }
        );
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

fn call_tool(name: &str, args: &Value) -> Value {
    let result = match name {
        "terminal_history" => terminal_history(args),
        "pack" | "scan" | "diff" | "semdiff" | "digest" | "tree" => cli(name, args),
        _ => Err(anyhow::anyhow!("Unknown tool: {name}")),
    };
    // Failures are tool errors the agent can read, not protocol errors.
    match result {
        Ok(text) => json!({"content": [{"type": "text", "text": text}], "isError": false}),
        Err(err) => {
            json!({"content": [{"type": "text", "text": err.to_string()}], "isError": true})
        }
    }
}

/// The JSON-RPC response for a request, or None for a notification.
pub fn handle_message(msg: &Value) -> Option<Value> {
    let id = msg.get("id").filter(|v| !v.is_null()).cloned();
    let empty = json!({});
    let params = msg
        .get("params")
        .filter(|p| p.is_object())
        .unwrap_or(&empty);
    let result = match msg.get("method").and_then(Value::as_str).unwrap_or("") {
        "initialize" => json!({
            "protocolVersion": params.get("protocolVersion").cloned().unwrap_or_else(|| json!(PROTOCOL_VERSION)),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "codigest", "version": env!("CARGO_PKG_VERSION")}
        }),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => call_tool(
            params.get("name").and_then(Value::as_str).unwrap_or(""),
            params
                .get("arguments")
                .filter(|a| a.is_object())
                .unwrap_or(&empty),
        ),
        "ping" => json!({}),
        // Notifications (initialized, cancelled, ...) need no reply.
        _ if id.is_none() => return None,
        method => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": format!("Method not found: {method}")}
            }));
        }
    };
    id.map(|id| json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// `json.dumps` layout: `", "` and `": "` separators, non-ASCII left as is.
pub fn to_python_json(value: &Value) -> String {
    struct PyFormatter;
    impl serde_json::ser::Formatter for PyFormatter {
        fn begin_array_value<W: ?Sized + Write>(
            &mut self,
            w: &mut W,
            first: bool,
        ) -> io::Result<()> {
            if first { Ok(()) } else { w.write_all(b", ") }
        }
        fn begin_object_key<W: ?Sized + Write>(
            &mut self,
            w: &mut W,
            first: bool,
        ) -> io::Result<()> {
            if first { Ok(()) } else { w.write_all(b", ") }
        }
        fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
            w.write_all(b": ")
        }
    }
    let mut out = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, PyFormatter);
    serde::Serialize::serialize(value, &mut serializer).expect("serializable");
    String::from_utf8(out).expect("utf-8")
}

pub fn run() -> anyhow::Result<i32> {
    let stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    for line in stdin.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle_message(&msg),
            Err(_) => Some(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}),
            ),
        };
        if let Some(response) = response {
            writeln!(stdout, "{}", to_python_json(&response))?;
            stdout.flush()?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_like_the_python_server() {
        let init = handle_message(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        )
        .unwrap();
        assert_eq!(
            to_python_json(&init),
            format!(
                "{{\"jsonrpc\": \"2.0\", \"id\": 1, \"result\": {{\"protocolVersion\": \"2025-06-18\", \"capabilities\": {{\"tools\": {{}}}}, \"serverInfo\": {{\"name\": \"codigest\", \"version\": \"{}\"}}}}}}",
                env!("CARGO_PKG_VERSION")
            )
        );
        assert!(
            handle_message(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
                .is_none()
        );
        let unknown =
            handle_message(&json!({"jsonrpc": "2.0", "id": 2, "method": "nope"})).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }

    #[test]
    fn unknown_tools_are_tool_errors() {
        let reply = call_tool("nope", &json!({}));
        assert_eq!(reply["isError"], true);
    }
}
