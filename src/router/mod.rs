// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use crate::assets;
use crate::filesystem;
use crate::lsp;
use crate::search::{self, SearchRequest};
use crate::terminal;
use crate::workspace;

use axum::{
    extract::{ws::WebSocket, ws::WebSocketUpgrade},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct AppState {
    pub active_workspace: String,
}

pub type SharedState = Arc<Mutex<AppState>>;

fn clean_path_str<P: AsRef<Path>>(path: P) -> String {
    let s = path.as_ref().to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

struct WsSessionState {
    active_workspace: String,
    terminal_cwd: PathBuf,
}

impl WsSessionState {
    fn new(default_ws: &str) -> Self {
        let initial_cwd = if !default_ws.is_empty() {
            let p = PathBuf::from("projects").join(default_ws);
            if p.exists() {
                std::fs::canonicalize(&p).unwrap_or(p)
            } else {
                std::env::current_dir().unwrap_or_default()
            }
        } else {
            std::env::current_dir().unwrap_or_default()
        };

        WsSessionState {
            active_workspace: default_ws.to_string(),
            terminal_cwd: initial_cwd,
        }
    }
}

async fn serve_asset(axum::extract::Path(path): axum::extract::Path<String>) -> impl IntoResponse {
    let full_path = format!("/{}", path);
    if let Some((content, mime)) = assets::get_asset(&full_path) {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime)],
            content.to_string(),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain")],
            "404 Not Found".to_string(),
        )
    }
}

async fn serve_style_asset(axum::extract::Path(path): axum::extract::Path<String>) -> impl IntoResponse {
    let full_path = format!("/styles/{}", path);
    if let Some((content, mime)) = assets::get_asset(&full_path) {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime)],
            content.to_string(),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain")],
            "404 Not Found".to_string(),
        )
    }
}

#[derive(Debug, Deserialize)]
struct WsIncomingMessage {
    r#type: String,
    payload: Option<serde_json::Value>,
}

