//! The screen registry: the contract other agents build against. Each
//! screen lives in its own file exposing a `descriptor()` function; this
//! module just lists them in the fixed display order.

pub mod common;

pub mod animations;
pub mod appearance;
pub mod buttons;
pub mod camera;
pub mod clipboard;
pub mod colors_shapes;
pub mod components;
pub mod device_info;
pub mod dialogs;
pub mod gestures;
pub mod haptics;
pub mod images;
pub mod layout_insets;
pub mod lifecycle;
pub mod location_maps;
pub mod media;
pub mod microphone;
pub mod performance;
pub mod report;
pub mod scrolling;
pub mod security_notify;
pub mod sensors;
pub mod storage;
pub mod text_input;
pub mod typography;
pub mod webview;

use gpui::{AnyView, App, Window};

/// Static metadata + view constructor for one gallery screen.
#[derive(Clone, Copy)]
pub struct ScreenDescriptor {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub blurb: &'static str,
    pub build: fn(&mut Window, &mut App) -> AnyView,
}

/// All gallery screens, in fixed display order (grouped by category).
pub fn all() -> Vec<ScreenDescriptor> {
    vec![
        // Core UI
        buttons::descriptor(),
        text_input::descriptor(),
        scrolling::descriptor(),
        gestures::descriptor(),
        typography::descriptor(),
        colors_shapes::descriptor(),
        images::descriptor(),
        animations::descriptor(),
        // Window & system
        layout_insets::descriptor(),
        appearance::descriptor(),
        clipboard::descriptor(),
        dialogs::descriptor(),
        haptics::descriptor(),
        lifecycle::descriptor(),
        storage::descriptor(),
        performance::descriptor(),
        // Hardware & media
        device_info::descriptor(),
        sensors::descriptor(),
        camera::descriptor(),
        media::descriptor(),
        microphone::descriptor(),
        security_notify::descriptor(),
        webview::descriptor(),
        location_maps::descriptor(),
        components::descriptor(),
        // Meta
        report::descriptor(),
    ]
}
