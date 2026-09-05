//! "Haptics & vibration": buttons for `cx.play_haptic_feedback`'s three
//! `HapticFeedbackStyle` variants, `cx.supports_haptic_feedback()`, and the
//! `vibration` package's `vibrate`/`haptic_feedback`/`can_vibrate`.
//!
//! NOTE: the brief this screen was built against also asked for `pattern`
//! and `cancel` functions on the `vibration` package; neither exists in
//! `crates/gpui_mobile/src/packages/vibration/mod.rs` (only `vibrate`,
//! `haptic_feedback`, `can_vibrate`) — that's a gap noted below rather than
//! silently working around.

use super::ScreenDescriptor;
use super::common::{button, mono, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, HapticFeedbackStyle, Window, div, prelude::*, px, rgb};
use gpui_mobile::packages::vibration::{self, HapticFeedback};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "haptics",
        title: "Haptics & vibration",
        category: "Window & system",
        blurb: "impact/selection/notification, vibrate",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| HapticsScreen {
        status: String::new(),
    })
    .into()
}

struct HapticsScreen {
    status: String,
}

impl Render for HapticsScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let supported = cx.supports_haptic_feedback();

        div()
            .id("haptics-scroll")
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
                    .child("Haptics & vibration"),
            )
            .child(note(
                "Expect actual feedback only on a real device — the \
                 simulator has no Taptic Engine, so these buttons will \
                 report their call succeeded even though nothing is felt.",
            ))
            .child(
                section("cx.supports_haptic_feedback()")
                    .child(mono(format!("supports_haptic_feedback() -> {supported}"))),
            )
            .child(
                section("cx.play_haptic_feedback (core HapticFeedbackStyle)")
                    .child(
                        row()
                            .flex_wrap()
                            .child(button(
                                "Generic",
                                cx.listener(|this, _, _window, cx| {
                                    cx.play_haptic_feedback(HapticFeedbackStyle::Generic);
                                    this.status = "play_haptic_feedback(Generic) called.".into();
                                    gallery_log::push("haptics: Generic");
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "Alignment",
                                cx.listener(|this, _, _window, cx| {
                                    cx.play_haptic_feedback(HapticFeedbackStyle::Alignment);
                                    this.status = "play_haptic_feedback(Alignment) called.".into();
                                    gallery_log::push("haptics: Alignment");
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "LevelChange",
                                cx.listener(|this, _, _window, cx| {
                                    cx.play_haptic_feedback(HapticFeedbackStyle::LevelChange);
                                    this.status =
                                        "play_haptic_feedback(LevelChange) called.".into();
                                    gallery_log::push("haptics: LevelChange");
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(if self.status.is_empty() {
                        note("(nothing played yet)")
                    } else {
                        mono(self.status.clone())
                    }),
            )
            .child(
                section("vibration package")
                    .child(mono(format!(
                        "can_vibrate() -> {}",
                        vibration::can_vibrate()
                    )))
                    .child(
                        row()
                            .flex_wrap()
                            .child(button(
                                "vibrate(200ms)",
                                cx.listener(|this, _, _window, cx| {
                                    this.status = match vibration::vibrate(200) {
                                        Ok(()) => "vibrate(200) -> Ok".into(),
                                        Err(e) => format!("vibrate(200) -> Err({e})"),
                                    };
                                    gallery_log::push(format!("haptics: {}", this.status));
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "Light",
                                cx.listener(|this, _, _window, cx| {
                                    run_pattern(this, cx, HapticFeedback::Light, "Light");
                                }),
                            ))
                            .child(button(
                                "Medium",
                                cx.listener(|this, _, _window, cx| {
                                    run_pattern(this, cx, HapticFeedback::Medium, "Medium");
                                }),
                            ))
                            .child(button(
                                "Heavy",
                                cx.listener(|this, _, _window, cx| {
                                    run_pattern(this, cx, HapticFeedback::Heavy, "Heavy");
                                }),
                            ))
                            .child(button(
                                "Selection",
                                cx.listener(|this, _, _window, cx| {
                                    run_pattern(this, cx, HapticFeedback::Selection, "Selection");
                                }),
                            ))
                            .child(button(
                                "Success",
                                cx.listener(|this, _, _window, cx| {
                                    run_pattern(this, cx, HapticFeedback::Success, "Success");
                                }),
                            ))
                            .child(button(
                                "Warning",
                                cx.listener(|this, _, _window, cx| {
                                    run_pattern(this, cx, HapticFeedback::Warning, "Warning");
                                }),
                            ))
                            .child(button(
                                "Error",
                                cx.listener(|this, _, _window, cx| {
                                    run_pattern(this, cx, HapticFeedback::Error, "Error");
                                }),
                            )),
                    )
                    .child(note(
                        "Gap: the vibration package has no `pattern`/`cancel` \
                         functions (only vibrate/haptic_feedback/can_vibrate) — \
                         see crates/gpui_mobile/src/packages/vibration/mod.rs.",
                    )),
            )
    }
}

fn run_pattern(
    this: &mut HapticsScreen,
    cx: &mut Context<HapticsScreen>,
    feedback: HapticFeedback,
    label: &'static str,
) {
    this.status = match vibration::haptic_feedback(feedback) {
        Ok(()) => format!("haptic_feedback({label}) -> Ok"),
        Err(e) => format!("haptic_feedback({label}) -> Err({e})"),
    };
    gallery_log::push(format!("haptics: {}", this.status));
    cx.notify();
}
