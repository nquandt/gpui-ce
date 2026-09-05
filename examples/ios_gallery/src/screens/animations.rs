//! Placeholder screen for "Animations". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "animations",
        title: "Animations",
        category: "Core UI",
        blurb: "with_animation, springs, FPS",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "animations",
        title: "Animations",
        blurb: "with_animation, springs, FPS",
    })
    .into()
}
