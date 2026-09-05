//! "Appearance": shows the current window appearance (light/dark), counts
//! `Window::observe_window_appearance` firings, and lets the tester force
//! the status bar style via `gpui_mobile::set_system_chrome`.

use super::ScreenDescriptor;
use super::common::{button, kv, note, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, WindowAppearance, div, prelude::*, px, rgb};
use gpui_mobile::{StatusBarContentStyle, SystemChromeStyle};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "appearance",
        title: "Appearance",
        category: "Window & system",
        blurb: "light/dark, status bar style, thermal",
        build,
    }
}

fn build(window: &mut Window, cx: &mut App) -> AnyView {
    let appearance = window.appearance();
    cx.new(|cx| {
        let subscription =
            cx.observe_window_appearance(window, |this: &mut AppearanceScreen, window, cx| {
                this.change_count += 1;
                this.last_appearance = window.appearance();
                gallery_log::push(format!(
                    "appearance: changed to {:?} (change #{})",
                    this.last_appearance, this.change_count
                ));
                cx.notify();
            });
        AppearanceScreen {
            last_appearance: appearance,
            change_count: 0,
            status: String::new(),
            _subscription: subscription,
        }
    })
    .into()
}

struct AppearanceScreen {
    last_appearance: WindowAppearance,
    change_count: u32,
    status: String,
    _subscription: gpui::Subscription,
}

fn set_chrome(style: StatusBarContentStyle, label: &'static str) -> impl Fn() -> String {
    move || {
        let chrome = SystemChromeStyle {
            status_bar_style: style,
            ..Default::default()
        };
        gpui_mobile::set_system_chrome(&chrome);
        format!("set_system_chrome: status bar -> {label}")
    }
}

impl Render for AppearanceScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("appearance-scroll")
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
                    .child("Appearance"),
            )
            .child(note(
                "Toggle Dark Mode in Control Center (or Settings > Display & \
                 Brightness) and watch the appearance readout and change \
                 counter below update.",
            ))
            .child(
                section("Window appearance")
                    .child(kv("current", format!("{:?}", self.last_appearance)))
                    .child(kv(
                        "observe_window_appearance fires",
                        self.change_count.to_string(),
                    )),
            )
            .child(
                section("Force status bar style (gpui_mobile::set_system_chrome)")
                    .child(note(
                        "These only change the status bar's content style, not \
                         the app's own color scheme — the window appearance \
                         readout above tracks the system, not these buttons.",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(button(
                                "Light status bar",
                                cx.listener(|this, _, _window, cx| {
                                    this.status =
                                        set_chrome(StatusBarContentStyle::Light, "Light")();
                                    gallery_log::push(this.status.clone());
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "Dark status bar",
                                cx.listener(|this, _, _window, cx| {
                                    this.status = set_chrome(StatusBarContentStyle::Dark, "Dark")();
                                    gallery_log::push(this.status.clone());
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(if self.status.is_empty() {
                        div()
                    } else {
                        div().child(note(self.status.clone()))
                    }),
            )
    }
}
