// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
}

pub fn get_explorer_context_menu() -> Vec<ContextMenuItem> {
    vec![
        ContextMenuItem { id: "new_file".into(), label: "New File".into(), shortcut: None },
        ContextMenuItem { id: "new_folder".into(), label: "New Folder".into(), shortcut: None },
        ContextMenuItem { id: "rename".into(), label: "Rename".into(), shortcut: Some("F2".into()) },
        ContextMenuItem { id: "delete".into(), label: "Delete".into(), shortcut: Some("Delete".into()) },
        ContextMenuItem { id: "copy_path".into(), label: "Copy Path".into(), shortcut: None },
    ]
}
