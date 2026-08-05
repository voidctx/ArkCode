// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use crate::router::create_router;
use std::net::SocketAddr;

pub async fn start_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let app = create_router();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));    
    println!("ArkCode WebIDE running at http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
