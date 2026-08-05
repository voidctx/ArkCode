// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub description: String,
    pub build: String,
    pub ls_type: bool,
    pub path: String,
}

impl ProjectConfig {
    pub fn load_from_dir<P: AsRef<Path>>(dir_path: P) -> Option<Self> {
        let dir = dir_path.as_ref();
        let proj_file = dir.join(".proj");

        if !proj_file.is_file() {
            return None;
        }

        let content = fs::read_to_string(&proj_file).ok()?;
        let mut name = String::new();
        let mut description = String::new();
        let mut build = String::new();
        let mut ls_type = true;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "name" => name = val.to_string(),
                    "description" => description = val.to_string(),
                    "build" => build = val.to_string(),
                    "ls_type" | "lsType" => {
                        let lower = val.to_lowercase();
                        ls_type = !(lower == "false" || lower == "none" || lower == "0");
                    }
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            name = dir.file_name()?.to_string_lossy().to_string();
        }

        Some(ProjectConfig {
            name,
            description,
            build,
            ls_type,
            path: dir.to_string_lossy().to_string(),
        })
    }
}

pub static CONFIG_JSON_STR: &str = include_str!("../../config.json");

pub fn scan_workspaces() -> Vec<ProjectConfig> {
    let mut projects = Vec::new();
    let projects_dir = PathBuf::from("projects");

    if projects_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&projects_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(config) = ProjectConfig::load_from_dir(&path) {
                        projects.push(config);
                    }
                }
            }
        }
    }

    if let Some(current_config) = ProjectConfig::load_from_dir(".") {
        if !projects.iter().any(|p| p.path == current_config.path) {
            projects.push(current_config);
        }
    }

    projects
}

pub fn create_workspace(
    name: &str,
    description: &str,
    ls_type: bool,
) -> Result<ProjectConfig, String> {
    let name_str = if name.trim().is_empty() {
        format!("project-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() % 10000)
    } else {
        name.trim().to_string()
    };

    let proj_dir = PathBuf::from("projects").join(&name_str);
    fs::create_dir_all(proj_dir.join("src")).map_err(|e| e.to_string())?;

    let proj_content = format!(
        "name={}\ndescription={}\nbuild=cargo check\nls_type={}\n",
        name_str, description, ls_type
    );
    fs::write(proj_dir.join(".proj"), proj_content).map_err(|e| e.to_string())?;

    let main_rs_path = proj_dir.join("src").join("main.rs");
    if !main_rs_path.exists() {
        let starter_code = "fn main() {\n    println!(\"Hello World!\");\n}\n";
        let _ = fs::write(main_rs_path, starter_code);
    }

    ProjectConfig::load_from_dir(&proj_dir)
        .ok_or_else(|| "Failed to load created workspace".to_string())
}

pub fn delete_workspace(name: &str) -> Result<(), String> {
    let proj_dir = PathBuf::from("projects").join(name);
    if proj_dir.is_dir() {
        fs::remove_dir_all(proj_dir).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
