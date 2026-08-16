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

pub struct WsSessionState {
    pub active_workspace: String,
    pub terminal_cwd: PathBuf,
}

impl WsSessionState {
    pub fn new(default_ws: &str) -> Self {
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

#[derive(Debug, Deserialize)]
struct WsIncomingMessage {
    r#type: String,
    payload: Option<serde_json::Value>,
}


pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let len_bits = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&len_bits.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };

            let temp = a.rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

pub fn base64_encode(input: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn handle_ws_handshake(headers: &str) -> Option<String> {
    let mut key = None;
    for line in headers.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("Sec-WebSocket-Key: ") {
            key = Some(val.trim());
            break;
        } else if let Some(val) = line.strip_prefix("sec-websocket-key: ") {
            key = Some(val.trim());
            break;
        }
    }

    let key = key?;
    let mut combined = key.to_string();
    combined.push_str("258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let hash = sha1(combined.as_bytes());
    let accept_key = base64_encode(&hash);

    Some(format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\r\n",
        accept_key
    ))
}

pub fn handle_http_request(path: &str) -> (u16, &'static str, Vec<u8>) {
    let clean_path = path.split('?').next().unwrap_or(path).trim_start_matches('/');

    if clean_path.is_empty() || clean_path == "index.html" {
        return (200, "text/html; charset=utf-8", assets::INDEX_HTML.as_bytes().to_vec());
    }

    let lookup_path = format!("/{}", clean_path);
    if let Some((content, mime)) = assets::get_asset(&lookup_path) {
        return (200, mime, content.as_bytes().to_vec());
    }

    if let Some(rest) = clean_path.strip_prefix("styles/") {
        let lookup_style = format!("/styles/{}", rest);
        if let Some((content, mime)) = assets::get_asset(&lookup_style) {
            return (200, mime, content.as_bytes().to_vec());
        }
    }

    (404, "text/plain", b"404 Not Found".to_vec())
}

pub async fn process_ws_message_async(
    text: &str,
    session: &mut WsSessionState,
    state: &SharedState,
) -> Option<String> {
    let incoming = serde_json::from_str::<WsIncomingMessage>(text).ok()?;
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
            Some(resp.to_string())
        }

        "workspace:create" => {
            let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let build = payload.get("build").and_then(|v| v.as_str()).unwrap_or("");
            let ls_type = payload.get("lsType").or_else(|| payload.get("ls_type")).and_then(|v| v.as_bool()).unwrap_or(true);

            if let Ok(config) = workspace::create_workspace(name, build, ls_type) {
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
                Some(resp.to_string())
            } else {
                None
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
            Some(resp.to_string())
        }

        "workspace:delete" => {
            let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let _ = workspace::delete_workspace(name);
            let resp = serde_json::json!({
                "type": "workspace:delete:response"
            });
            Some(resp.to_string())
        }

        "fs:list" => {
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);
            let tree = filesystem::build_recursive_tree(&base_dir, "");

            let resp = serde_json::json!({
                "type": "fs:list:response",
                "tree": tree
            });
            Some(resp.to_string())
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
            Some(resp.to_string())
        }

        "fs:write" => {
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);

