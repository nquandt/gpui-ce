//! "Sensors": live accelerometer / gyroscope / magnetometer bar graphs,
//! start/stop polling, sample-rate readout, and an in_app_review request.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::packages::{in_app_review, sensors};
use std::time::{Duration, Instant};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "sensors",
        title: "Sensors",
        category: "Hardware & media",
        blurb: "accelerometer, gyroscope, magnetometer",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| SensorsScreen {
        running: false,
        availability: sensors::available_sensors(),
        accel: None,
        gyro: None,
        mag: None,
        sample_count: 0,
        rate_started_at: None,
        sample_rate_hz: 0.0,
        review_status: String::new(),
    })
    .into()
}

struct SensorsScreen {
    running: bool,
    availability: sensors::SensorAvailability,
    accel: Option<sensors::SensorData>,
    gyro: Option<sensors::SensorData>,
    mag: Option<sensors::SensorData>,
    sample_count: u32,
    rate_started_at: Option<Instant>,
    sample_rate_hz: f64,
    review_status: String,
}

impl SensorsScreen {
    fn start(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.running = true;
        self.sample_count = 0;
        self.rate_started_at = Some(Instant::now());
        gallery_log::push("sensors: started polling");
        let weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let result = weak.update(cx, |this: &mut SensorsScreen, cx| {
                    if !this.running {
                        return false;
                    }
                    this.accel = sensors::accelerometer();
                    this.gyro = sensors::gyroscope();
                    this.mag = sensors::magnetometer();
                    this.sample_count += 1;
                    if let Some(start) = this.rate_started_at {
                        let elapsed = start.elapsed().as_secs_f64();
                        if elapsed > 0.0 {
                            this.sample_rate_hz = this.sample_count as f64 / elapsed;
                        }
                    }
                    cx.notify();
                    true
                });
                match result {
                    Ok(true) => continue,
                    _ => break,
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.running = false;
        gallery_log::push("sensors: stopped polling");
        cx.notify();
    }
}

/// A horizontal bar whose fill width follows `value` scaled into `[-max, max]`,
/// centered at 50%.
fn bar_graph(label: &str, value: Option<f64>, max: f64) -> impl IntoElement {
    let (text, frac) = match value {
        Some(v) => (
            format!("{label}: {v:.3}"),
            ((v / max).clamp(-1.0, 1.0) + 1.0) / 2.0,
        ),
        None => (format!("{label}: (unavailable)"), 0.5),
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_color(rgb(0xffffff))
                .text_size(px(12.0))
                .child(text),
        )
        .child(
            div()
                .h(px(10.0))
                .w_full()
                .bg(rgb(0x333333))
                .rounded_sm()
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(frac as f32))
                        .bg(rgb(0x5a9bd5))
                        .rounded_sm(),
                ),
        )
}

fn axis_bars(label: &str, data: Option<sensors::SensorData>, max: f64) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_color(rgb(0xd0d0d0))
                .text_size(px(13.0))
                .child(label.to_string()),
        )
        .child(bar_graph("x", data.map(|d| d.x), max))
        .child(bar_graph("y", data.map(|d| d.y), max))
        .child(bar_graph("z", data.map(|d| d.z), max))
}

impl Render for SensorsScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sensors-scroll")
            .overflow_y_scroll()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(div().text_color(rgb(0xffffff)).text_size(px(20.0)).child("Sensors"))
            .child(note(
                "Tap Start to poll the motion sensors 10x/sec. Bars fill from center; \
                 full bar = clamped at the scale shown per sensor.",
            ))
            .child(
                section("availability")
                    .child(kv("accelerometer", self.availability.accelerometer.to_string()))
                    .child(kv("gyroscope", self.availability.gyroscope.to_string()))
                    .child(kv("magnetometer", self.availability.magnetometer.to_string()))
                    .child(kv("barometer", self.availability.barometer.to_string())),
            )
            .child(
                section("live readings")
                    .child(axis_bars("Accelerometer (m/s^2)", self.accel, 20.0))
                    .child(axis_bars("Gyroscope (rad/s)", self.gyro, 10.0))
                    .child(axis_bars("Magnetometer (uT)", self.mag, 100.0))
                    .child(kv("samples", self.sample_count.to_string()))
                    .child(kv("sample rate", format!("{:.1} Hz", self.sample_rate_hz)))
                    .child(row().child(if self.running {
                        button(
                            "Stop",
                            cx.listener(|this, _, _window, cx| this.stop(cx)),
                        )
                    } else {
                        button(
                            "Start",
                            cx.listener(|this, _, _window, cx| this.start(cx)),
                        )
                    })),
            )
            .child(
                section("in_app_review")
                    .child(note("Requests the App Store review sheet. iOS rate-limits this and gives no signal on whether it was actually shown."))
                    .child(row()
                        .child(button(
                            "Check availability",
                            cx.listener(|this, _, _window, cx| {
                                this.review_status = match in_app_review::is_available() {
                                    Ok(v) => format!("is_available: {v}"),
                                    Err(e) => format!("error: {e}"),
                                };
                                gallery_log::push(format!("sensors: in_app_review is_available -> {}", this.review_status));
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Request review",
                            cx.listener(|this, _, _window, cx| {
                                this.review_status = match in_app_review::request_review() {
                                    Ok(()) => "requested (no shown/not-shown signal from OS)".into(),
                                    Err(e) => format!("error: {e}"),
                                };
                                gallery_log::push(format!("sensors: in_app_review request -> {}", this.review_status));
                                cx.notify();
                            }),
                        )))
                    .child(if self.review_status.is_empty() {
                        div()
                    } else {
                        div().child(note(self.review_status.clone()))
                    }),
            )
    }
}
