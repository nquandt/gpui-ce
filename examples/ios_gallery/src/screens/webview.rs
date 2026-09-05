//! Placeholder screen for "WebView". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "webview",
        title: "WebView",
        category: "Hardware & media",
        blurb: "WKWebView + native text field platform views",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "webview",
        title: "WebView",
        blurb: "WKWebView + native text field platform views",
    })
    .into()
}
