//! Placeholder screen for "Haptics & vibration". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "haptics",
        title: "Haptics & vibration",
        category: "Window & system",
        blurb: "impact/selection/notification, vibrate",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "haptics",
        title: "Haptics & vibration",
        blurb: "impact/selection/notification, vibrate",
    })
    .into()
}
