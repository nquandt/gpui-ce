//! Placeholder screen for "Device info". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "device_info",
        title: "Device info",
        category: "Hardware & media",
        blurb: "device, battery, connectivity, network, package",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "device_info",
        title: "Device info",
        blurb: "device, battery, connectivity, network, package",
    })
    .into()
}
