//! Placeholder screen for "Text input". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "text_input",
        title: "Text input",
        category: "Core UI",
        blurb: "single/multi-line, keyboard types, return key",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "text_input",
        title: "Text input",
        blurb: "single/multi-line, keyboard types, return key",
    })
    .into()
}
