//! "Microphone": permission, recording lifecycle with a live amplitude bar,
//! and playback of the resulting recording through the `audio` package.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::packages::audio::AudioPlayer;
use gpui_mobile::packages::microphone::{self, RecordingConfig};
use gpui_mobile::packages::permission_handler::{self, Permission, PermissionStatus};
use std::time::{Duration, Instant};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "microphone",
        title: "Microphone",
        category: "Hardware & media",
        blurb: "record, amplitude, playback",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| MicrophoneScreen {
        is_available: microphone::is_available(),
        recording: false,
        paused: false,
        amplitude: 0.0,
        started_at: None,
        elapsed_ms: 0,
        status: String::new(),
        last_recording: None,
        player: None,
        player_status: String::new(),
    })
    .into()
}

struct MicrophoneScreen {
    is_available: bool,
    recording: bool,
    paused: bool,
    amplitude: f64,
    started_at: Option<Instant>,
    elapsed_ms: u64,
    status: String,
    last_recording: Option<microphone::Recording>,
    player: Option<AudioPlayer>,
    player_status: String,
}

fn permission_label(p: PermissionStatus) -> &'static str {
    match p {
        PermissionStatus::Granted => "Granted",
        PermissionStatus::Denied => "Denied",
        PermissionStatus::Restricted => "Restricted",
        PermissionStatus::PermanentlyDenied => "PermanentlyDenied",
        PermissionStatus::Limited => "Limited",
        PermissionStatus::Provisional => "Provisional",
    }
}

impl MicrophoneScreen {
    fn start_amplitude_poll(&self, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let result = weak.update(cx, |this: &mut MicrophoneScreen, cx| {
                    if !this.recording {
                        return false;
                    }
                    this.amplitude = microphone::get_amplitude().unwrap_or(0.0);
                    if let Some(started) = this.started_at {
                        this.elapsed_ms = started.elapsed().as_millis() as u64;
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
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let config = RecordingConfig {
            format: microphone::AudioFormat::Wav,
            ..Default::default()
        };
        match microphone::start_recording(&config) {
            Ok(path) => {
                self.recording = true;
                self.paused = false;
                self.started_at = Some(Instant::now());
                self.elapsed_ms = 0;
                self.status = format!("recording -> {path}");
                gallery_log::push(format!("microphone: started recording -> {path}"));
                self.start_amplitude_poll(cx);
            }
            Err(e) => self.status = format!("start_recording error: {e}"),
        }
        cx.notify();
    }

    fn pause(&mut self, cx: &mut Context<Self>) {
        match microphone::pause_recording() {
            Ok(()) => {
                self.paused = true;
                self.status = "paused".into();
            }
            Err(e) => self.status = format!("pause_recording error: {e}"),
        }
        cx.notify();
    }

    fn resume(&mut self, cx: &mut Context<Self>) {
        match microphone::resume_recording() {
            Ok(()) => {
                self.paused = false;
                self.status = "resumed".into();
            }
            Err(e) => self.status = format!("resume_recording error: {e}"),
        }
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.recording = false;
        self.paused = false;
        match microphone::stop_recording() {
            Ok(rec) => {
                self.status = format!("saved {} ({}ms)", rec.path, rec.duration_ms);
                gallery_log::push(format!("microphone: stopped -> {}", rec.path));
                self.last_recording = Some(rec);
            }
            Err(e) => self.status = format!("stop_recording error: {e}"),
        }
        cx.notify();
    }

    fn play_last(&mut self, cx: &mut Context<Self>) {
        let Some(rec) = self.last_recording.clone() else {
            self.player_status = "no recording yet".into();
            cx.notify();
            return;
        };
        match AudioPlayer::new() {
            Ok(player) => match player.set_file_path(&rec.path) {
                Ok(_) => {
                    let _ = player.play();
                    self.player_status = format!("playing {}", rec.path);
                    self.player = Some(player);
                    gallery_log::push("microphone: playing back recording");
                }
                Err(e) => self.player_status = format!("set_file_path error: {e}"),
            },
            Err(e) => self.player_status = format!("AudioPlayer::new error: {e}"),
        }
        cx.notify();
    }
}

/// Amplitude bar: fills left-to-right with `amplitude` in `[0.0, 1.0]`.
fn amplitude_bar(amplitude: f64) -> impl IntoElement {
    div()
        .h(px(14.0))
        .w_full()
        .bg(rgb(0x333333))
        .rounded_sm()
        .child(
            div()
                .h_full()
                .w(gpui::relative(amplitude.clamp(0.0, 1.0) as f32))
                .bg(rgb(0x5ad57a))
                .rounded_sm(),
        )
}

impl Render for MicrophoneScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let permission = permission_handler::check_permission(Permission::Microphone);
        let player_state = self
            .player
            .as_ref()
            .map(|p| {
                let state = p
                    .state()
                    .map(|s| format!("{s:?}"))
                    .unwrap_or_else(|e| format!("error: {e}"));
                let position = p.position().unwrap_or(0);
                let duration = p.duration().unwrap_or(0);
                format!("{state} ({position}ms / {duration}ms)")
            })
            .unwrap_or_else(|| "(none)".into());

        div()
            .id("microphone-scroll")
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
                    .child("Microphone"),
            )
            .child(note(
                "Record with the device microphone, watch the live amplitude bar, then play back \
                 the result.",
            ))
            .child(
                section("availability / permission")
                    .child(kv("is_available()", self.is_available.to_string()))
                    .child(kv(
                        "microphone permission",
                        permission
                            .map(permission_label)
                            .unwrap_or("error")
                            .to_string(),
                    ))
                    .child(button(
                        "Request microphone",
                        cx.listener(|this, _, _window, cx| {
                            let result =
                                permission_handler::request_permission(Permission::Microphone);
                            this.status = match result {
                                Ok(s) => format!("permission -> {}", permission_label(s)),
                                Err(e) => format!("error: {e}"),
                            };
                            gallery_log::push(format!(
                                "microphone: request permission -> {}",
                                this.status
                            ));
                            cx.notify();
                        }),
                    )),
            )
            .child(
                section("recording")
                    .child(
                        row()
                            .child(button(
                                "Start",
                                cx.listener(|this, _, _window, cx| this.start(cx)),
                            ))
                            .child(button(
                                "Pause",
                                cx.listener(|this, _, _window, cx| this.pause(cx)),
                            ))
                            .child(button(
                                "Resume",
                                cx.listener(|this, _, _window, cx| this.resume(cx)),
                            ))
                            .child(button(
                                "Stop",
                                cx.listener(|this, _, _window, cx| this.stop(cx)),
                            )),
                    )
                    .child(kv(
                        "state",
                        if self.recording {
                            if self.paused { "paused" } else { "recording" }
                        } else {
                            "stopped"
                        },
                    ))
                    .child(kv(
                        "elapsed",
                        format!("{:.1}s", self.elapsed_ms as f64 / 1000.0),
                    ))
                    .child(amplitude_bar(self.amplitude))
                    .child(kv("amplitude", format!("{:.3}", self.amplitude)))
                    .child(note(self.status.clone())),
            )
            .child(
                section("playback")
                    .child(button(
                        "Play last recording",
                        cx.listener(|this, _, _window, cx| this.play_last(cx)),
                    ))
                    .child(kv("player state", player_state))
                    .child(note(self.player_status.clone())),
            )
    }
}
