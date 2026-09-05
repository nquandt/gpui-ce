//! Placeholder screen for "Buttons & taps". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "buttons",
        title: "Buttons & taps",
        category: "Core UI",
        blurb: "tap, double tap, long press, mouse down/up, disabled",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "buttons",
        title: "Buttons & taps",
        blurb: "tap, double tap, long press, mouse down/up, disabled",
    })
    .into()
}