            let _ = filesystem::write_file_content(&base_dir, path, content);
            let resp = serde_json::json!({ "type": "fs:write:response" });
            Some(resp.to_string())
        }

        "fs:createDir" => {
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);

            let _ = filesystem::create_directory(&base_dir, path);
            let resp = serde_json::json!({ "type": "fs:write:response" });
            Some(resp.to_string())
        }

        "fs:rename" => {
            let old_path = payload.get("oldPath").and_then(|v| v.as_str()).unwrap_or("");
            let new_name = payload.get("newName").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);

            let _ = filesystem::rename_path(&base_dir, old_path, new_name);
            let resp = serde_json::json!({ "type": "fs:write:response" });
            Some(resp.to_string())
        }

        "fs:delete" => {
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);

            let _ = filesystem::delete_path(&base_dir, path);
            let resp = serde_json::json!({ "type": "fs:write:response" });
            Some(resp.to_string())
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
            Some(resp.to_string())
        }

        "terminal:init" => {
            let clean_cwd = clean_path_str(&session.terminal_cwd);
            let prompt_str = format!("{}>", clean_cwd);
            let resp = serde_json::json!({
                "type": "terminal:init:response",
                "banner": "",
                "prompt": prompt_str
            });
            Some(resp.to_string())
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
            Some(resp.to_string())
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
            Some(resp.to_string())
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
                    return Some(resp.to_string());
                }
            }

            let lang = payload.get("language").or_else(|| payload.get("ext")).and_then(|v| v.as_str()).unwrap_or("rs");
            let word = payload.get("word").or_else(|| payload.get("query")).and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            
            #[cfg(feature = "threading")]
            let lsp_suggestions = if let Some(port) = lsp::get_lsp_port_for_ext(lang) {
                lsp::query_lsp_server(port, &word, lang).await
            } else {
                None
            };

            #[cfg(not(feature = "threading"))]
            let lsp_suggestions = if let Some(port) = lsp::get_lsp_port_for_ext(lang) {
                lsp::query_lsp_server(port, &word, lang)
            } else {
                None
            };

            let filtered: Vec<serde_json::Value> = if let Some(items) = lsp_suggestions {
                items
            } else {
                get_fallback_completions(lang, &word)
            };

            let resp = serde_json::json!({
                "type": "lsp:completion:response",
                "suggestions": filtered
            });
            Some(resp.to_string())
        }

        _ => None,
    }
}

