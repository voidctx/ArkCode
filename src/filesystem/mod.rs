// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct FsTreeNode {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    pub children: Option<Vec<FsTreeNode>>,
}

pub fn build_recursive_tree<P: AsRef<Path>>(base_path: P, current_rel_path: &str) -> Option<FsTreeNode> {
    let base = base_path.as_ref();
    let full_path = if current_rel_path.is_empty() {
        base.to_path_buf()
    } else {
        base.join(current_rel_path)
    };

    if !full_path.exists() {
        return None;
    }

    let file_name = if current_rel_path.is_empty() {
        "root".to_string()
    } else {
        full_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    };

    let is_dir = full_path.is_dir();
    let mut children = None;

    if is_dir {
        let mut child_nodes = Vec::new();
        if let Ok(entries) = fs::read_dir(&full_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();

                if name == ".git" || name == "target" {
                    continue;
                }

                let child_rel = if current_rel_path.is_empty() {
                    name
                } else {
                    format!("{}/{}", current_rel_path, name)
                };

                if let Some(child_node) = build_recursive_tree(base, &child_rel) {
                    child_nodes.push(child_node);
                }
            }
        }

        child_nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        children = Some(child_nodes);
    }

    Some(FsTreeNode {
        name: file_name,
        path: current_rel_path.to_string(),
        is_dir,
        children,
    })
}

pub fn read_file_content<P: AsRef<Path>>(base_path: P, rel_path: &str) -> io::Result<String> {
    let full = base_path.as_ref().join(rel_path);
    fs::read_to_string(full)
}

pub fn write_file_content<P: AsRef<Path>>(base_path: P, rel_path: &str, content: &str) -> io::Result<()> {
    let full = base_path.as_ref().join(rel_path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, content)
}

pub fn create_directory<P: AsRef<Path>>(base_path: P, rel_path: &str) -> io::Result<()> {
    let full = base_path.as_ref().join(rel_path);
    fs::create_dir_all(full)
}

pub fn delete_path<P: AsRef<Path>>(base_path: P, rel_path: &str) -> io::Result<()> {
    let full = base_path.as_ref().join(rel_path);
    if full.is_dir() {
        fs::remove_dir_all(full)
    } else if full.is_file() {
        fs::remove_file(full)
    } else {
        Ok(())
    }
}

pub fn rename_path<P: AsRef<Path>>(base_path: P, old_rel: &str, new_name: &str) -> io::Result<()> {
    let old_full = base_path.as_ref().join(old_rel);
    let parent = old_full
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid parent"))?;
    let new_full = parent.join(new_name);
    fs::rename(old_full, new_full)
}
