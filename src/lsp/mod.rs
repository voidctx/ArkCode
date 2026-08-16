// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use serde_json::Value;
use std::time::Duration;

pub fn get_lsp_port_for_ext(ext: &str) -> Option<u16> {
    let config: Value = serde_json::from_str(crate::workspace::CONFIG_JSON_STR).ok()?;

    let mut matched_lang = ext;
    if let Some(lang_exts) = config.get("language_extensions") {
        if let Some(obj) = lang_exts.as_object() {
            for (lang, exts_val) in obj {
                if let Some(arr) = exts_val.as_array() {
                    if arr.iter().any(|e| e.as_str() == Some(ext)) {
                        matched_lang = lang.as_str();
                        break;
                    }
                }
            }
        }
    }

    let lsp_map = config.get("lsp")?;
    let port = lsp_map.get(matched_lang).or_else(|| lsp_map.get(ext))?.as_u64()?;
    Some(port as u16)
}

#[cfg(feature = "threading")]
pub async fn query_lsp_server(port: u16, word: &str, lang: &str) -> Option<Vec<Value>> {
    use crate::threading::io::{AsyncReadExt, AsyncWriteExt};
    use crate::threading::net::TcpStream;
    use crate::threading::time::timeout;

    let stream_res = timeout(Duration::from_millis(200), TcpStream::connect(("127.0.0.1", port))).await;
    let mut stream = stream_res.ok()?.ok()?;

    let json_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "textDocument/completion",
        "params": {
            "language": lang,
            "query": word
        }
    });

    let payload = json_req.to_string();
    let http_req = format!(
        "POST /lsp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        port, payload.len(), payload
    );

    if stream.write_all(http_req.as_bytes()).await.is_err() {
        return None;
    }

    let mut response_bytes = Vec::new();
    let _ = timeout(Duration::from_millis(300), stream.read_to_end(&mut response_bytes)).await;

    let response_str = String::from_utf8_lossy(&response_bytes);
    let json_body = response_str.split("\r\n\r\n").nth(1)?;

    let val: Value = serde_json::from_str(json_body).ok()?;
    let items = val.get("items")
        .or_else(|| val.get("suggestions"))
        .or_else(|| val.get("result"))
        .and_then(|v| v.as_array())?;

    Some(items.clone())
}

#[cfg(not(feature = "threading"))]
pub fn query_lsp_server(port: u16, word: &str, lang: &str) -> Option<Vec<Value>> {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(200)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));

    let json_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "textDocument/completion",
        "params": {
            "language": lang,
            "query": word
        }
    });

    let payload = json_req.to_string();
    let http_req = format!(
        "POST /lsp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        port, payload.len(), payload
    );

    if stream.write_all(http_req.as_bytes()).is_err() {
        return None;
    }

    let mut response_bytes = Vec::new();
    let _ = stream.read_to_end(&mut response_bytes);

    let response_str = String::from_utf8_lossy(&response_bytes);
    let json_body = response_str.split("\r\n\r\n").nth(1)?;

    let val: Value = serde_json::from_str(json_body).ok()?;
    let items = val.get("items")
        .or_else(|| val.get("suggestions"))
        .or_else(|| val.get("result"))
        .and_then(|v| v.as_array())?;

    Some(items.clone())
}
