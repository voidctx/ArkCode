// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivityItem {
    pub id: String,
    pub title: String,
    pub icon: String,
}

pub fn get_default_activities() -> Vec<ActivityItem> {
    vec![
        ActivityItem { id: "explorer".into(), title: "Explorer".into(), icon: "files".into() },
        ActivityItem { id: "search".into(), title: "Search".into(), icon: "search".into() },
        ActivityItem { id: "build".into(), title: "Run / Build".into(), icon: "play".into() },
        ActivityItem { id: "settings".into(), title: "Settings".into(), icon: "gear".into() },
    ]
}
