//! Placeholder screen for "Sensors". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "sensors",
        title: "Sensors",
        category: "Hardware & media",
        blurb: "accelerometer, gyroscope, magnetometer",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "sensors",
        title: "Sensors",
        blurb: "accelerometer, gyroscope, magnetometer",
    })
    .into()
}
