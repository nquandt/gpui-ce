//! Placeholder screen for "Storage". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "storage",
        title: "Storage",
        category: "Window & system",
        blurb: "preferences, files, keychain credentials",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "storage",
        title: "Storage",
        blurb: "preferences, files, keychain credentials",
    })
    .into()
}
