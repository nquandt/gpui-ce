//! Placeholder screen for "Clipboard". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "clipboard",
        title: "Clipboard",
        category: "Window & system",
        blurb: "copy/paste text, paste image",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "clipboard",
        title: "Clipboard",
        blurb: "copy/paste text, paste image",
    })
    .into()
}
