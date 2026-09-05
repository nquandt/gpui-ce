//! Placeholder screen for "Scrolling & lists". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "scrolling",
        title: "Scrolling & lists",
        category: "Core UI",
        blurb: "5000-row list, nested horizontal, momentum",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "scrolling",
        title: "Scrolling & lists",
        blurb: "5000-row list, nested horizontal, momentum",
    })
    .into()
}
