//! Placeholder screen for "Appearance". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "appearance",
        title: "Appearance",
        category: "Window & system",
        blurb: "light/dark, status bar style, thermal",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "appearance",
        title: "Appearance",
        blurb: "light/dark, status bar style, thermal",
    })
    .into()
}
