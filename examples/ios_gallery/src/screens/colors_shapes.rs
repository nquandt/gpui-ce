//! Placeholder screen for "Colors & shapes". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "colors_shapes",
        title: "Colors & shapes",
        category: "Core UI",
        blurb: "gradients, shadows, borders, radius, opacity",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "colors_shapes",
        title: "Colors & shapes",
        blurb: "gradients, shadows, borders, radius, opacity",
    })
    .into()
}
