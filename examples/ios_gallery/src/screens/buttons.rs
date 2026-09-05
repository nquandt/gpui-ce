//! "Buttons & taps" screen: tap counter, double tap, long press, disabled
//! button, mouse down/up/move ticker, haptics, and `window.prompt`.

use super::ScreenDescriptor;
use super::common::{button, kv, label, note, row, section};
use crate::log as gallery_log;
use gpui::{
    AnyView, App, ClickEvent, Context, HapticFeedbackStyle, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PromptLevel, Window, div, prelude::*, px, rgb,
};
use std::collections::VecDeque;
use std::time::Duration;

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
        disabled_taps: 0,
        long_press_generation: 0,
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
    disabled_taps: u64,
    /// Incremented on every mouse-down; a spawned 500ms timer only fires a
    /// long-press if the generation it captured is still current (i.e. the
    /// finger hasn't lifted or a new press hasn't started) when it wakes.
    long_press_generation: u64,
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

    fn start_long_press_timer(&mut self, cx: &mut Context<Self>) {
        self.long_press_generation = self.long_press_generation.wrapping_add(1);
        let generation = self.long_press_generation;
        self.long_press_status = "pressed — hold for 500ms…".into();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let Some(this) = this.upgrade() else { return };
            this.update(cx, |this, cx| {
                if this.long_press_generation == generation {
                    this.long_press_status = "long-press recognized!".into();
                    gallery_log::push("buttons: long-press recognized");
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn cancel_long_press_timer(&mut self, reason: &str) {
        // Bump the generation so the pending timer callback becomes a no-op.
        self.long_press_generation = self.long_press_generation.wrapping_add(1);
        self.long_press_status = reason.to_string();
    }
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
                         increment 'double taps'.",
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
                                if count >= 2 {
                                    this.double_taps += 1;
                                }
                                gallery_log::push(format!(
                                    "buttons: click_count={count}, double_taps={}",
                                    this.double_taps
                                ));
                                cx.notify();
                            }))
                            .child("Tap x2"),
                    )
                    .child(kv("last click_count", self.last_click_count.to_string()))
                    .child(kv("double taps", self.double_taps.to_string())),
            )
            .child(
                section("Long press")
                    .child(note(
                        "Press and hold for 500ms without lifting — status should read \
                         'long-press recognized!'. Lifting early should read 'released \
                         early'. Implemented with a timer since div has no fluent \
                         on_long_press yet (see gpui/src/gestures.rs: LongPressEvent is \
                         recognized internally but not wired to a div listener).",
                    ))
                    .child(
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
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                                    this.start_long_press_timer(cx);
                                    cx.notify();
                                }),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                    if this.long_press_status != "long-press recognized!" {
                                        this.cancel_long_press_timer("released early");
                                    }
                                    cx.notify();
                                }),
                            )
                            .child("Hold me"),
                    )
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
                section("Mouse down / up / move ticker")
                    .child(note(
                        "Touch and drag inside the box below — the log should show the \
                         last 5 down/up/move events with positions, most recent first.",
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
                            )
                            .on_mouse_move(cx.listener(
                                |this, event: &MouseMoveEvent, _window, cx| {
                                    this.push_mouse_event(format!(
                                        "move  @ ({:.0}, {:.0})",
                                        f32::from(event.position.x),
                                        f32::from(event.position.y)
                                    ));
                                    cx.notify();
                                },
                            )),
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
