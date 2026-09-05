//! "Scrolling & lists" screen: a 5000-row `uniform_list`, a nested
//! horizontally scrolling row of cards, and scroll-to-item buttons.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{
    AnyView, App, Context, ScrollStrategy, UniformListScrollHandle, Window, div, hsla, prelude::*,
    px, rgb, uniform_list,
};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "scrolling",
        title: "Scrolling & lists",
        category: "Core UI",
        blurb: "5000-row list, nested horizontal, momentum",
        build,
    }
}

const ROW_COUNT: usize = 5000;
const CARD_COUNT: usize = 50;

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| ScrollingScreen {
        scroll_handle: UniformListScrollHandle::new(),
    })
    .into()
}

struct ScrollingScreen {
    scroll_handle: UniformListScrollHandle,
}

fn swatch_color(index: usize) -> gpui::Hsla {
    hsla((index % 360) as f32 / 360.0, 0.55, 0.5, 1.0)
}

impl Render for ScrollingScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let offset = self.scroll_handle.0.borrow().base_handle.offset();

        div()
            .id("scrolling-scroll")
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .on_scroll_wheel(|event, _window, _cx| {
                log::info!(
                    "gallery scrolling: outer scroller got delta {:?} phase {:?}",
                    event.delta,
                    event.touch_phase
                );
            })
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(20.0))
                    .child("Scrolling & lists"),
            )
            .child(note(
                "Fling the list below — it should decelerate with momentum. The \
                 horizontal row of cards nested inside the vertical scroller should \
                 scroll independently, and horizontal drags on it should not also \
                 scroll the outer vertical list.",
            ))
            .child(
                row()
                    .flex_wrap()
                    .child(button(
                        "Scroll to row 2500",
                        cx.listener(|this, _, _window, cx| {
                            this.scroll_handle
                                .scroll_to_item(2500, ScrollStrategy::Center);
                            gallery_log::push("scrolling: scroll_to_item(2500)");
                            cx.notify();
                        }),
                    ))
                    .child(button(
                        "Scroll to top",
                        cx.listener(|this, _, _window, cx| {
                            this.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
                            gallery_log::push("scrolling: scroll_to_item(0)");
                            cx.notify();
                        }),
                    )),
            )
            .child(kv(
                "scroll offset",
                format!("x={:.0}, y={:.0}", f32::from(offset.x), f32::from(offset.y)),
            ))
            .child(
                section("Nested horizontal row (50 cards)").child(
                    div()
                        .id("card-row-scroll")
                        .on_scroll_wheel(|event, _window, _cx| {
                            log::info!(
                                "gallery scrolling: card row got delta {:?} phase {:?}",
                                event.delta,
                                event.touch_phase
                            );
                        })
                        .flex()
                        .flex_row()
                        .gap_2()
                        .p_2()
                        .h(px(96.0))
                        .overflow_x_scroll()
                        .children((0..CARD_COUNT).map(|i| {
                            div()
                                .id(gpui::ElementId::from(format!("card-{i}")))
                                .flex_none()
                                .w(px(72.0))
                                .h_full()
                                .rounded_md()
                                .bg(swatch_color(i * 7))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(0xffffff))
                                .child(format!("{i}"))
                        })),
                ),
            )
            .child(
                div()
                    .text_color(rgb(0xd0d0d0))
                    .text_size(px(13.0))
                    .child("5000-row list:"),
            )
            .child(
                div().h(px(420.0)).child(
                    uniform_list("row-list", ROW_COUNT, move |range, _window, _cx| {
                        range
                            .map(|i| {
                                div()
                                    .id(gpui::ElementId::from(format!("row-{i}")))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .h(px(36.0))
                                    .border_b_1()
                                    .border_color(rgb(0x2a2a2a))
                                    .child(
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .rounded_sm()
                                            .bg(swatch_color(i * 37)),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(0xffffff))
                                            .text_size(px(13.0))
                                            .child(format!("row {i}")),
                                    )
                            })
                            .collect()
                    })
                    .track_scroll(&self.scroll_handle)
                    .size_full(),
                ),
            )
    }
}
