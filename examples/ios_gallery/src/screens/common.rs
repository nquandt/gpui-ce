//! Small shared UI helpers used by every gallery screen. Keep this file
//! dependency-light and stable — other screens are built against it.

use gpui::{App, ClickEvent, Div, Stateful, Window, div, prelude::*, px, rgb};

/// A titled vertical section with a dark card background.
pub fn section(title: impl Into<gpui::SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .bg(rgb(0x252525))
        .rounded_md()
        .child(
            div()
                .text_color(rgb(0xd0d0d0))
                .text_size(px(13.0))
                .child(title.into()),
        )
}

/// A horizontal row with a small gap, useful inside a [`section`].
pub fn row() -> Div {
    div().flex().flex_row().items_center().gap_2()
}

/// A dark pill button. `label` is also used as the element id, so labels
/// used more than once on a screen should be made unique by the caller.
pub fn button(
    label: impl Into<gpui::SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(gpui::ElementId::from(label.clone()))
        .flex()
        .items_center()
        .justify_center()
        .px_4()
        .py_2()
        .bg(rgb(0x33415c))
        .rounded_full()
        .text_color(rgb(0xffffff))
        .text_size(px(14.0))
        .on_click(on_click)
        .child(label)
}

/// A plain text label in the standard body color/size.
pub fn label(text: impl Into<gpui::SharedString>) -> Div {
    div()
        .text_color(rgb(0xffffff))
        .text_size(px(14.0))
        .child(text.into())
}

/// A monospace-styled snippet of text (uses the default font family since
/// this crate does not bundle a monospace font, but sizes/colors it as code).
pub fn mono(text: impl Into<gpui::SharedString>) -> Div {
    div()
        .text_color(rgb(0xa0d0a0))
        .text_size(px(12.0))
        .child(text.into())
}

/// A "key: value" row.
pub fn kv(key: impl Into<gpui::SharedString>, value: impl Into<gpui::SharedString>) -> Div {
    row()
        .child(
            div()
                .text_color(rgb(0x9aa0a6))
                .text_size(px(13.0))
                .w(px(140.0))
                .child(key.into()),
        )
        .child(
            div()
                .text_color(rgb(0xffffff))
                .text_size(px(13.0))
                .child(value.into()),
        )
}

/// A muted paragraph of explanatory text.
pub fn note(text: impl Into<gpui::SharedString>) -> Div {
    div()
        .text_color(rgb(0x777d84))
        .text_size(px(12.0))
        .child(text.into())
}

/// The standard "not implemented yet" placeholder body for a screen that
/// hasn't been built out yet.
pub fn placeholder(id: &'static str, title: &'static str, blurb: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .child(
            div()
                .text_color(rgb(0xffffff))
                .text_size(px(20.0))
                .child(title),
        )
        .child(note(blurb))
        .child(mono(format!("Not implemented yet — see screens/{id}.rs")))
}

/// A view that renders [`placeholder`] for a screen not yet implemented.
/// Screen files that haven't been built out yet can construct this directly
/// from their `descriptor()`'s `build` function.
pub struct PlaceholderScreen {
    pub id: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
}

impl gpui::Render for PlaceholderScreen {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        placeholder(self.id, self.title, self.blurb)
    }
}
