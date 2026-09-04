#![cfg(target_os = "ios")]

use gpui::{
    App, MouseButton, WindowOptions, div, prelude::*, rgb,
};

struct CounterView {
    count: i32,
}

impl Render for CounterView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .justify_center()
            .items_center()
            .gap_4()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(gpui::px(64.0))
                    .child(self.count.to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(
                        div()
                            .id("minus")
                            .flex()
                            .justify_center()
                            .items_center()
                            .w(gpui::px(96.0))
                            .h(gpui::px(64.0))
                            .bg(rgb(0x8a2020))
                            .rounded_md()
                            .text_color(rgb(0xffffff))
                            .text_size(gpui::px(32.0))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _event, _window, cx| {
                                    this.count -= 1;
                                    cx.notify();
                                }),
                            )
                            .child("-1"),
                    )
                    .child(
                        div()
                            .id("plus")
                            .flex()
                            .justify_center()
                            .items_center()
                            .w(gpui::px(96.0))
                            .h(gpui::px(64.0))
                            .bg(rgb(0x206a2f))
                            .rounded_md()
                            .text_color(rgb(0xffffff))
                            .text_size(gpui::px(32.0))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _event, _window, cx| {
                                    this.count += 1;
                                    cx.notify();
                                }),
                            )
                            .child("+1"),
                    ),
            )
    }
}

/// C entry point called from the iOS app delegate after `gpui_ios_initialize()`.
///
/// Registers the counter view as the root window content, then hands off to
/// `gpui_mobile`'s iOS run loop.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_counter_main() {
    gpui_mobile::ios::ffi::set_app_callback(Box::new(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| CounterView { count: 0 })
        })
        .expect("failed to open counter window");
        cx.activate(true);
    }));

    gpui_mobile::ios::ffi::gpui_ios_run_demo();
}
