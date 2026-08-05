// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferState {
    pub path: String,
    pub is_dirty: bool,
    pub cursor_line: usize,
    pub cursor_column: usize,
}

#[derive(Debug, Default)]
pub struct EditorManager {
    pub active_buffers: Vec<BufferState>,
}

impl EditorManager {
    pub fn new() -> Self {
        Self::default()
    }
}
