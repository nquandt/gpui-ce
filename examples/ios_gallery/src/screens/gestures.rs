//! "Gestures & touch" screen: finger position + trail, a draggable box, a
//! pinch-detection area (expected to never fire today), a note on raw
//! multi-touch delivery, and an event log of every recognized gesture kind.

use super::ScreenDescriptor;
use super::common::{kv, note, section};
use crate::log as gallery_log;
use gpui::{
    AnyView, App, Bounds, ClickEvent, Context, DispatchPhase, Entity, LongPressEvent, PinchEvent,
    Pixels, Point, ScrollWheelEvent, TouchDragEvent, TouchPhase, Window, canvas, div, prelude::*,
    px, rgb,
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
        drag_pos: Point::new(px(20.0), px(20.0)),
        pinch_observed: false,
        event_log: VecDeque::new(),
    })
    .into()
}

const TRAIL_CAP: usize = 30;
const LOG_CAP: usize = 10;

struct GesturesScreen {
    trail: VecDeque<Point<gpui::Pixels>>,
    drag_pos: Point<gpui::Pixels>,
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

/// Wraps `content` in an absolutely-positioned overlay whose bounds scope a
/// [`TouchDragEvent`] listener registered via [`Window::on_mouse_event`]
/// (`crates/gpui/src/window.rs:5120`), following the same pattern as
/// `buttons.rs`'s `long_press_area` (`div`/`Interactivity` has no fluent
/// `on_touch_drag` helper either — checked
/// `crates/gpui/src/elements/div.rs` and `crates/gpui/src/interactive.rs`).
///
/// `TouchDragEvent::start_position` is constant across the whole gesture
/// (`crates/gpui/src/gestures.rs:386-393`), so testing it against `bounds`
/// on every phase — not just `Started` — is enough to route `Moved`/`Ended`/
/// `Cancelled` back to whichever overlay's `Started` originally claimed the
/// gesture, since `Window::on_mouse_event` listeners are not hitbox-scoped
/// the way `div`'s mouse listeners are (see `dispatch_recognized_touch_gesture`'s
/// `TouchDrag` arm, `crates/gpui/src/window.rs:5539-5590`: `Started` is only
/// kept as a drag if the listener calls `window.prevent_default()`, via
/// `resolve_touch_drag`).
fn touch_drag_area<E: IntoElement>(
    content: E,
    on_event: impl Fn(&TouchDragEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .relative()
        .child(content)
        .child(div().absolute().inset_0().child(canvas(
            |_bounds, _window, _cx| {},
            move |bounds: Bounds<Pixels>, _state, window, _cx| {
                window.on_mouse_event(
                    move |event: &TouchDragEvent, phase, window: &mut Window, cx: &mut App| {
                        if phase != DispatchPhase::Bubble || !bounds.contains(&event.start_position)
                        {
                            return;
                        }
                        if event.phase == TouchPhase::Started {
                            window.prevent_default();
                        }
                        on_event(event, window, cx);
                    },
                );
            },
        )))
}

/// Same low-level pattern as `buttons.rs`'s `long_press_area`: `div` has no
/// fluent `on_long_press`, so the `Started` phase is claimed via
/// `Window::capture_long_press` + `Window::prevent_default()` and the
/// listener is registered on the window during the `canvas` overlay's paint
/// callback (`crates/gpui/src/window.rs:5120,2867`).
fn long_press_area<E: IntoElement>(
    content: E,
    entity: Entity<GesturesScreen>,
    on_event: impl Fn(&LongPressEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .relative()
        .child(content)
        .child(div().absolute().inset_0().child(canvas(
            |_bounds, _window, _cx| {},
            move |bounds: Bounds<Pixels>, _state, window, _cx| {
                let entity = entity.clone();
                window.on_mouse_event(
                    move |event: &LongPressEvent, phase, window: &mut Window, cx: &mut App| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        if event.phase == TouchPhase::Started {
                            if !bounds.contains(&event.start_position) {
                                return;
                            }
                            window.capture_long_press(&entity);
                            window.prevent_default();
                        } else if !window.has_long_press_capture(&entity) {
                            return;
                        }
                        on_event(event, window, cx);
                    },
                );
            },
        )))
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
        let entity = cx.entity();

        div()
            .id("gestures-scroll")
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
                    .child("Gestures & touch"),
            )
            .child(note(
                "Single-finger gestures (tap, pan/scroll, long-press, drag) are \
                 recognized by GPUI core's `TouchGestureRecognizer` \
                 (`crates/gpui/src/gestures.rs`) from the raw per-finger \
                 `PlatformInput::Touch` events the iOS backend now forwards \
                 (`crates/gpui_mobile/src/ios/window.rs::handle_touch`). \
                 Multi-touch (a second finger down) is now delivered to GPUI \
                 too, but the core recognizer does not implement pinch yet — \
                 see the note on the pinch section below.",
            ))
            .child(
                section("Touch position + trail")
                    .child(note(
                        "Drag your finger inside the box — a circle should follow it \
                         with a fading trail of the last 30 positions. Built on \
                         `TouchDragEvent` (`crates/gpui/src/gestures.rs:386`): the \
                         trail is pushed on `Started`/`Moved` and stops on \
                         `Ended`/`Cancelled`. Note there is no `MouseMoveEvent` \
                         synthesized during a touch pan/drag, so this can no \
                         longer be built on `on_mouse_move`.",
                    ))
                    .child(touch_drag_area(
                        div()
                            .id("touch-trail-area")
                            .relative()
                            .w_full()
                            .h(px(220.0))
                            .rounded_md()
                            .bg(rgb(0x161616))
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
                        cx.listener(|this, event: &TouchDragEvent, _window, cx| {
                            match event.phase {
                                TouchPhase::Started => {
                                    this.push_trail(event.position);
                                    this.push_log(format!(
                                        "touch-drag Started @ ({:.0},{:.0})",
                                        f32::from(event.position.x),
                                        f32::from(event.position.y)
                                    ));
                                }
                                TouchPhase::Moved => {
                                    this.push_trail(event.position);
                                    this.push_log(format!(
                                        "touch-drag Moved @ ({:.0},{:.0})",
                                        f32::from(event.position.x),
                                        f32::from(event.position.y)
                                    ));
                                }
                                TouchPhase::Ended => {
                                    this.push_log("touch-drag Ended".to_string());
                                }
                                TouchPhase::Cancelled => {
                                    this.push_log("touch-drag Cancelled".to_string());
                                }
                            }
                            cx.notify();
                        }),
                    )),
            )
            .child(
                section("Draggable box")
                    .child(note(
                        "Press and drag the small box below — it should follow your \
                         finger and stay put when released or cancelled. Built on \
                         `TouchDragEvent`: the overlay is sized to the box itself, so \
                         only a `Started` whose `start_position` lands on the box \
                         claims the gesture (via `window.prevent_default()`); \
                         `Moved` then repositions the box and `Ended`/`Cancelled` \
                         leave it in place.",
                    ))
                    .child(
                        div()
                            .id("drag-area")
                            .relative()
                            .w_full()
                            .h(px(160.0))
                            .rounded_md()
                            .bg(rgb(0x161616))
                            .child(touch_drag_area(
                                div()
                                    .id("drag-box")
                                    .absolute()
                                    .left(px(f32::from(self.drag_pos.x) - 24.0))
                                    .top(px(f32::from(self.drag_pos.y) - 24.0))
                                    .w(px(48.0))
                                    .h(px(48.0))
                                    .rounded_md()
                                    .bg(rgb(0xff9a3c)),
                                cx.listener(|this, event: &TouchDragEvent, _window, cx| {
                                    match event.phase {
                                        TouchPhase::Started | TouchPhase::Moved => {
                                            this.drag_pos = event.position;
                                        }
                                        TouchPhase::Ended | TouchPhase::Cancelled => {}
                                    }
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(
                section("Long press (for the event log)")
                    .child(note(
                        "Press and hold in the box below — the event log at the \
                         bottom shows each `LongPressEvent` phase \
                         (`crates/gpui/src/gestures.rs:406`) as it arrives.",
                    ))
                    .child(long_press_area(
                        div()
                            .id("gestures-long-press-target")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(160.0))
                            .h(px(64.0))
                            .bg(rgb(0x2c2c34))
                            .rounded_md()
                            .text_color(rgb(0xffffff))
                            .child("Hold me"),
                        entity,
                        cx.listener(|this, event: &LongPressEvent, _window, cx| {
                            this.push_log(format!("long-press {:?}", event.phase));
                            cx.notify();
                        }),
                    )),
            )
            .child(
                section("Raw multi-touch (`PlatformInput::Touch`)").child(note(
                    "Each finger's raw touch is forwarded from iOS as its own \
                         `PlatformInput::Touch(TouchEvent)`, but `Window::dispatch_event` \
                         routes that variant straight into the portable gesture \
                         recognizer (`dispatch_touch_event`, \
                         `crates/gpui/src/window.rs:5397-5422,5500`) instead of through \
                         the ordinary mouse-event path — `TouchEvent` has no \
                         `MouseEvent`/`impl InputEvent::mouse_event` — so elements \
                         cannot subscribe to raw touches directly the way they can to \
                         `LongPressEvent`/`TouchDragEvent`/`ScrollWheelEvent`. A second \
                         finger down is still ignored by today's single-touch \
                         recognizer (see `TouchGestureRecognizer`'s doc comment, \
                         `crates/gpui/src/gestures.rs:488`: \"Pinch recognition is not \
                         implemented yet, and additional touches are ignored\").",
                )),
            )
            .child(
                section("Pinch (multi-touch) attempt")
                    .child(note(
                        "Try pinching with two fingers in the box below. Expected: \
                         currently NO pinch is ever observed. The iOS backend does now \
                         forward every finger's touches to GPUI \
                         (`crates/gpui_mobile/src/ios/window.rs::handle_touch`), but \
                         GPUI core's `TouchGestureRecognizer` only tracks a single \
                         touch and does not implement pinch recognition yet \
                         (`crates/gpui/src/gestures.rs:488`), so no `PinchEvent` is \
                         ever synthesized regardless of platform.",
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
                section("Recent event log (last 10)")
                    .child(note(
                        "Shows every gesture kind GPUI can currently recognize: touch \
                         drag phases, long press phases, scroll wheel (with \
                         `touch_phase`), click (`ClickEvent::Touch` vs `Mouse`), and \
                         pinch (not expected to appear).",
                    ))
                    .child(
                        div()
                            .id("gesture-event-log")
                            .h(px(160.0))
                            .overflow_y_scroll()
                            .restrict_scroll_to_axis()
                            .on_scroll_wheel(cx.listener(
                                |this, event: &ScrollWheelEvent, _window, cx| {
                                    this.push_log(format!(
                                        "scroll @ ({:.0},{:.0}) delta={:?} touch_phase={:?}",
                                        f32::from(event.position.x),
                                        f32::from(event.position.y),
                                        event.delta,
                                        event.touch_phase
                                    ));
                                    cx.notify();
                                },
                            ))
                            .on_click(cx.listener(|this, event: &ClickEvent, _window, cx| {
                                let kind = match event {
                                    ClickEvent::Mouse(_) => "Mouse",
                                    ClickEvent::Touch(_) => "Touch",
                                    ClickEvent::Keyboard(_) => "Keyboard",
                                };
                                this.push_log(format!(
                                    "click kind={kind} count={}",
                                    event.click_count()
                                ));
                                cx.notify();
                            }))
                            .child(super::common::mono(if log_text.is_empty() {
                                "(no events yet)".to_string()
                            } else {
                                log_text
                            })),
                    ),
            )
    }
}
