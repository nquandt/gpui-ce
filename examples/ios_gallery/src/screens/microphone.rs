//! Placeholder screen for "Microphone". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "microphone",
        title: "Microphone",
        category: "Hardware & media",
        blurb: "record, amplitude, playback",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "microphone",
        title: "Microphone",
        blurb: "record, amplitude, playback",
    })
    .into()
}
