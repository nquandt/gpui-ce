//! Placeholder screen for "Performance". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "performance",
        title: "Performance",
        category: "Window & system",
        blurb: "quad/text stress, FPS",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "performance",
        title: "Performance",
        blurb: "quad/text stress, FPS",
    })
    .into()
}
