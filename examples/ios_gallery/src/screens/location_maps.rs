//! Placeholder screen for "Location & maps". See `screens/mod.rs`'s
//! `ScreenDescriptor` contract — this file will be filled in by another
//! agent; keep the exported `descriptor()` signature stable.

use super::ScreenDescriptor;
use super::common::PlaceholderScreen;
use gpui::{AnyView, App, Window, prelude::*};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "location_maps",
        title: "Location & maps",
        category: "Hardware & media",
        blurb: "location, MapKit, launchers, contacts/calendar",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PlaceholderScreen {
        id: "location_maps",
        title: "Location & maps",
        blurb: "location, MapKit, launchers, contacts/calendar",
    })
    .into()
}
