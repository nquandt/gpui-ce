//! "Text input" screen: single-line/multi-line editable text, keyboard-type
//! switching via `gpui_mobile::show_keyboard_with_type`, and live focus /
//! length / keyboard-height readouts.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Entity, Focusable, Window, div, prelude::*, px, rgb};
use gpui_ce_elements::editable_text::{EditableTextState, StringStorage, text_area, text_input};
use gpui_mobile::KeyboardType;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "text_input",
        title: "Text input",
        category: "Core UI",
        blurb: "single/multi-line, keyboard types, return key",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| TextInputScreen {
        single_line: cx.new(|cx| EditableTextState::new(StringStorage::default(), cx)),
        multi_line: cx.new(|cx| EditableTextState::new(StringStorage::default(), cx)),
        last_keyboard_request: "none yet".into(),
    })
    .into()
}

struct TextInputScreen {
    single_line: Entity<EditableTextState>,
    multi_line: Entity<EditableTextState>,
    last_keyboard_request: String,
}

impl TextInputScreen {
    fn keyboard_button(
        label: &'static str,
        kind: KeyboardType,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        button(
            label,
            cx.listener(move |this, _, window, cx| {
                gpui_mobile::show_keyboard_with_type(kind);
                let handle = this.single_line.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
                this.last_keyboard_request = format!("{label} (focused single-line field)");
                gallery_log::push(format!("text_input: show_keyboard_with_type({label})"));
                cx.notify();
            }),
        )
    }
}

impl Render for TextInputScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let single_len = self.single_line.read(cx).as_str().len();
        let multi_len = self.multi_line.read(cx).as_str().len();
        let single_focused = self
            .single_line
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        let multi_focused = self.multi_line.read(cx).focus_handle(cx).is_focused(window);
        let keyboard_height = gpui_mobile::keyboard_height();

        div()
            .id("text-input-scroll")
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(20.0))
                    .child("Text input"),
            )
            .child(note(
                "Tap a field to bring up the software keyboard. Manual test checklist: \
                 typing, autocorrect suggestions, predictive text bar, dictation (mic \
                 key), the emoji keyboard, hardware-keyboard arrow-key navigation, and \
                 copy/paste via the system edit menu. Expected: no system edit menu yet \
                 — selection handles/long-press-to-select are a known gap in \
                 gpui_ce_elements::editable_text.",
            ))
            .child(
                section("Single-line")
                    .child(
                        text_input(gpui::ElementId::from("single_line"))
                            .state(self.single_line.downgrade())
                            .border_1()
                            .rounded_md()
                            .border_color(rgb(0x555555))
                            .bg(rgb(0x2a2a2a))
                            .text_color(rgb(0xffffff))
                            .p_2()
                            .w_full()
                            .h(px(40.0))
                            .placeholder("Type here…"),
                    )
                    .child(kv("length", single_len.to_string()))
                    .child(kv("focused", single_focused.to_string())),
            )
            .child(
                section("Multi-line")
                    .child(
                        text_area(gpui::ElementId::from("multi_line"))
                            .state(self.multi_line.downgrade())
                            .border_1()
                            .rounded_md()
                            .border_color(rgb(0x555555))
                            .bg(rgb(0x2a2a2a))
                            .text_color(rgb(0xffffff))
                            .p_2()
                            .w_full()
                            .h(px(120.0))
                            .placeholder("Write something longer…"),
                    )
                    .child(kv("length", multi_len.to_string()))
                    .child(kv("focused", multi_focused.to_string())),
            )
            .child(kv("keyboard height", format!("{keyboard_height:.0}pt")))
            .child(
                section("Keyboard types")
                    .child(note(
                        "Each button focuses the single-line field above and requests a \
                         keyboard type — the software keyboard's layout should change \
                         accordingly (e.g. NumberPad shows only digits).",
                    ))
                    .child(
                        row()
                            .flex_wrap()
                            .child(Self::keyboard_button("Default", KeyboardType::Default, cx))
                            .child(Self::keyboard_button(
                                "EmailAddress",
                                KeyboardType::EmailAddress,
                                cx,
                            ))
                            .child(Self::keyboard_button("Phone", KeyboardType::Phone, cx))
                            .child(Self::keyboard_button(
                                "NumberPad",
                                KeyboardType::NumberPad,
                                cx,
                            ))
                            .child(Self::keyboard_button("URL", KeyboardType::URL, cx))
                            .child(Self::keyboard_button("Decimal", KeyboardType::Decimal, cx)),
                    )
                    .child(kv("last request", self.last_keyboard_request.clone())),
            )
            .child(
                section("Input configuration")
                    .child(note(
                        "Gap: gpui_ce_elements::editable_text::EditableTextState / \
                         text_input()/text_area() expose no per-field input \
                         configuration (no autocorrect/autocapitalize/secure-entry/\
                         return-key-type builder methods — grepped \
                         crates/gpui_elements/src/editable_text for 'autocorrect', \
                         'input_action', 'TextInputConfiguration': no matches). Only \
                         the platform's own default keyboard behavior applies.",
                    ))
                    .child(button(
                        "Copy this note to clipboard",
                        cx.listener(|_this, _, _window, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                "editable_text exposes no TextInputConfiguration".into(),
                            ));
                            gallery_log::push("text_input: copied config-gap note");
                            cx.notify();
                        }),
                    )),
            )
    }
}