pub fn process_ws_message_sync(
    text: &str,
    session: &mut WsSessionState,
    state: &SharedState,
) -> Option<String> {
    let incoming = serde_json::from_str::<WsIncomingMessage>(text).ok()?;
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
            Some(resp.to_string())
        }
        "workspace:create" => {
            let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let build = payload.get("build").and_then(|v| v.as_str()).unwrap_or("");
            let ls_type = payload.get("lsType").or_else(|| payload.get("ls_type")).and_then(|v| v.as_bool()).unwrap_or(true);
            if let Ok(config) = workspace::create_workspace(name, build, ls_type) {
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
                Some(resp.to_string())
            } else {
                None
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
            Some(resp.to_string())
        }
        "workspace:delete" => {
            let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let _ = workspace::delete_workspace(name);
            Some(serde_json::json!({ "type": "workspace:delete:response" }).to_string())
        }
        "fs:list" => {
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);
            let tree = filesystem::build_recursive_tree(&base_dir, "");
            Some(serde_json::json!({ "type": "fs:list:response", "tree": tree }).to_string())
        }
        "fs:read" => {
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);
            let content = filesystem::read_file_content(&base_dir, path).unwrap_or_default();
            Some(serde_json::json!({ "type": "fs:read:response", "path": path, "content": content }).to_string())
        }
        "fs:write" => {
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);
            let _ = filesystem::write_file_content(&base_dir, path, content);
            Some(serde_json::json!({ "type": "fs:write:response" }).to_string())
        }
        "fs:createDir" => {
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);
            let _ = filesystem::create_directory(&base_dir, path);
            Some(serde_json::json!({ "type": "fs:write:response" }).to_string())
        }
        "fs:rename" => {
            let old_path = payload.get("oldPath").and_then(|v| v.as_str()).unwrap_or("");
            let new_name = payload.get("newName").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);
            let _ = filesystem::rename_path(&base_dir, old_path, new_name);
            Some(serde_json::json!({ "type": "fs:write:response" }).to_string())
        }
        "fs:delete" => {
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let active = state.lock().unwrap().active_workspace.clone();
            let base_dir = PathBuf::from("projects").join(&active);
            let _ = filesystem::delete_path(&base_dir, path);
            Some(serde_json::json!({ "type": "fs:write:response" }).to_string())
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
            Some(serde_json::json!({ "type": "fs:search:response", "results": search_res.results }).to_string())
        }
        "terminal:init" => {
            let clean_cwd = clean_path_str(&session.terminal_cwd);
            let prompt_str = format!("{}>", clean_cwd);
            Some(serde_json::json!({ "type": "terminal:init:response", "banner": "", "prompt": prompt_str }).to_string())
        }
        "terminal:input" => {
            let cmd_raw = payload.get("command").or_else(|| payload.get("input")).and_then(|v| v.as_str()).unwrap_or("");
            let cmd_str = cmd_raw.trim();
            let (stdout, stderr) = if !cmd_str.is_empty() {
                let mut path_env = std::env::var("PATH").unwrap_or_default();
                if let Ok(user_profile) = std::env::var("USERPROFILE") {
                    path_env = format!("{}\\.cargo\\bin;{}", user_profile, path_env);
                }
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
            Some(serde_json::json!({
                "type": "terminal:data",
                "cmd": cmd_str,
                "stdout": stdout,
                "stderr": stderr,
                "prompt": prompt_str
            }).to_string())
        }
        "build:run" | "workspace:build" => {
            let active = state.lock().unwrap().active_workspace.clone();
            let proj_dir = PathBuf::from("projects").join(&active);
            let config = workspace::ProjectConfig::load_from_dir(&proj_dir);
            let build_cmd = config.map(|c| c.build).unwrap_or_else(|| "cargo check".to_string());
            let target_dir = if proj_dir.exists() { &proj_dir } else { &session.terminal_cwd };
            let (stdout, stderr) = terminal::execute_shell_command(target_dir, &build_cmd)
                .unwrap_or_else(|e| (String::new(), format!("Build execution error: {}", e)));
            Some(serde_json::json!({
                "type": "build:response",
                "cmd": build_cmd,
                "stdout": stdout,
                "stderr": stderr
            }).to_string())
        }
        "lsp:completion" | "lsp:complete" => {
            let active = state.lock().unwrap().active_workspace.clone();
            let proj_dir = PathBuf::from("projects").join(&active);
            let config = workspace::ProjectConfig::load_from_dir(&proj_dir);
            if let Some(cfg) = config {
                if !cfg.ls_type {
                    return Some(serde_json::json!({ "type": "lsp:completion:response", "suggestions": [] }).to_string());
                }
            }
            let lang = payload.get("language").or_else(|| payload.get("ext")).and_then(|v| v.as_str()).unwrap_or("rs");
            let word = payload.get("word").or_else(|| payload.get("query")).and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            
            #[cfg(feature = "threading")]
            let lsp_suggestions = None;

            #[cfg(not(feature = "threading"))]
            let lsp_suggestions = if let Some(port) = lsp::get_lsp_port_for_ext(lang) {
                lsp::query_lsp_server(port, &word, lang)
            } else {
                None
            };

            let filtered: Vec<serde_json::Value> = if let Some(items) = lsp_suggestions {
                items
            } else {
                get_fallback_completions(lang, &word)
            };
            Some(serde_json::json!({
                "type": "lsp:completion:response",
                "suggestions": filtered
            }).to_string())
        }
        _ => None,
    }
}

fn get_fallback_completions(lang: &str, word: &str) -> Vec<serde_json::Value> {
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
        .filter(|(label, _)| word.is_empty() || label.to_lowercase().starts_with(word))
        .map(|(label, kind)| serde_json::json!({ "label": label, "kind": kind }))
        .collect();

    if prefix_matches.is_empty() && !word.is_empty() {
        prefix_matches = all_items
            .iter()
            .filter(|(label, _)| label.to_lowercase().contains(word))
            .map(|(label, kind)| serde_json::json!({ "label": label, "kind": kind }))
            .collect();
    }

    prefix_matches
}
