//! "Gestures & touch" screen: finger position + trail, a draggable box, a
//! pinch-detection area (expected to never fire on iOS today), and an event
//! log of recent mouse/scroll events.

use super::ScreenDescriptor;
use super::common::{kv, note, section};
use crate::log as gallery_log;
use gpui::{
    AnyView, App, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PinchEvent,
    Point, ScrollWheelEvent, Window, div, prelude::*, px, rgb,
};
use std::collections::VecDeque;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "gestures",
        title: "Gestures & touch",
        category: "Core UI",
        blurb: "touch points, drag, pinch attempt, event log",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| GesturesScreen {
        trail: VecDeque::new(),
        touching: false,
        drag_pos: Point::new(px(20.0), px(20.0)),
        dragging: false,
        pinch_observed: false,
        event_log: VecDeque::new(),
    })
    .into()
}

const TRAIL_CAP: usize = 30;
const LOG_CAP: usize = 10;

struct GesturesScreen {
    trail: VecDeque<Point<gpui::Pixels>>,
    touching: bool,
    drag_pos: Point<gpui::Pixels>,
    dragging: bool,
    pinch_observed: bool,
    event_log: VecDeque<String>,
}

impl GesturesScreen {
    fn push_log(&mut self, text: String) {
        if self.event_log.len() >= LOG_CAP {
            self.event_log.pop_front();
        }
        self.event_log.push_back(text);
    }

    fn push_trail(&mut self, pos: Point<gpui::Pixels>) {
        if self.trail.len() >= TRAIL_CAP {
            self.trail.pop_front();
        }
        self.trail.push_back(pos);
    }
}

impl Render for GesturesScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let log_text = self
            .event_log
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        div()
            .id("gestures-scroll")
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
                    .child("Gestures & touch"),
            )
            .child(
                section("Touch position + trail")
                    .child(note(
                        "Drag your finger inside the box — a circle should follow it \
                         with a fading trail of the last 30 positions.",
                    ))
                    .child(
                        div()
                            .id("touch-trail-area")
                            .relative()
                            .w_full()
                            .h(px(220.0))
                            .rounded_md()
                            .bg(rgb(0x161616))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                    this.touching = true;
                                    this.push_trail(event.position);
                                    this.push_log(format!(
                                        "down @ ({:.0},{:.0})",
                                        f32::from(event.position.x),
                                        f32::from(event.position.y)
                                    ));
                                    cx.notify();
                                }),
                            )
                            .on_mouse_move(cx.listener(
                                |this, event: &MouseMoveEvent, _window, cx| {
                                    if this.touching {
                                        this.push_trail(event.position);
                                        this.push_log(format!(
                                            "move @ ({:.0},{:.0})",
                                            f32::from(event.position.x),
                                            f32::from(event.position.y)
                                        ));
                                        cx.notify();
                                    }
                                },
                            ))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseUpEvent, _window, cx| {
                                    this.touching = false;
                                    this.push_log(format!(
                                        "up @ ({:.0},{:.0})",
                                        f32::from(event.position.x),
                                        f32::from(event.position.y)
                                    ));
                                    cx.notify();
                                }),
                            )
                            .children(self.trail.iter().enumerate().map(|(i, pos)| {
                                let fade = (i + 1) as f32 / TRAIL_CAP as f32;
                                let size = 6.0 + fade * 10.0;
                                div()
                                    .absolute()
                                    .left(px(f32::from(pos.x) - size / 2.0))
                                    .top(px(f32::from(pos.y) - size / 2.0))
                                    .w(px(size))
                                    .h(px(size))
                                    .rounded_full()
                                    .bg(rgb(0x7fb0ff))
                                    .opacity(fade)
                            })),
                    ),
            )
            .child(
                section("Draggable box")
                    .child(note(
                        "Press and drag the small box below — it should follow your \
                         finger and stay put when released.",
                    ))
                    .child(
                        div()
                            .id("drag-area")
                            .relative()
                            .w_full()
                            .h(px(160.0))
                            .rounded_md()
                            .bg(rgb(0x161616))
                            .on_mouse_move(cx.listener(
                                |this, event: &MouseMoveEvent, _window, cx| {
                                    if this.dragging {
                                        this.drag_pos = event.position;
                                        cx.notify();
                                    }
                                },
                            ))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                    this.dragging = false;
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .id("drag-box")
                                    .absolute()
                                    .left(px(f32::from(self.drag_pos.x) - 24.0))
                                    .top(px(f32::from(self.drag_pos.y) - 24.0))
                                    .w(px(48.0))
                                    .h(px(48.0))
                                    .rounded_md()
                                    .bg(rgb(0xff9a3c))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                            this.dragging = true;
                                            this.drag_pos = event.position;
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
            .child(
                section("Pinch (multi-touch) attempt")
                    .child(note(
                        "Try pinching with two fingers in the box below. Expected: \
                         currently NO pinch is ever observed — gpui_mobile's iOS \
                         window forwards touches through a single-touch state machine \
                         (see crates/gpui_mobile/src/ios/window.rs: `TouchState` enum \
                         and the single `mouse_position`/`touch_pressed` Cell fields \
                         around line 340-360), so only one finger is ever tracked and \
                         no PinchEvent is ever synthesized on this platform.",
                    ))
                    .child(
                        div()
                            .id("pinch-area")
                            .w_full()
                            .h(px(120.0))
                            .rounded_md()
                            .bg(rgb(0x161616))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(0x9aa0a6))
                            .on_pinch(cx.listener(|this, _: &PinchEvent, _window, cx| {
                                this.pinch_observed = true;
                                gallery_log::push("gestures: pinch observed (unexpected!)");
                                cx.notify();
                            }))
                            .child("pinch here"),
                    )
                    .child(kv("pinch ever observed", self.pinch_observed.to_string())),
            )
            .child(
                section("Recent event log (last 10)").child(
                    div()
                        .id("gesture-event-log")
                        .h(px(160.0))
                        .overflow_y_scroll()
                        .on_scroll_wheel(cx.listener(
                            |this, event: &ScrollWheelEvent, _window, cx| {
                                this.push_log(format!(
                                    "scroll @ ({:.0},{:.0}) delta={:?} phase={:?}",
                                    f32::from(event.position.x),
                                    f32::from(event.position.y),
                                    event.delta,
                                    event.touch_phase
                                ));
                                cx.notify();
                            },
                        ))
                        .child(super::common::mono(if log_text.is_empty() {
                            "(no events yet)".to_string()
                        } else {
                            log_text
                        })),
                ),
            )
    }
}
