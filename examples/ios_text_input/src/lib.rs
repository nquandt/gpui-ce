#![cfg(target_os = "ios")]

//! Acceptance example for native iOS text input (see
//! `crates/gpui_mobile/TODO.md`). Two `gpui_ce_elements::editable_text`
//! fields — single-line and multi-line — built the normal GPUI way
//! (`Render` + `EntityInputHandler` via `EditableTextState`), with no use of
//! `gpui_mobile`'s legacy `set_text_input_callback` bridge. Typing,
//! selection, and IME composition on either field should be driven entirely
//! by `GPUITextInputView`'s `UITextInput` conformance
//! (`crates/gpui_mobile/src/ios/text_input_view.rs`).

use gpui::{App, ElementId, Entity, WindowOptions, div, prelude::*, px, rgb};
use gpui_ce_elements::editable_text::{EditableTextState, text_area, text_input};

struct TextInputDemo {
    single_line: Entity<EditableTextState>,
    multi_line: Entity<EditableTextState>,
}

impl Render for TextInputDemo {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let single_len = self.single_line.read(cx).as_str().len();
        let multi_len = self.multi_line.read(cx).as_str().len();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .p_6()
            .gap_4()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(20.0))
                    .child("Native iOS text input demo"),
            )
            .child(
                div()
                    .text_color(rgb(0x9aa0a6))
                    .text_size(px(14.0))
                    .child("Single-line field:"),
            )
            .child(
                text_input(ElementId::from("single_line"))
                    .state(self.single_line.downgrade())
                    .border_1()
                    .rounded_md()
                    .border_color(rgb(0x555555))
                    .bg(rgb(0x2a2a2a))
                    .text_color(rgb(0xffffff))
                    .p_2()
                    .w(px(320.0))
                    .h(px(40.0))
                    .placeholder("Type here…"),
            )
            .child(
                div()
                    .text_color(rgb(0x9aa0a6))
                    .text_size(px(14.0))
                    .child("Multi-line field:"),
            )
            .child(
                text_area(ElementId::from("multi_line"))
                    .state(self.multi_line.downgrade())
                    .border_1()
                    .rounded_md()
                    .border_color(rgb(0x555555))
                    .bg(rgb(0x2a2a2a))
                    .text_color(rgb(0xffffff))
                    .p_2()
                    .w(px(320.0))
                    .h(px(160.0))
                    .placeholder("Write something longer…"),
            )
            .child(
                div()
                    .text_color(rgb(0x9aa0a6))
                    .text_size(px(12.0))
                    .child(format!(
                        "single-line: {single_len} chars, multi-line: {multi_len} chars"
                    )),
            )
    }
}

/// C entry point called from the iOS app delegate after `gpui_ios_initialize()`.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_text_input_main() {
    gpui_mobile::ios::ffi::set_app_callback(Box::new(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| {
                let single_line =
                    EditableTextState::use_keyed(ElementId::from("single_line"), window, cx);
                let multi_line =
                    EditableTextState::use_keyed(ElementId::from("multi_line"), window, cx);

                TextInputDemo {
                    single_line,
                    multi_line,
                }
            })
        })
        .expect("failed to open text input window");
        cx.activate(true);
    }));

    gpui_mobile::ios::ffi::gpui_ios_run_demo();
}
