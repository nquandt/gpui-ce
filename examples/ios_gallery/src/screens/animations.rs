//! "Animations" screen: `with_animation` (bounce, fade, ease curves, repeat),
//! a spring animation, a live FPS counter driven by
//! `window.request_animation_frame`, and a start/stop toggle.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use gpui::{
    Animation, AnimationExt, AnyView, App, Context, SpringAnimation, SpringConfig, Window, bounce,
    div, ease_in_out, prelude::*, px, rgb,
};
use std::time::{Duration, Instant};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "animations",
        title: "Animations",
        category: "Core UI",
        blurb: "with_animation, springs, FPS",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| AnimationsScreen {
        running: true,
        fps_frames: 0,
        fps: 0.0,
        fps_window_start: Instant::now(),
        spring_target: 0.0,
    })
    .into()
}

struct AnimationsScreen {
    running: bool,
    fps_frames: u32,
    fps: f32,
    fps_window_start: Instant,
    spring_target: f32,
}

impl AnimationsScreen {
    fn tick_fps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.running {
            return;
        }
        self.fps_frames += 1;
        let elapsed = self.fps_window_start.elapsed();
        if elapsed >= Duration::from_millis(500) {
            self.fps = self.fps_frames as f32 / elapsed.as_secs_f32();
            self.fps_frames = 0;
            self.fps_window_start = Instant::now();
        }
        window.request_animation_frame();
        cx.notify();
    }
}

impl Render for AnimationsScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tick_fps(window, cx);

        div()
            .id("animations-scroll")
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
                    .child("Animations"),
            )
            .child(
                section("FPS counter")
                    .child(note(
                        "Driven by window.request_animation_frame() + on_next_frame; \
                         should read close to the display's refresh rate while running.",
                    ))
                    .child(kv("fps", format!("{:.1}", self.fps)))
                    .child(button(
                        if self.running { "Stop" } else { "Start" },
                        cx.listener(|this, _, window, cx| {
                            this.running = !this.running;
                            if this.running {
                                this.fps_window_start = Instant::now();
                                this.fps_frames = 0;
                                window.request_animation_frame();
                            }
                            cx.notify();
                        }),
                    )),
            )
            .child(
                section("Fade (repeat, ease-in-out)").child(
                    div()
                        .w(px(64.0))
                        .h(px(64.0))
                        .bg(rgb(0x7fb0ff))
                        .rounded_md()
                        .with_animation(
                            "fade",
                            Animation::new(Duration::from_secs(2))
                                .repeat()
                                .with_easing(ease_in_out),
                            |element, delta| element.opacity(0.15 + delta * 0.85),
                        ),
                ),
            )
            .child(
                section("Bounce").child(
                    div()
                        .w(px(48.0))
                        .h(px(48.0))
                        .bg(rgb(0xff9a3c))
                        .rounded_full()
                        .with_animation(
                            "bounce",
                            Animation::new(Duration::from_millis(900))
                                .repeat()
                                .with_easing(bounce(ease_in_out)),
                            |element, delta| element.mb(px(delta * 60.0)),
                        ),
                ),
            )
            .child(
                section("Ease curves (linear vs ease-in-out, one-shot)").child(
                    row()
                        .gap_6()
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .bg(rgb(0x4dff88))
                                .rounded_md()
                                .with_animation(
                                    "linear-shot",
                                    Animation::new(Duration::from_secs(2)),
                                    |element, delta| element.ml(px(delta * 120.0)),
                                ),
                        )
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .bg(rgb(0xff4d4d))
                                .rounded_md()
                                .with_animation(
                                    "ease-shot",
                                    Animation::new(Duration::from_secs(2)).with_easing(ease_in_out),
                                    |element, delta| element.ml(px(delta * 120.0)),
                                ),
                        ),
                ),
            )
            .child(
                section("Spring")
                    .child(note(
                        "Tap to retarget the spring — the box should overshoot and \
                         settle rather than move linearly.",
                    ))
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .bg(rgb(0xffe14d))
                            .rounded_md()
                            .with_spring(
                                "spring-box",
                                SpringAnimation::new(SpringConfig::new(180.0, 12.0, 1.0))
                                    .to(self.spring_target),
                                |element, value: f32| element.ml(px(value)),
                            ),
                    )
                    .child(button(
                        "Toggle spring target",
                        cx.listener(|this, _, _window, cx| {
                            this.spring_target = if this.spring_target > 100.0 {
                                0.0
                            } else {
                                160.0
                            };
                            cx.notify();
                        }),
                    )),
            )
    }
}
