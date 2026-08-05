// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use crate::workspace::ProjectConfig;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Deserialize)]
pub struct BuildRequest {
    pub workspace: String,
}

#[derive(Debug, Serialize)]
pub struct BuildResponse {
    pub exit_code: i32,
    pub output: String,
}

pub fn execute_build(req: &BuildRequest) -> Result<BuildResponse, String> {
    let config = ProjectConfig::load_from_dir(&req.workspace)
        .ok_or_else(|| "Invalid workspace directory or missing .proj file".to_string())?;

    let cmd_str = if config.build.is_empty() {
        "cargo build".to_string()
    } else {
        config.build
    };

    let mut path_env = std::env::var("PATH").unwrap_or_default();
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let cargo_bin = format!("{}\\.cargo\\bin", user_profile);
        path_env = format!("{};{}", cargo_bin, path_env);
    }

    let output = if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", &cmd_str])
            .env("PATH", &path_env)
            .current_dir(&req.workspace)
            .output()
    } else {
        Command::new("sh")
            .args(["-c", &cmd_str])
            .env("PATH", &path_env)
            .current_dir(&req.workspace)
            .output()
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let full_log = format!("=== Running: {} ===\n{}\n{}", cmd_str, stdout, stderr);
            let code = out.status.code().unwrap_or(-1);

            Ok(BuildResponse {
                exit_code: code,
                output: full_log,
            })
        }
        Err(err) => Err(format!("Failed to execute build process: {}", err)),
    }
}
