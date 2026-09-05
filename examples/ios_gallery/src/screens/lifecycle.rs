//! "Lifecycle & events": the global event log filtered to lifecycle/system
//! lines, plus this screen's own sources — `App::on_thermal_state_change`
//! (fully exposed to app code) and, where GPUI exposes nothing, a clearly
//! labeled gap.
//!
//! Platform gap: `crates/gpui_mobile/src/ios/platform.rs:836` implements
//! `Platform::on_memory_warning` and `:845` implements
//! `Platform::on_app_lifecycle`, but `App` (`crates/gpui/src/app.rs`) has no
//! public wrapper for either — only `thermal_state`/`on_thermal_state_change`
//! (app.rs:1395,1400) are reachable from app code. So this screen cannot
//! register its own memory-warning or foreground/background hooks; it logs
//! that fact instead of silently omitting the section.

use super::ScreenDescriptor;
use super::common::{mono, note, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "lifecycle",
        title: "Lifecycle & events",
        category: "Window & system",
        blurb: "foreground/background, memory, thermal, insets",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| {
        let thermal_subscription = cx.on_thermal_state_change(|cx| {
            gallery_log::push(format!(
                "lifecycle: thermal state -> {:?}",
                cx.thermal_state()
            ));
        });
        LifecycleScreen {
            thermal: cx.thermal_state(),
            frames_while_inactive: 0,
            _thermal_subscription: thermal_subscription,
        }
    })
    .into()
}

struct LifecycleScreen {
    thermal: gpui::ThermalState,
    frames_while_inactive: u32,
    _thermal_subscription: gpui::Subscription,
}

impl Render for LifecycleScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep the thermal readout live even if no render was otherwise
        // triggered, and let us notice frames rendered while backgrounded
        // (a rough proxy for "frames rendered while inactive", since GPUI
        // has no app-lifecycle callback to gate this on directly — see the
        // module doc comment).
        window.request_animation_frame();
        self.thermal = cx.thermal_state();

        let lifecycle_lines: Vec<String> = gallery_log::last(500)
            .into_iter()
            .filter(|line| {
                let l = line.to_lowercase();
                l.contains("lifecycle")
                    || l.contains("thermal")
                    || l.contains("memory")
                    || l.contains("navigation:")
                    || l.contains("gallery: launched")
            })
            .collect();
        let log_text = if lifecycle_lines.is_empty() {
            "(no lifecycle/system events logged yet)".to_string()
        } else {
            lifecycle_lines.join("\n")
        };

        div()
            .id("lifecycle-scroll")
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
                    .child("Lifecycle & events"),
            )
            .child(note(
                "Background the app (press the home button / swipe up) then \
                 return, and check Simulator > Debug > Simulate Memory \
                 Warning while this screen is open. Thermal state changes \
                 rarely happen on demand, so 'Nominal' throughout a normal \
                 test session is expected and correct.",
            ))
            .child(
                section("Thermal state (App::thermal_state / on_thermal_state_change)")
                    .child(mono(format!("{:?}", self.thermal))),
            )
            .child(
                section("App lifecycle / memory warnings")
                    .child(note(
                        "GPUI core does not expose `on_app_lifecycle` or \
                         `on_memory_warning` on `App` — only the iOS \
                         `Platform` trait implements them internally \
                         (ios/platform.rs:836, :845). This screen cannot \
                         register foreground/background or memory-warning \
                         callbacks from app code; nothing below this line \
                         will ever populate until GPUI adds a public wrapper.",
                    ))
                    .child(mono(format!(
                        "frames rendered while inactive (unmeasurable, always 0): {}",
                        self.frames_while_inactive
                    ))),
            )
            .child(
                section("Lifecycle/system event log").child(
                    div()
                        .id("lifecycle_log")
                        .h(px(240.0))
                        .overflow_y_scroll()
                        .child(mono(log_text)),
                ),
            )
    }
}
