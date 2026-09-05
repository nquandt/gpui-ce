//! "Buttons & taps" screen: tap counter, double tap, long press, disabled
//! button, mouse down/up ticker, haptics, and `window.prompt`.

use super::ScreenDescriptor;
use super::common::{button, kv, label, note, row, section};
use crate::log as gallery_log;
use gpui::{
    AnyView, App, Bounds, ClickEvent, Context, DispatchPhase, Entity, HapticFeedbackStyle,
    LongPressEvent, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, PromptLevel, TouchPhase,
    Window, canvas, div, prelude::*, px, rgb,
};
use std::collections::VecDeque;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "buttons",
        title: "Buttons & taps",
        category: "Core UI",
        blurb: "tap, double tap, long press, mouse down/up, disabled",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| ButtonsScreen {
        taps: 0,
        double_taps: 0,
        last_click_count: 0,
        last_click_kind: "none yet".into(),
        disabled_taps: 0,
        long_press_status: "idle".into(),
        mouse_events: VecDeque::new(),
        haptic_status: String::new(),
        prompt_status: "no answer yet".into(),
    })
    .into()
}

const MOUSE_EVENT_CAP: usize = 5;

struct ButtonsScreen {
    taps: u64,
    double_taps: u64,
    last_click_count: usize,
    last_click_kind: String,
    disabled_taps: u64,
    long_press_status: String,
    mouse_events: VecDeque<String>,
    haptic_status: String,
    prompt_status: String,
}

impl ButtonsScreen {
    fn push_mouse_event(&mut self, text: String) {
        if self.mouse_events.len() >= MOUSE_EVENT_CAP {
            self.mouse_events.pop_front();
        }
        self.mouse_events.push_back(text);
    }
}

