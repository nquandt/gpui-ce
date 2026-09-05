//! Placeholder screen for "Dialogs & pickers". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "dialogs",
        title: "Dialogs & pickers",
        category: "Window & system",
        blurb: "prompt(), file picker, open with, share",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "dialogs",
        title: "Dialogs & pickers",
        blurb: "prompt(), file picker, open with, share",
    })
    .into()
}
