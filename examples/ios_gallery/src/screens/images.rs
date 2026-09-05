//! Placeholder screen for "Images & SVG". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "images",
        title: "Images & SVG",
        category: "Core UI",
        blurb: "embedded PNG, remote image, SVG, GIF",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "images",
        title: "Images & SVG",
        blurb: "embedded PNG, remote image, SVG, GIF",
    })
    .into()
}
