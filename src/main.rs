// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

pub mod activities;
pub mod assets;
pub mod build;
pub mod contextmenu;
pub mod editor;
pub mod filesystem;
pub mod http;
pub mod lsp;
pub mod router;
pub mod search;
pub mod settings;
pub mod terminal;
pub mod workspace;

use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let port: u16 = args
        .get(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let _ = std::fs::create_dir_all("projects");
    println!("Starting ArkCode WebIDE on port {}...", port);
    http::start_server(port).await?;
    Ok(())
}
