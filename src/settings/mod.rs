// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeSettings {
    pub editor_font_size: u32,
    pub terminal_font_size: u32,
    pub tab_size: u32,
    pub auto_save: String,
    pub lsp_target: String,
    pub theme: String,
}

impl Default for IdeSettings {
    fn default() -> Self {
        IdeSettings {
            editor_font_size: 14,
            terminal_font_size: 13,
            tab_size: 4,
            auto_save: "off".to_string(),
            lsp_target: "127.0.0.1:2087".to_string(),
            theme: "glass".to_string(),
        }
    }
}

impl IdeSettings {
    pub fn load() -> Self {
        if let Ok(content) = fs::read_to_string("settings.json") {
            if let Ok(settings) = serde_json::from_str::<IdeSettings>(&content) {
                return settings;
            }
        }
        IdeSettings::default()
    }

    pub fn save(&self) -> io::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("settings.json", content)
    }
}
