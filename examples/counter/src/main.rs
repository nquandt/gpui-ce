use gpui::{
    App, Bounds, ClickEvent, Context, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

struct Counter {
    count: i32,
}

impl Counter {
    fn new() -> Self {
        Self { count: 0 }
    }
}

impl Render for Counter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.count;

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .size(px(400.0))
            .bg(rgb(0x1e1e2e))
            .child(
                div()
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xcdd6f4))
                    .child(format!("{count}")),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .id("decrement")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x45475a))
                            .text_color(rgb(0xcdd6f4))
                            .text_lg()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x585b70)))
                            .child("-")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.count -= 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("increment")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0xa6e3a1))
                            .text_color(rgb(0x1e1e2e))
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x94e2d5)))
                            .child("+")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.count += 1;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("reset")
                    .px_4()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x585b70))
                    .text_color(rgb(0x6c7086))
                    .text_sm()
                    .cursor_pointer()
                    .hover(|style| style.border_color(rgb(0xa6adc8)).text_color(rgb(0xa6adc8)))
                    .child("Reset")
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.count = 0;
                        cx.notify();
                    })),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(400.), px(300.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Counter::new()),
        )
        .unwrap();
        cx.activate(true);
    });
}