/// Wraps `content` in an absolutely-positioned area whose bounds are used to
/// scope a [`LongPressEvent`] listener registered directly on the window via
/// [`Window::on_mouse_event`] (see `crates/gpui/src/window.rs:5120`).
///
/// `div`/`Interactivity` has no fluent `on_long_press` helper yet (checked
/// `crates/gpui/src/elements/div.rs` and `crates/gpui/src/interactive.rs`:
/// only `on_mouse_down`/`on_mouse_up`/`on_mouse_move`/`on_scroll_wheel`/
/// `on_drag`/`on_click`/`on_pinch` exist), so this screen registers the
/// listener itself using the low-level `canvas` element
/// (`crates/gpui/src/elements/canvas.rs`), whose `paint` callback runs during
/// the paint phase, which is required by `on_mouse_event`
/// (`Window::debug_assert_paint`, `crates/gpui/src/window.rs:280`).
///
/// The `Started` phase must be claimed with `window.capture_long_press` +
/// `window.prevent_default()` or the recognizer treats it as unclaimed and
/// will not deliver `Moved`/`Ended` (`crates/gpui/src/window.rs:5539-5590`
/// `dispatch_recognized_touch_gesture`, and `capture_long_press`/
/// `has_long_press_capture` around `crates/gpui/src/window.rs:2867`).
fn long_press_area<E: IntoElement>(
    content: E,
    entity: Entity<ButtonsScreen>,
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
                        let hit_position = if event.phase == TouchPhase::Started {
                            event.start_position
                        } else {
                            event.position
                        };
                        if event.phase == TouchPhase::Started {
                            if !bounds.contains(&hit_position) {
                                return;
                            }
                            // Claim the gesture so the recognizer keeps sending
                            // Moved/Ended/Cancelled to us instead of treating it
                            // as unclaimed.
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

impl Render for ButtonsScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mouse_log = self
            .mouse_events
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let entity = cx.entity();

        div()
            .id("buttons-scroll")
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
                    .child("Buttons & taps"),
            )
            .child(note(
                "Every control below states what it exercises and what result to expect.",
            ))
            .child(
                section("Tap counter")
                    .child(note(
                        "Tap the button — the count below should increment by 1 each tap.",
                    ))
                    .child(row().child(button(
                        "Tap me",
                        cx.listener(|this, _, _window, cx| {
                            this.taps += 1;
                            gallery_log::push(format!("buttons: tap counter -> {}", this.taps));
                            cx.notify();
                        }),
                    )))
                    .child(kv("taps", self.taps.to_string())),
            )
            .child(
                section("Double tap")
                    .child(note(
                        "Tap the box twice quickly — `click_count` should read 2 and \
                         'double taps' should increment. A single tap should not \
                         increment 'double taps'. On a touch device the tap arrives \
                         as `ClickEvent::Touch` (no modifiers, no first-click info); \
                         on a mouse/trackpad it arrives as `ClickEvent::Mouse`. The \
                         variant that fired is shown below.",
                    ))
                    .child(
                        div()
                            .id("double-tap-target")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(160.0))
                            .h(px(64.0))
                            .bg(rgb(0x2c2c34))
                            .rounded_md()
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|this, event: &ClickEvent, _window, cx| {
                                let count = event.click_count();
                                this.last_click_count = count;
                                this.last_click_kind = match event {
                                    ClickEvent::Mouse(_) => "Mouse".to_string(),
                                    ClickEvent::Touch(_) => "Touch".to_string(),
                                    ClickEvent::Keyboard(_) => "Keyboard".to_string(),
                                };
                                if count >= 2 {
                                    this.double_taps += 1;
                                }
                                gallery_log::push(format!(
                                    "buttons: click_count={count}, kind={}, double_taps={}",
                                    this.last_click_kind, this.double_taps
                                ));
                                cx.notify();
                            }))
                            .child("Tap x2"),
                    )
                    .child(kv("last click_count", self.last_click_count.to_string()))
                    .child(kv("last click kind", self.last_click_kind.clone()))
                    .child(kv("double taps", self.double_taps.to_string())),
            )
            .child(
                section("Long press")
                    .child(note(
                        "Press and hold without lifting — status should read \
                         'long-press recognized!' once GPUI's gesture recognizer \
                         fires `LongPressEvent::Started` (see \
                         `crates/gpui/src/gestures.rs:406`). Moving your finger while \
                         still holding shows 'moved'; lifting or having the touch \
                         cancelled (e.g. by the OS) shows 'released'/'cancelled'. \
                         Unlike the previous timer-based implementation, this is \
                         driven entirely by the core recognizer once the gesture is \
                         claimed via `Window::capture_long_press` + \
                         `Window::prevent_default()` on the Started phase.",
                    ))
                    .child(long_press_area(
                        div()
                            .id("long-press-target")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(160.0))
                            .h(px(64.0))
                            .bg(rgb(0x2c2c34))
                            .rounded_md()
                            .text_color(rgb(0xffffff))
                            .child("Hold me"),
                        entity.clone(),
                        cx.listener(|this, event: &LongPressEvent, _window, cx| {
                            this.long_press_status = match event.phase {
                                TouchPhase::Started => "long-press recognized!".to_string(),
                                TouchPhase::Moved => format!(
                                    "moved @ ({:.0}, {:.0})",
                                    f32::from(event.position.x),
                                    f32::from(event.position.y)
                                ),
                                TouchPhase::Ended => "released".to_string(),
                                TouchPhase::Cancelled => "cancelled".to_string(),
                            };
                            if event.phase == TouchPhase::Started {
                                gallery_log::push("buttons: long-press recognized");
                            }
                            cx.notify();
                        }),
                    ))
                    .child(kv("status", self.long_press_status.clone())),
            )
            .child(
                section("Disabled button")
                    .child(note(
                        "This button must NOT react to taps — the counter below should \
                         stay at 0 no matter how many times you tap it.",
                    ))
                    .child(
                        div()
                            .id("disabled-target")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(160.0))
                            .h(px(48.0))
                            .bg(rgb(0x3a3a3a))
                            .opacity(0.4)
                            .rounded_full()
                            .text_color(rgb(0x9aa0a6))
                            .cursor_not_allowed()
                            .child("Disabled"),
                    )
                    .child(kv("disabled taps", self.disabled_taps.to_string())),
            )
            .child(
                section("Mouse down / up ticker")
                    .child(note(
                        "Tap inside the box below. On a touch device GPUI does NOT \
                         synthesize a `MouseMoveEvent` during a pan/drag, and a tap's \
                         `MouseDown` is only delivered together with `MouseUp` at the \
                         moment the finger lifts (see \
                         `dispatch_recognized_touch_gesture`'s `Tap { down, up }` arm, \
                         `crates/gpui/src/window.rs:5539-5590`) — so on-device you \
                         should see 'down' and 'up' appear together, back to back, \
                         never a 'down' on its own while your finger is still \
                         resting. On a mouse/trackpad they still arrive separately.",
                    ))
                    .child(
                        div()
                            .id("mouse-ticker-target")
                            .w_full()
                            .h(px(96.0))
                            .bg(rgb(0x202028))
                            .rounded_md()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                    this.push_mouse_event(format!(
                                        "down  @ ({:.0}, {:.0})",
                                        f32::from(event.position.x),
                                        f32::from(event.position.y)
                                    ));
                                    cx.notify();
                                }),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseUpEvent, _window, cx| {
                                    this.push_mouse_event(format!(
                                        "up    @ ({:.0}, {:.0})",
                                        f32::from(event.position.x),
                                        f32::from(event.position.y)
                                    ));
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("mouse-event-log")
                            .h(px(96.0))
                            .overflow_y_scroll()
                            .child(super::common::mono(if mouse_log.is_empty() {
                                "(no events yet)".to_string()
                            } else {
                                mouse_log
                            })),
                    ),
            )
            .child(
                section("Haptic feedback")
                    .child(note(
                        "Tap each button — you should feel the corresponding haptic on a \
                         physical device (the simulator cannot vibrate).",
                    ))
                    .child(
                        row()
                            .flex_wrap()
                            .child(button(
                                "Generic",
                                cx.listener(|this, _, _window, cx| {
                                    cx.play_haptic_feedback(HapticFeedbackStyle::Generic);
                                    this.haptic_status = "played Generic".into();
                                    gallery_log::push("buttons: haptic Generic");
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "Alignment",
                                cx.listener(|this, _, _window, cx| {
                                    cx.play_haptic_feedback(HapticFeedbackStyle::Alignment);
                                    this.haptic_status = "played Alignment".into();
                                    gallery_log::push("buttons: haptic Alignment");
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "LevelChange",
                                cx.listener(|this, _, _window, cx| {
                                    cx.play_haptic_feedback(HapticFeedbackStyle::LevelChange);
                                    this.haptic_status = "played LevelChange".into();
                                    gallery_log::push("buttons: haptic LevelChange");
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(kv("last haptic", self.haptic_status.clone())),
            )
            .child(
                section("window.prompt")
                    .child(note(
                        "Tap 'Ask' — a native prompt with two answers ('Cats' / 'Dogs') \
                         should appear; whichever you choose should show up below.",
                    ))
                    .child(row().child(button(
                        "Ask",
                        cx.listener(|_this, _, window, cx| {
                            let receiver = window.prompt(
                                PromptLevel::Info,
                                "Cats or dogs?",
                                None,
                                &["Cats", "Dogs"],
                                cx,
                            );
                            cx.spawn(async move |this, cx| {
                                if let Ok(answer) = receiver.await {
                                    let Some(this) = this.upgrade() else { return };
                                    this.update(cx, |this, cx| {
                                        let label = if answer == 0 { "Cats" } else { "Dogs" };
                                        this.prompt_status = format!("chose: {label}");
                                        gallery_log::push(format!("buttons: prompt -> {label}"));
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        }),
                    )))
                    .child(kv("answer", self.prompt_status.clone()))
                    .child(label(if self.disabled_taps > 0 {
                        "warning: disabled button should not react!"
                    } else {
                        ""
                    })),
            )
    }
}
