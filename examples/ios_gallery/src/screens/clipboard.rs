//! "Clipboard": round-trips text through `cx.write_to_clipboard` /
//! `cx.read_from_clipboard`, and does the same for images — copying a small
//! bundled swatch PNG, and rendering whatever image entry (if any) is
//! currently on the clipboard (e.g. a photo copied from Photos).

use super::ScreenDescriptor;
use super::common::{button, note, section};
use crate::log as gallery_log;
use gpui::{
    AnyView, App, ClipboardEntry, ClipboardItem, Context, Image, ImageFormat, Window, div, img,
    prelude::*, px, rgb,
};
use gpui_ce_elements::editable_text::{EditableTextState, StringStorage, text_input};
use std::sync::Arc;

/// A tiny bundled 32x32 red PNG, used so "Copy image" has something cheap to
/// put on the clipboard without a network fetch or new crate dependency.
const SWATCH_PNG: &[u8] = include_bytes!("../../assets/gallery_swatch.png");

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "clipboard",
        title: "Clipboard",
        category: "Window & system",
        blurb: "copy/paste text, paste image",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| ClipboardScreen {
        field: cx.new(|cx| EditableTextState::new(StringStorage::default(), cx)),
        pasted_text: None,
        pasted_image: None,
        status: String::new(),
    })
    .into()
}

struct ClipboardScreen {
    field: gpui::Entity<EditableTextState>,
    pasted_text: Option<String>,
    pasted_image: Option<Arc<Image>>,
    status: String,
}

impl Render for ClipboardScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("clipboard-scroll")
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
                    .child("Clipboard"),
            )
            .child(note(
                "Type in the field and tap Copy, then Paste to read it back. \
                 To test image paste: open Photos, copy a photo, come back \
                 here and tap Paste image.",
            ))
            .child(
                section("Text")
                    .child(
                        text_input(gpui::ElementId::from("clipboard_field"))
                            .state(self.field.downgrade())
                            .border_1()
                            .rounded_md()
                            .border_color(rgb(0x555555))
                            .bg(rgb(0x2a2a2a))
                            .text_color(rgb(0xffffff))
                            .p_2()
                            .w_full()
                            .h(px(40.0))
                            .placeholder("Type something to copy…"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(button(
                                "Copy",
                                cx.listener(|this, _, _window, cx| {
                                    let text = this.field.read(cx).as_str().to_string();
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                    this.status = format!("Copied {} chars.", text.len());
                                    gallery_log::push("clipboard: copied text");
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "Paste",
                                cx.listener(|this, _, _window, cx| {
                                    match cx.read_from_clipboard() {
                                        Some(item) => match item.text() {
                                            Some(text) => {
                                                this.status =
                                                    format!("Pasted {} chars.", text.len());
                                                this.pasted_text = Some(text);
                                            }
                                            None => {
                                                this.status = "Clipboard has no text entry.".into();
                                                this.pasted_text = None;
                                            }
                                        },
                                        None => {
                                            this.status = "Clipboard is empty.".into();
                                            this.pasted_text = None;
                                        }
                                    }
                                    gallery_log::push("clipboard: paste text attempted");
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(match &self.pasted_text {
                        Some(text) => note(format!("Pasted text: {text}")),
                        None => note("(nothing pasted yet)"),
                    }),
            )
            .child(
                section("Image")
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(button(
                                "Copy image",
                                cx.listener(|this, _, _window, cx| {
                                    let image =
                                        Image::from_bytes(ImageFormat::Png, SWATCH_PNG.to_vec());
                                    cx.write_to_clipboard(ClipboardItem::new_image(&image));
                                    this.status = "Copied the bundled swatch image.".into();
                                    gallery_log::push("clipboard: copied image");
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "Paste image",
                                cx.listener(|this, _, _window, cx| {
                                    match cx.read_from_clipboard() {
                                        Some(item) => {
                                            let image = item.entries().iter().find_map(|entry| {
                                                if let ClipboardEntry::Image(image) = entry {
                                                    Some(Arc::new(image.clone()))
                                                } else {
                                                    None
                                                }
                                            });
                                            match image {
                                                Some(image) => {
                                                    this.status = format!(
                                                        "Pasted a {:?} image ({} bytes).",
                                                        image.format,
                                                        image.bytes.len()
                                                    );
                                                    this.pasted_image = Some(image);
                                                }
                                                None => {
                                                    this.status =
                                                        "Clipboard has no image entry.".into();
                                                    this.pasted_image = None;
                                                }
                                            }
                                        }
                                        None => {
                                            this.status = "Clipboard is empty.".into();
                                            this.pasted_image = None;
                                        }
                                    }
                                    gallery_log::push("clipboard: paste image attempted");
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(match self.pasted_image.clone() {
                        Some(image) => div()
                            .child(img(image).w(px(120.0)).h(px(120.0)))
                            .child(note("Rendered above via gpui::img().")),
                        None => div().child(note("(no image pasted yet)")),
                    }),
            )
            .child(if self.status.is_empty() {
                div()
            } else {
                div().child(note(self.status.clone()))
            })
    }
}
