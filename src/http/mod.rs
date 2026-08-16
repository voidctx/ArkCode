// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use crate::router::{self, AppState, SharedState, WsSessionState};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

fn encode_ws_text_frame(text: &str) -> Vec<u8> {
    let payload = text.as_bytes();
    let len = payload.len();
    let mut frame = Vec::with_capacity(len + 10);
    frame.push(0x81); // FIN + Text Opcode

    if len < 126 {
        frame.push(len as u8);
    } else if len < 65536 {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    frame.extend_from_slice(payload);
    frame
}

#[cfg(feature = "threading")]
pub async fn start_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    use crate::threading::io::{AsyncReadExt, AsyncWriteExt};
    use crate::threading::net::TcpListener;

    let state: SharedState = Arc::new(Mutex::new(AppState::default()));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    println!("ArkCode WebIDE running at http://localhost:{}", port);

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
                continue;
            }
        };

        let state = Arc::clone(&state);
        crate::threading::task::spawn(async move {
            let mut req_buf = vec![0u8; 4096];
            let n = match stream.read(&mut req_buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let req_str = String::from_utf8_lossy(&req_buf[..n]);
            let mut lines = req_str.lines();
            let first_line = match lines.next() {
                Some(l) => l,
                None => return,
            };

            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() < 2 {
                return;
            }

            let method = parts[0];
            let path = parts[1];
            if req_str.to_lowercase().contains("upgrade: websocket") {
                if let Some(handshake_resp) = router::handle_ws_handshake(&req_str) {
                    if stream.write_all(handshake_resp.as_bytes()).await.is_err() {
                        return;
                    }
                    let initial_workspace = state.lock().unwrap().active_workspace.clone();
                    let mut session = WsSessionState::new(&initial_workspace);
                    let mut pending = Vec::new();
                    let mut read_buf = [0u8; 8192];

                    loop {
                        let bytes_read = match stream.read(&mut read_buf).await {
                            Ok(n) if n > 0 => n,
                            _ => break,
                        };
                        pending.extend_from_slice(&read_buf[..bytes_read]);

                        while pending.len() >= 2 {
                            let b0 = pending[0];
                            let b1 = pending[1];
                            let opcode = b0 & 0x0f;
                            let masked = (b1 & 0x80) != 0;
                            let mut payload_len = (b1 & 0x7f) as usize;
                            let mut header_len = 2;

                            if payload_len == 126 {
                                if pending.len() < 4 {
                                    break;
                                }
                                payload_len = u16::from_be_bytes([pending[2], pending[3]]) as usize;
                                header_len += 2;
                            } else if payload_len == 127 {
                                if pending.len() < 10 {
                                    break;
                                }
                                payload_len = u64::from_be_bytes(pending[2..10].try_into().unwrap()) as usize;
                                header_len += 8;
                            }

                            if masked {
                                header_len += 4;
                            }

                            let frame_len = header_len + payload_len;
                            if pending.len() < frame_len {
                                break;
                            }

                            if opcode == 0x8 {
                                return;
                            }

                            let mut payload = pending[header_len..frame_len].to_vec();
                            if masked {
                                let mask = &pending[header_len - 4..header_len];
                                for (i, byte) in payload.iter_mut().enumerate() {
                                    *byte ^= mask[i % 4];
                                }
                            }

                            pending.drain(..frame_len);

                            if opcode == 0x1 {
                                if let Ok(text) = String::from_utf8(payload) {
                                    if let Some(resp) = router::process_ws_message_async(&text, &mut session, &state).await {
                                        let frame = encode_ws_text_frame(&resp);
                                        if stream.write_all(&frame).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
            }

            if method == "GET" {
                let (status, mime, body) = router::handle_http_request(path);
                let header = format!(
                    "HTTP/1.1 {} OK\r\n\
                     Content-Type: {}\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n",
                    status, mime, body.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(&body).await;
            } else {
                let resp = "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes()).await;
            }
        });
    }
}

#[cfg(not(feature = "threading"))]
pub fn start_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let state: SharedState = Arc::new(Mutex::new(AppState::default()));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr)?;
    println!("ArkCode WebIDE running (single-threaded) at http://localhost:{}", port);

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
                continue;
            }
        };

        let mut req_buf = [0u8; 4096];
        let n = match stream.read(&mut req_buf) {
            Ok(n) if n > 0 => n,
            _ => continue,
        };

        let req_str = String::from_utf8_lossy(&req_buf[..n]);
        let mut lines = req_str.lines();
        let first_line = match lines.next() {
            Some(l) => l,
            None => continue,
        };

        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let method = parts[0];
        let path = parts[1];

        if req_str.to_lowercase().contains("upgrade: websocket") {
            if let Some(handshake_resp) = router::handle_ws_handshake(&req_str) {
                if stream.write_all(handshake_resp.as_bytes()).is_err() {
                    continue;
                }

                let initial_workspace = state.lock().unwrap().active_workspace.clone();
                let mut session = WsSessionState::new(&initial_workspace);

                let mut pending = Vec::new();
                let mut read_buf = [0u8; 8192];

                loop {
                    let bytes_read = match stream.read(&mut read_buf) {
                        Ok(n) if n > 0 => n,
                        _ => break,
                    };
                    pending.extend_from_slice(&read_buf[..bytes_read]);

                    while pending.len() >= 2 {
                        let b0 = pending[0];
                        let b1 = pending[1];
                        let opcode = b0 & 0x0f;
                        let masked = (b1 & 0x80) != 0;
                        let mut payload_len = (b1 & 0x7f) as usize;
                        let mut header_len = 2;

                        if payload_len == 126 {
                            if pending.len() < 4 {
                                break;
                            }
                            payload_len = u16::from_be_bytes([pending[2], pending[3]]) as usize;
                            header_len += 2;
                        } else if payload_len == 127 {
                            if pending.len() < 10 {
                                break;
                            }
                            payload_len = u64::from_be_bytes(pending[2..10].try_into().unwrap()) as usize;
                            header_len += 8;
                        }

                        if masked {
                            header_len += 4;
                        }

                        let frame_len = header_len + payload_len;
                        if pending.len() < frame_len {
                            break;
                        }

                        if opcode == 0x8 {
                            break;
                        }

                        let mut payload = pending[header_len..frame_len].to_vec();
                        if masked {
                            let mask = &pending[header_len - 4..header_len];
                            for (i, byte) in payload.iter_mut().enumerate() {
                                *byte ^= mask[i % 4];
                            }
                        }

                        pending.drain(..frame_len);

                        if opcode == 0x1 {
                            if let Ok(text) = String::from_utf8(payload) {
                                if let Some(resp) = router::process_ws_message_sync(&text, &mut session, &state) {
                                    let frame = encode_ws_text_frame(&resp);
                                    if stream.write_all(&frame).is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                continue;
            }
        }

        if method == "GET" {
            let (status, mime, body) = router::handle_http_request(path);
            let header = format!(
                "HTTP/1.1 {} OK\r\n\
                 Content-Type: {}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                status, mime, body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        } else {
            let resp = "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
        }
    }

    Ok(())
}
