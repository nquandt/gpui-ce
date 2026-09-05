//! Placeholder screen for "Layout & insets". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "layout_insets",
        title: "Layout & insets",
        category: "Window & system",
        blurb: "safe area, keyboard inset, rotation",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "layout_insets",
        title: "Layout & insets",
        blurb: "safe area, keyboard inset, rotation",
    })
    .into()
}