async fn handle_ws_session(socket: WebSocket, state: SharedState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let initial_workspace = state.lock().unwrap().active_workspace.clone();
    let mut session = WsSessionState::new(&initial_workspace);

    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let axum::extract::ws::Message::Text(text) = msg {
            if let Ok(incoming) = serde_json::from_str::<WsIncomingMessage>(&text) {
                let msg_type = incoming.r#type.as_str();
                let payload = incoming.payload.unwrap_or(serde_json::json!({}));

                match msg_type {
                    "workspace:list" => {
                        let list = workspace::scan_workspaces();
                        let ws_names: Vec<String> = list.iter().map(|w| w.name.clone()).collect();
                        
                        let active = {
                            let mut lock = state.lock().unwrap();
                            if lock.active_workspace.is_empty() && !ws_names.is_empty() {
                                lock.active_workspace = ws_names[0].clone();
                            }
                            lock.active_workspace.clone()
                        };

                        session.active_workspace = active.clone();
                        let proj_p = PathBuf::from("projects").join(&active);
                        if proj_p.exists() {
                            session.terminal_cwd = std::fs::canonicalize(&proj_p).unwrap_or(proj_p);
                        }

                        let resp = serde_json::json!({
                            "type": "workspace:list:response",
                            "workspaces": ws_names,
                            "current": active
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "workspace:create" => {
                        let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let desc = payload.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        let ls_type = payload.get("lsType").or_else(|| payload.get("ls_type")).and_then(|v| v.as_bool()).unwrap_or(true);

                        if let Ok(config) = workspace::create_workspace(name, desc, ls_type) {
                            {
                                let mut lock = state.lock().unwrap();
                                lock.active_workspace = config.name.clone();
                            }
                            session.active_workspace = config.name.clone();
                            let proj_p = PathBuf::from("projects").join(&config.name);
                            if proj_p.exists() {
                                session.terminal_cwd = std::fs::canonicalize(&proj_p).unwrap_or(proj_p);
                            }

                            let resp = serde_json::json!({
                                "type": "workspace:create:response",
                                "config": config
                            });
                            let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                        }
                    }

                    "workspace:switch" | "workspace:select" => {
                        let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        {
                            let mut lock = state.lock().unwrap();
                            lock.active_workspace = name.to_string();
                        }
                        session.active_workspace = name.to_string();
                        let proj_dir = PathBuf::from("projects").join(name);
                        if proj_dir.exists() {
                            session.terminal_cwd = std::fs::canonicalize(&proj_dir).unwrap_or(proj_dir.clone());
                        }

                        let config = workspace::ProjectConfig::load_from_dir(&proj_dir);
                        let resp = serde_json::json!({
                            "type": "workspace:switch:response",
                            "config": config
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "workspace:delete" => {
                        let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let _ = workspace::delete_workspace(name);
                        let resp = serde_json::json!({
                            "type": "workspace:delete:response"
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "fs:list" => {
                        let active = state.lock().unwrap().active_workspace.clone();
                        let base_dir = PathBuf::from("projects").join(&active);
                        let tree = filesystem::build_recursive_tree(&base_dir, "");

                        let resp = serde_json::json!({
                            "type": "fs:list:response",
                            "tree": tree
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "fs:read" => {
                        let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        let active = state.lock().unwrap().active_workspace.clone();
                        let base_dir = PathBuf::from("projects").join(&active);

                        let content = filesystem::read_file_content(&base_dir, path).unwrap_or_default();
                        let resp = serde_json::json!({
                            "type": "fs:read:response",
                            "path": path,
                            "content": content
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "fs:write" => {
                        let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        let active = state.lock().unwrap().active_workspace.clone();
                        let base_dir = PathBuf::from("projects").join(&active);

                        let _ = filesystem::write_file_content(&base_dir, path, content);
                        let resp = serde_json::json!({ "type": "fs:write:response" });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "fs:createDir" => {
                        let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        let active = state.lock().unwrap().active_workspace.clone();
                        let base_dir = PathBuf::from("projects").join(&active);

                        let _ = filesystem::create_directory(&base_dir, path);
                        let resp = serde_json::json!({ "type": "fs:write:response" });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "fs:rename" => {
                        let old_path = payload.get("oldPath").and_then(|v| v.as_str()).unwrap_or("");
                        let new_name = payload.get("newName").and_then(|v| v.as_str()).unwrap_or("");
                        let active = state.lock().unwrap().active_workspace.clone();
                        let base_dir = PathBuf::from("projects").join(&active);

                        let _ = filesystem::rename_path(&base_dir, old_path, new_name);
                        let resp = serde_json::json!({ "type": "fs:write:response" });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "fs:delete" => {
                        let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        let active = state.lock().unwrap().active_workspace.clone();
                        let base_dir = PathBuf::from("projects").join(&active);

                        let _ = filesystem::delete_path(&base_dir, path);
                        let resp = serde_json::json!({ "type": "fs:write:response" });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "fs:search" | "search" => {
                        let query = payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        let active = state.lock().unwrap().active_workspace.clone();
                        let base_dir = PathBuf::from("projects").join(&active);

                        let req = SearchRequest {
                            workspace: base_dir.to_string_lossy().to_string(),
                            query: query.to_string(),
                            case_sensitive: false,
                            whole_word: false,
                            is_regex: false,
                        };

                        let search_res = search::search_workspace(&req).unwrap_or_else(|_| search::SearchResponse { results: vec![] });
                        let resp = serde_json::json!({
                            "type": "fs:search:response",
                            "results": search_res.results
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "terminal:init" => {
                        let clean_cwd = clean_path_str(&session.terminal_cwd);
                        let prompt_str = format!("{}>", clean_cwd);
                        let resp = serde_json::json!({
                            "type": "terminal:init:response",
                            "banner": "",
                            "prompt": prompt_str
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "terminal:input" => {
                        let cmd_raw = payload.get("command")
                            .or_else(|| payload.get("input"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let cmd_str = cmd_raw.trim();

                        let (stdout, stderr) = if !cmd_str.is_empty() {
                            let mut path_env = std::env::var("PATH").unwrap_or_default();
                            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                                path_env = format!("{}\\.cargo\\bin;{}", user_profile, path_env);
                            }

                            // Pass command directly to system shell, appending `& cd` (on Windows) or `; pwd` (on Unix) to track resulting CWD
                            let shell_cmd = if cfg!(windows) {
                                format!("{} & cd", cmd_str)
                            } else {
                                format!("{} ; pwd", cmd_str)
                            };

                            let out = if cfg!(windows) {
                                std::process::Command::new("cmd")
                                    .args(["/C", &shell_cmd])
                                    .env("PATH", &path_env)
                                    .current_dir(&session.terminal_cwd)
                                    .output()
                            } else {
                                std::process::Command::new("sh")
                                    .args(["-c", &shell_cmd])
                                    .env("PATH", &path_env)
                                    .current_dir(&session.terminal_cwd)
                                    .output()
                            };

                            match out {
                                Ok(o) => {
                                    let mut out_str = String::from_utf8_lossy(&o.stdout).to_string();
                                    let err_str = String::from_utf8_lossy(&o.stderr).to_string();
                                    let lines: Vec<&str> = out_str.lines().collect();
                                    if let Some(last_line) = lines.last() {
                                        let candidate_p = PathBuf::from(last_line.trim());
                                        if candidate_p.exists() && candidate_p.is_dir() {
                                            session.terminal_cwd = candidate_p;
                                            if lines.len() > 1 {
                                                out_str = lines[..lines.len() - 1].join("\n");
                                                if !out_str.is_empty() {
                                                    out_str.push('\n');
                                                }
                                            } else {
                                                out_str.clear();
                                            }
                                        }
                                    }

                                    (out_str, err_str)
                                }
                                Err(e) => (String::new(), format!("Command execution error: {}", e)),
                            }
                        } else {
                            (String::new(), String::new())
                        };

                        let clean_cwd = clean_path_str(&session.terminal_cwd);
                        let prompt_str = format!("{}>", clean_cwd);
                        let resp = serde_json::json!({
                            "type": "terminal:data",
                            "cmd": cmd_str,
                            "stdout": stdout,
                            "stderr": stderr,
                            "prompt": prompt_str
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "build:run" | "workspace:build" => {
                        let active = state.lock().unwrap().active_workspace.clone();
                        let proj_dir = PathBuf::from("projects").join(&active);
                        let config = workspace::ProjectConfig::load_from_dir(&proj_dir);
                        let build_cmd = config.map(|c| c.build).unwrap_or_else(|| "cargo check".to_string());

                        let target_dir = if proj_dir.exists() { &proj_dir } else { &session.terminal_cwd };
                        let (stdout, stderr) = terminal::execute_shell_command(target_dir, &build_cmd)
                            .unwrap_or_else(|e| (String::new(), format!("Build execution error: {}", e)));

                        let resp = serde_json::json!({
                            "type": "build:response",
                            "cmd": build_cmd,
                            "stdout": stdout,
                            "stderr": stderr
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    "lsp:completion" | "lsp:complete" => {
                        let active = state.lock().unwrap().active_workspace.clone();
                        let proj_dir = PathBuf::from("projects").join(&active);
                        let config = workspace::ProjectConfig::load_from_dir(&proj_dir);
                        if let Some(cfg) = config {
                            if !cfg.ls_type {
                                let resp = serde_json::json!({
                                    "type": "lsp:completion:response",
                                    "suggestions": []
                                });
                                let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                                continue;
                            }
                        }

                        let lang = payload.get("language").or_else(|| payload.get("ext")).and_then(|v| v.as_str()).unwrap_or("rs");
                        let word = payload.get("word").or_else(|| payload.get("query")).and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                        let lsp_suggestions = if let Some(port) = lsp::get_lsp_port_for_ext(lang) {
                            lsp::query_lsp_server(port, &word, lang).await
                        } else {
                            None
                        };

                        let filtered: Vec<serde_json::Value> = if let Some(items) = lsp_suggestions {
                            items
                        } else {
                            let all_items: Vec<(&str, &str)> = match lang {
                                "rs" | "rust" => vec![
                                    ("println!()", "macro"),
                                    ("print!()", "macro"),
                                    ("eprintln!()", "macro"),
                                    ("eprint!()", "macro"),
                                    ("format!()", "macro"),
                                    ("vec![]", "macro"),
                                    ("panic!()", "macro"),
                                    ("assert!()", "macro"),
                                    ("assert_eq!()", "macro"),
                                    ("todo!()", "macro"),
                                    ("unimplemented!()", "macro"),
                                    ("dbg!()", "macro"),
                                    ("concat!()", "macro"),
                                    ("env!()", "macro"),
                                    ("include_str!()", "macro"),
                                    ("main()", "function"),
                                    ("String::new()", "method"),
                                    ("String::from()", "method"),
                                    ("Vec::new()", "method"),
                                    ("Vec::with_capacity()", "method"),
                                    ("Option::Some()", "variant"),
                                    ("Option::None()", "variant"),
                                    ("Result::Ok()", "variant"),
                                    ("Result::Err()", "variant"),
                                    ("unwrap()", "method"),
                                    ("expect()", "method"),
                                    ("clone()", "method"),
                                    ("collect()", "method"),
                                    ("map()", "method"),
                                    ("filter()", "method"),
                                    ("push()", "method"),
                                    ("pop()", "method"),
                                    ("as_bytes()", "method"),
                                    ("to_string()", "method"),
                                    ("as_str()", "method"),
                                    ("trim()", "method"),
                                    ("lines()", "method"),
                                    ("split()", "method"),
                                    ("len()", "method"),
                                    ("is_empty()", "method"),
                                    ("struct", "keyword"),
                                    ("enum", "keyword"),
                                    ("impl", "keyword"),
                                    ("trait", "keyword"),
                                    ("fn", "keyword"),
                                    ("pub", "keyword"),
                                    ("mut", "keyword"),
                                    ("let", "keyword"),
                                    ("const", "keyword"),
                                    ("static", "keyword"),
                                    ("match", "keyword"),
                                    ("if let", "snippet"),
                                    ("while let", "snippet"),
                                    ("loop", "keyword"),
                                    ("for", "keyword"),
                                    ("while", "keyword"),
                                    ("break", "keyword"),
                                    ("continue", "keyword"),
                                    ("return", "keyword"),
                                    ("use", "keyword"),
                                    ("mod", "keyword"),
                                    ("extern", "keyword"),
                                    ("unsafe", "keyword"),
                                    ("async", "keyword"),
                                    ("await", "keyword"),
                                    ("String", "type"),
                                    ("str", "type"),
                                    ("Vec", "type"),
                                    ("Option", "type"),
                                    ("Result", "type"),
                                    ("Box", "type"),
                                    ("Arc", "type"),
                                    ("Rc", "type"),
                                    ("Mutex", "type"),
                                    ("RefCell", "type"),
                                    ("HashMap", "type"),
                                    ("BTreeMap", "type"),
                                    ("usize", "type"),
                                    ("u8", "type"),
                                    ("u16", "type"),
                                    ("u32", "type"),
                                    ("u64", "type"),
                                    ("u128", "type"),
                                    ("isize", "type"),
                                    ("i8", "type"),
                                    ("i16", "type"),
                                    ("i32", "type"),
                                    ("i64", "type"),
                                    ("i128", "type"),
                                    ("f32", "type"),
                                    ("f64", "type"),
                                    ("bool", "type"),
                                    ("char", "type"),
                                ],
                                "js" | "ts" | "javascript" | "typescript" => vec![
                                    ("console.log()", "function"),
                                    ("console.error()", "function"),
                                    ("console.warn()", "function"),
                                    ("document.getElementById()", "function"),
                                    ("document.querySelector()", "function"),
                                    ("document.querySelectorAll()", "function"),
                                    ("document.createElement()", "function"),
                                    ("addEventListener()", "method"),
                                    ("removeEventListener()", "method"),
                                    ("setTimeout()", "function"),
                                    ("setInterval()", "function"),
                                    ("clearTimeout()", "function"),
                                    ("clearInterval()", "function"),
                                    ("fetch()", "function"),
                                    ("JSON.stringify()", "method"),
                                    ("JSON.parse()", "method"),
                                    ("Object.keys()", "method"),
                                    ("Object.values()", "method"),
                                    ("Object.entries()", "method"),
                                    ("Object.assign()", "method"),
                                    ("Array.from()", "method"),
                                    ("Array.isArray()", "method"),
                                    ("Math.floor()", "method"),
                                    ("Math.ceil()", "method"),
                                    ("Math.round()", "method"),
                                    ("Math.random()", "method"),
                                    ("Math.max()", "method"),
                                    ("Math.min()", "method"),
                                    ("function", "keyword"),
                                    ("const", "keyword"),
                                    ("let", "keyword"),
                                    ("var", "keyword"),
                                    ("return", "keyword"),
                                    ("async", "keyword"),
                                    ("await", "keyword"),
                                    ("Promise", "class"),
                                    ("import", "keyword"),
                                    ("export", "keyword"),
                                ],
                                "py" | "python" => vec![
                                    ("print()", "function"),
                                    ("len()", "function"),
                                    ("range()", "function"),
                                    ("str()", "function"),
                                    ("int()", "function"),
                                    ("float()", "function"),
                                    ("list()", "function"),
                                    ("dict()", "function"),
                                    ("set()", "function"),
                                    ("tuple()", "function"),
                                    ("isinstance()", "function"),
                                    ("def", "keyword"),
                                    ("class", "keyword"),
                                    ("import", "keyword"),
                                    ("from", "keyword"),
                                    ("return", "keyword"),
                                    ("self", "keyword"),
                                    ("__init__()", "method"),
                                ],
                                "c" | "cpp" => vec![
                                    ("printf()", "function"),
                                    ("scanf()", "function"),
                                    ("main()", "function"),
                                    ("#include", "directive"),
                                    ("std::cout", "object"),
                                    ("std::cin", "object"),
                                    ("std::vector", "class"),
                                    ("std::string", "class"),
                                    ("struct", "keyword"),
                                    ("class", "keyword"),
                                    ("void", "keyword"),
                                    ("int", "keyword"),
                                ],
                                _ => vec![
                                    ("function", "keyword"),
                                    ("return", "keyword"),
                                    ("class", "keyword"),
                                    ("const", "keyword"),
                                ],
                            };

                            let mut prefix_matches: Vec<serde_json::Value> = all_items
                                .iter()
                                .filter(|(label, _)| word.is_empty() || label.to_lowercase().starts_with(&word))
                                .map(|(label, kind)| serde_json::json!({ "label": label, "kind": kind }))
                                .collect();

                            if prefix_matches.is_empty() && !word.is_empty() {
                                prefix_matches = all_items
                                    .iter()
                                    .filter(|(label, _)| label.to_lowercase().contains(&word))
                                    .map(|(label, kind)| serde_json::json!({ "label": label, "kind": kind }))
                                    .collect();
                            }

                            prefix_matches
                        };

                        let resp = serde_json::json!({
                            "type": "lsp:completion:response",
                            "suggestions": filtered
                        });
                        let _ = ws_sender.send(axum::extract::ws::Message::Text(resp.to_string())).await;
                    }

                    _ => {}
                }
            }
        }
    }
}

async fn serve_index_or_ws(
    ws_opt: Option<WebSocketUpgrade>,
    axum::extract::State(state): axum::extract::State<SharedState>,
) -> Response {
    if let Some(ws) = ws_opt {
        ws.on_upgrade(move |socket| handle_ws_session(socket, state))
            .into_response()
    } else {
        Html(assets::INDEX_HTML).into_response()
    }
}

async fn ws_handler(ws: WebSocketUpgrade, axum::extract::State(state): axum::extract::State<SharedState>) -> Response {
    ws.on_upgrade(move |socket| handle_ws_session(socket, state))
}

pub fn create_router() -> Router {
    let state: SharedState = Arc::new(Mutex::new(AppState::default()));

    Router::new()
        .route("/", get(serve_index_or_ws))
        .route("/app.js", get(|| async { serve_asset(axum::extract::Path("app.js".to_string())).await }))
        .route("/styles/:file", get(serve_style_asset))
        .route("/ws", get(ws_handler))
        .with_state(state)
}
