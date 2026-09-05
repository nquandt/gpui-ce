//! Placeholder screen for "Security & notifications". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "security_notify",
        title: "Security & notifications",
        category: "Hardware & media",
        blurb: "Face ID, local notifications, permissions",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "security_notify",
        title: "Security & notifications",
        blurb: "Face ID, local notifications, permissions",
    })
    .into()
}
