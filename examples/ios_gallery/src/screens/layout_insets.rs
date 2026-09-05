//! "Layout & insets": draws bands over the safe-area insets and the
//! keyboard inset reported by `gpui_mobile`, plus live viewport/scale
//! readouts, so a tester can rotate the device / open the keyboard / use
//! split view and watch the bands track reality.
//!
//! NOTE: GPUI core does not currently expose `Window::insets()` /
//! `Window::on_insets_changed()` to app code even though the iOS backend
//! implements both (`crates/gpui_mobile/src/ios/window.rs:2000` and
//! `:2004`, behind the `PlatformWindow` trait). This screen therefore reads
//! `gpui_mobile::safe_area_insets()` / `gpui_mobile::keyboard_height()`
//! (globals updated by the iOS shell) instead of a `Window` API.

use super::ScreenDescriptor;
use super::common::{kv, note, section};
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "layout_insets",
        title: "Layout & insets",
        category: "Window & system",
        blurb: "safe area, keyboard inset, rotation",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    use gpui_ce_elements::editable_text::{EditableTextState, StringStorage};
    cx.new(|cx| LayoutInsetsScreen {
        field: cx.new(|cx| EditableTextState::new(StringStorage::default(), cx)),
    })
    .into()
}

struct LayoutInsetsScreen {
    field: gpui::Entity<gpui_ce_elements::editable_text::EditableTextState>,
}

fn band(label: &'static str, size: f32, color: u32, horizontal: bool) -> impl IntoElement {
    let mut d = div()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(color))
        .text_color(rgb(0xffffff))
        .text_size(px(11.0))
        .child(format!("{label} {size:.0}pt"));
    d = if horizontal {
        d.h(px(size.max(1.0))).w_full()
    } else {
        d.w(px(size.max(1.0))).h(px(60.0))
    };
    d
}

impl Render for LayoutInsetsScreen {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Keep readouts live across rotation / keyboard show-hide, which are
        // driven by native callbacks into the `gpui_mobile` globals, not by
        // GPUI's own invalidation.
        window.request_animation_frame();

        let (top, bottom, left, right) = gpui_mobile::safe_area_insets();
        let keyboard = gpui_mobile::keyboard_height();
        let viewport = window.viewport_size();
        let scale = window.scale_factor();

        div()
            .id("layout_insets-scroll")
            .overflow_y_scroll()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(20.0))
                    .child("Layout & insets"),
            )
            .child(note(
                "Rotate the device, open the keyboard (tap the field below), \
                 and — on iPad — resize this app in Split View. Watch the \
                 colored bands and numbers update to match the new safe area, \
                 keyboard height, viewport size and scale factor.",
            ))
            .child(
                section("Safe-area bands (drawn to scale, capped at 60pt)")
                    .child(band("top", top, 0xd9534f, true))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1()
                            .child(band("left", left, 0x5bc0de, false))
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(60.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(rgb(0x2f2f2f))
                                    .text_color(rgb(0x9aa0a6))
                                    .text_size(px(11.0))
                                    .child("content area"),
                            )
                            .child(band("right", right, 0x5bc0de, false)),
                    )
                    .child(band("bottom", bottom, 0xd9534f, true))
                    .child(band("keyboard", keyboard, 0xf0ad4e, true)),
            )
            .child(
                section("Readouts")
                    .child(kv("safe area top", format!("{top:.1}pt")))
                    .child(kv("safe area bottom", format!("{bottom:.1}pt")))
                    .child(kv("safe area left", format!("{left:.1}pt")))
                    .child(kv("safe area right", format!("{right:.1}pt")))
                    .child(kv("keyboard height", format!("{keyboard:.1}pt")))
                    .child(kv(
                        "viewport size",
                        format!(
                            "{:.1} x {:.1}",
                            f32::from(viewport.width),
                            f32::from(viewport.height)
                        ),
                    ))
                    .child(kv("scale factor", format!("{scale:.2}"))),
            )
            .child(
                section("Summon the keyboard")
                    .child(note("Tap the field — the keyboard band above should grow."))
                    .child(
                        gpui_ce_elements::editable_text::text_input(gpui::ElementId::from(
                            "layout_insets_field",
                        ))
                        .state(self.field.downgrade())
                        .border_1()
                        .rounded_md()
                        .border_color(rgb(0x555555))
                        .bg(rgb(0x2a2a2a))
                        .text_color(rgb(0xffffff))
                        .p_2()
                        .w_full()
                        .h(px(40.0))
                        .placeholder("Tap to open the keyboard…"),
                    ),
            )
            .child(note(
                "Platform gap: gpui_mobile's iOS PlatformWindow implements \
                 `insets()`/`on_insets_changed()` (ios/window.rs:2000,2004) but \
                 core GPUI's `Window` does not expose either to app code, so \
                 this screen reads the gpui_mobile globals instead of a \
                 Window API.",
            ))
    }
}
