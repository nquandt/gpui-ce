//! Placeholder screen for "Video & audio". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "media",
        title: "Video & audio",
        category: "Hardware & media",
        blurb: "AVPlayer video, audio player, media session",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "media",
        title: "Video & audio",
        blurb: "AVPlayer video, audio player, media session",
    })
    .into()
}
