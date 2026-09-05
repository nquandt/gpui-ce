//! Placeholder screen for "Camera". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "camera",
        title: "Camera",
        category: "Hardware & media",
        blurb: "preview, capture, image picker",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "camera",
        title: "Camera",
        blurb: "preview, capture, image picker",
    })
    .into()
}
