//! Placeholder screen for "Typography". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "typography",
        title: "Typography",
        category: "Core UI",
        blurb: "sizes, weights, emoji, CJK, RTL, wrapping",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "typography",
        title: "Typography",
        blurb: "sizes, weights, emoji, CJK, RTL, wrapping",
    })
    .into()
}
