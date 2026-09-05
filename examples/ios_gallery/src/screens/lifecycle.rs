//! Placeholder screen for "Lifecycle & events". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "lifecycle",
        title: "Lifecycle & events",
        category: "Window & system",
        blurb: "foreground/background, memory, thermal, insets",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "lifecycle",
        title: "Lifecycle & events",
        blurb: "foreground/background, memory, thermal, insets",
    })
    .into()
}
