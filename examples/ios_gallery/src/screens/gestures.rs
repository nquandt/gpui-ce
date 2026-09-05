//! Placeholder screen for "Gestures & touch". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "gestures",
        title: "Gestures & touch",
        category: "Core UI",
        blurb: "touch points, drag, pinch attempt, event log",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "gestures",
        title: "Gestures & touch",
        blurb: "touch points, drag, pinch attempt, event log",
    })
    .into()
}
