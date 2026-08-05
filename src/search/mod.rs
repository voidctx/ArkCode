// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub workspace: String,
    pub query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub is_regex: bool,
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub file: String,
    pub path: String,
    pub line: usize,
    pub text: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchMatch>,
}

pub fn search_workspace(req: &SearchRequest) -> io::Result<SearchResponse> {
    let mut results = Vec::new();
    if req.query.trim().is_empty() {
        return Ok(SearchResponse { results });
    }

    let root_path = Path::new(&req.workspace);
    if !root_path.exists() {
        return Ok(SearchResponse { results });
    }

    let query_term = if req.case_sensitive {
        req.query.clone()
    } else {
        req.query.to_lowercase()
    };

    visit_dirs_for_search(root_path, root_path, &query_term, req, &mut results)?;

    Ok(SearchResponse { results })
}

fn visit_dirs_for_search(
    root: &Path,
    dir: &Path,
    query_term: &str,
    req: &SearchRequest,
    results: &mut Vec<SearchMatch>,
) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name == ".git" || name == "target" || name.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                let _ = visit_dirs_for_search(root, &path, query_term, req, results);
            } else if path.is_file() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let rel_path = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_string();
                    for (line_idx, line) in content.lines().enumerate() {
                        let target_line = if req.case_sensitive {
                            line.to_string()
                        } else {
                            line.to_lowercase()
                        };

                        if target_line.contains(query_term) {
                            let line_str = line.trim().to_string();
                            results.push(SearchMatch {
                                file: rel_path.clone(),
                                path: rel_path.clone(),
                                line: line_idx + 1,
                                text: line_str.clone(),
                                content: line_str,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
