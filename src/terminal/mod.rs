// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use std::io;
use std::path::Path;
use std::process::Command;

pub fn execute_shell_command<P: AsRef<Path>>(
    cwd: P,
    command: &str,
) -> io::Result<(String, String)> {
    let mut path_env = std::env::var("PATH").unwrap_or_default();
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        path_env = format!("{}\\.cargo\\bin;{}", user_profile, path_env);
    }

    let out = if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", command])
            .env("PATH", &path_env)
            .current_dir(cwd)
            .output()?
    } else {
        Command::new("sh")
            .args(["-c", command])
            .env("PATH", &path_env)
            .current_dir(cwd)
            .output()?
    };

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    Ok((stdout, stderr))
}
