// SPDX-License-Identifier: AGPL-3.0-only
//
// Copyright (C) 2026 xpert0
//
// This file is part of ArkCode.

pub static INDEX_HTML: &str = include_str!("../static/index.html");
pub static APP_JS: &str = include_str!("../static/app.js");
pub static MAIN_CSS: &str = include_str!("../static/styles/main.css");
pub static BASE_CSS: &str = include_str!("../static/styles/base.css");
pub static EDITOR_CSS: &str = include_str!("../static/styles/editor.css");
pub static GLASS_CSS: &str = include_str!("../static/styles/glass.css");
pub static SIDEBAR_CSS: &str = include_str!("../static/styles/sidebar.css");

pub fn get_asset(path: &str) -> Option<(&'static str, &'static str)> {
    match path {
        "/" | "/index.html" => Some((INDEX_HTML, "text/html; charset=utf-8")),
        "/app.js" => Some((APP_JS, "application/javascript; charset=utf-8")),
        "/styles/main.css" => Some((MAIN_CSS, "text/css; charset=utf-8")),
        "/styles/base.css" => Some((BASE_CSS, "text/css; charset=utf-8")),
        "/styles/editor.css" => Some((EDITOR_CSS, "text/css; charset=utf-8")),
        "/styles/glass.css" => Some((GLASS_CSS, "text/css; charset=utf-8")),
        "/styles/sidebar.css" => Some((SIDEBAR_CSS, "text/css; charset=utf-8")),
        _ => None,
    }
}
