//! "Performance": a stress toggle rendering N colored quads + N text
//! labels in a grid, an FPS/frame-time readout driven by
//! `window.request_animation_frame()`, and a separate scroll-stress list.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use std::time::Instant;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "performance",
        title: "Performance",
        category: "Window & system",
        blurb: "quad/text stress, FPS",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| PerformanceScreen {
        count: 0,
        last_frame: Instant::now(),
        frame_ms: 0.0,
        fps: 0.0,
        frame_count: 0,
    })
    .into()
}

struct PerformanceScreen {
    count: usize,
    last_frame: Instant,
    frame_ms: f32,
    fps: f32,
    frame_count: u64,
}

const COLORS: [u32; 6] = [0xd9534f, 0x5bc0de, 0x5cb85c, 0xf0ad4e, 0x9370db, 0x33415c];

impl Render for PerformanceScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Drive continuous repainting so the FPS/frame-time readout stays
        // live while the stress grid is on screen.
        window.request_animation_frame();

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        if elapsed > 0.0 {
            self.frame_ms = elapsed * 1000.0;
            let instant_fps = 1.0 / elapsed;
            // Light smoothing so the number is readable rather than jittering
            // every frame.
            self.fps = if self.fps == 0.0 {
                instant_fps
            } else {
                self.fps * 0.9 + instant_fps * 0.1
            };
        }
        self.frame_count += 1;

        let mut grid = div().flex().flex_row().flex_wrap().gap_1();
        for i in 0..self.count {
            let color = COLORS[i % COLORS.len()];
            grid = grid.child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .bg(rgb(color))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0xffffff))
                    .text_size(px(8.0))
                    .child(format!("{i}")),
            );
        }

        let mut scroll_list = div()
            .id("performance_scroll_stress")
            .flex()
            .flex_col()
            .h(px(200.0))
            .overflow_y_scroll()
            .border_1()
            .border_color(rgb(0x444444));
        for i in 0..300 {
            scroll_list = scroll_list.child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(0x2a2a2a))
                    .text_color(rgb(0xffffff))
                    .text_size(px(13.0))
                    .child(format!("Scroll stress row {i}")),
            );
        }

        div()
            .id("performance-scroll")
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
                    .child("Performance"),
            )
            .child(note(
                "Smooth should look like: the FPS readout stays near the \
                 display's refresh rate (60 or 120) and frame time stays low \
                 and steady as N grows; stutter or a climbing frame time \
                 means the renderer is struggling at that N. Scroll the list \
                 below with a finger — it should track your finger with no \
                 visible lag or tearing.",
            ))
            .child(
                section("Frame timing (window.request_animation_frame)")
                    .child(kv("fps (smoothed)", format!("{:.1}", self.fps)))
                    .child(kv("frame time", format!("{:.2} ms", self.frame_ms)))
                    .child(kv("frames rendered", self.frame_count.to_string())),
            )
            .child(
                section("Quad + text stress grid")
                    .child(
                        row()
                            .flex_wrap()
                            .child(button(
                                "0",
                                cx.listener(|this, _, _window, cx| {
                                    this.count = 0;
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "100",
                                cx.listener(|this, _, _window, cx| {
                                    this.count = 100;
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "1000",
                                cx.listener(|this, _, _window, cx| {
                                    this.count = 1000;
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "5000",
                                cx.listener(|this, _, _window, cx| {
                                    this.count = 5000;
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(note(format!("rendering {} quads + labels", self.count)))
                    .child(grid),
            )
            .child(section("Scroll stress (300-row list)").child(scroll_list))
    }
}
