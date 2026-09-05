//! "Media": video_player platform view, a synthesized + a remote audio
//! player through the `audio` package, and media_session probing.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::components::platform_view_element::platform_view_element;
use gpui_mobile::packages::audio::AudioPlayer;
use gpui_mobile::packages::video_player::VideoPlayer;
use gpui_mobile::packages::{media_session, path_provider};
use std::f32::consts::PI;
use std::time::Duration;

const VIDEO_URL: &str =
    "https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4";
const REMOTE_MP3_URL: &str = "https://www2.cs.uic.edu/~i101/SoundFiles/StarWars3.wav";

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "media",
        title: "Media",
        category: "Hardware & media",
        blurb: "video_player, audio, media_session",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| {
        let mut this = MediaScreen {
            video: None,
            video_status: String::new(),
            video_duration_ms: 0,
            video_playing: false,
            tone_audio: None,
            tone_status: String::new(),
            tone_playing: false,
            remote_audio: None,
            remote_status: String::new(),
            remote_playing: false,
            session_log: Vec::new(),
        };
        this.init_video();
        let weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if weak
                    .update(cx, |_this: &mut MediaScreen, cx| cx.notify())
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        this
    })
    .into()
}

struct MediaScreen {
    video: Option<VideoPlayer>,
    video_status: String,
    video_duration_ms: u64,
    video_playing: bool,
    tone_audio: Option<AudioPlayer>,
    tone_status: String,
    tone_playing: bool,
    remote_audio: Option<AudioPlayer>,
    remote_status: String,
    remote_playing: bool,
    session_log: Vec<String>,
}

/// `VideoPlayer::position/duration` and `AudioPlayer::position/duration/state`
/// all crash the whole process on iOS: their native implementations call
/// `msg_send![player, currentTime]` expecting the Objective-C runtime to
/// return a `CMTime` struct by value, but the `Encode` impl doesn't match
/// what `objc_msgSend` actually does for a struct this size, so the runtime
/// aborts with "invalid message send" — and since this crate builds with
/// panic=abort, it's not catchable, not a `Result::Err`, it just kills the
/// app. See crates/gpui_mobile/src/packages/video_player/ios.rs:171-174 and
/// crates/gpui_mobile/src/packages/audio/ios.rs:240-251,290-300. This screen
/// therefore never calls those three functions — everything below tracks
/// playback state locally instead (button taps + the duration reported once
/// by `set_url`/`set_file_path`, which do not hit this path).
const CMTIME_CRASH_NOTE: &str = "position()/duration()/state() are not called here — they crash \
     the whole app on iOS (msg_send returning CMTime by value has a mismatched Encode impl in \
     gpui_mobile; see video_player/ios.rs:171 and audio/ios.rs:240,290). Play/Pause/Stop below \
     just track local button-tap state instead.";

/// Write a minimal 44-byte canonical PCM WAV header followed by a 440Hz
/// sine tone, 3 seconds, mono, 16-bit, 44100Hz.
fn synth_tone_wav() -> Vec<u8> {
    let sample_rate: u32 = 44100;
    let seconds: u32 = 3;
    let num_samples = sample_rate * seconds;
    let data_size = num_samples * 2; // 16-bit mono
    let mut buf = Vec::with_capacity(44 + data_size as usize);

    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_size).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 2;
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * 2.0 * PI).sin() * i16::MAX as f32 * 0.5;
        buf.extend_from_slice(&(sample as i16).to_le_bytes());
    }
    buf
}

impl MediaScreen {
    fn init_video(&mut self) {
        match VideoPlayer::new() {
            Ok(player) => {
                match player.set_url(VIDEO_URL) {
                    Ok(info) => {
                        self.video_status = format!(
                            "loaded: {}x{}, {}ms",
                            info.width, info.height, info.duration_ms
                        );
                        self.video_duration_ms = info.duration_ms;
                    }
                    Err(e) => self.video_status = format!("set_url error: {e}"),
                }
                self.video = Some(player);
                gallery_log::push("media: video player created");
            }
            Err(e) => self.video_status = format!("VideoPlayer::new error: {e}"),
        }
    }

    fn ensure_surface(&mut self) {
        if let Some(video) = self.video.as_mut()
            && let Err(e) = video.show_surface(0.0, 0.0, 300.0, 168.75)
        {
            self.video_status = format!("show_surface error: {e}");
        }
    }
}

impl Render for MediaScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_surface();

        div()
            .id("media-scroll")
            .overflow_y_scroll()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(div().text_color(rgb(0xffffff)).text_size(px(20.0)).child("Media"))
            .child(note(
                "Video and audio playback via native platform APIs. Playback state below is \
                 tracked locally from button taps, not queried live — see the CMTime crash note \
                 in each player section.",
            ))
            .child(
                section("video_player")
                    .child(note(VIDEO_URL))
                    .child(
                        div()
                            .w(px(300.0))
                            .h(px(168.75)) // 300 / (16/9)
                            .bg(rgb(0x111111))
                            .child(
                                self.video
                                    .as_ref()
                                    .and_then(|v| v.platform_view_handle())
                                    .map(platform_view_element)
                                    .unwrap_or_else(|| div().size_full()),
                            ),
                    )
                    .child(kv("duration (from set_url)", format!("{}ms", self.video_duration_ms)))
                    .child(kv("playing (local)", self.video_playing.to_string()))
                    .child(note(CMTIME_CRASH_NOTE))
                    .child(row().flex_wrap()
                        .child(button("Play", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() {
                                match v.play() {
                                    Ok(()) => this.video_playing = true,
                                    Err(e) => this.video_status = format!("play error: {e}"),
                                }
                            }
                            cx.notify();
                        })))
                        .child(button("Pause", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() {
                                match v.pause() {
                                    Ok(()) => this.video_playing = false,
                                    Err(e) => this.video_status = format!("pause error: {e}"),
                                }
                            }
                            cx.notify();
                        })))
                        .child(button("Seek to 0:10", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref()
                                && let Err(e) = v.seek(10_000) { this.video_status = format!("seek error: {e}"); }
                            cx.notify();
                        })))
                        .child(button("Seek to 0:30", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref()
                                && let Err(e) = v.seek(30_000) { this.video_status = format!("seek error: {e}"); }
                            cx.notify();
                        }))))
                    .child(row().flex_wrap()
                        .child(button("Vol 0.5", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() { let _ = v.set_volume(0.5); }
                            cx.notify();
                        })))
                        .child(button("Vol 1.0", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() { let _ = v.set_volume(1.0); }
                            cx.notify();
                        })))
                        .child(button("Speed 0.5x", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() { let _ = v.set_speed(0.5); }
                            cx.notify();
                        })))
                        .child(button("Speed 1x", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() { let _ = v.set_speed(1.0); }
                            cx.notify();
                        })))
                        .child(button("Speed 2x", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() { let _ = v.set_speed(2.0); }
                            cx.notify();
                        })))
                        .child(button("Loop on", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() { let _ = v.set_looping(true); }
                            cx.notify();
                        })))
                        .child(button("Loop off", cx.listener(|this, _, _window, cx| {
                            if let Some(v) = this.video.as_ref() { let _ = v.set_looping(false); }
                            cx.notify();
                        }))))
                    .child(note(self.video_status.clone())),
            )
            .child(
                section("audio: synthesized tone")
                    .child(note("A 440Hz sine, 3s, hand-rolled 44-byte WAV header, written to path_provider::temporary_directory()."))
                    .child(button("Create + play tone", cx.listener(|this, _, _window, cx| {
                        match path_provider::temporary_directory() {
                            Ok(dir) => {
                                let path = dir.join("gallery-tone.wav");
                                let wav = synth_tone_wav();
                                match std::fs::write(&path, &wav) {
                                    Ok(()) => match AudioPlayer::new() {
                                        Ok(player) => {
                                            match player.set_file_path(&path.to_string_lossy()) {
                                                Ok(_) => {
                                                    let _ = player.play();
                                                    this.tone_playing = true;
                                                    this.tone_status = format!("playing {}", path.display());
                                                    gallery_log::push("media: tone audio playing");
                                                }
                                                Err(e) => this.tone_status = format!("set_file_path error: {e}"),
                                            }
                                            this.tone_audio = Some(player);
                                        }
                                        Err(e) => this.tone_status = format!("AudioPlayer::new error: {e}"),
                                    },
                                    Err(e) => this.tone_status = format!("write error: {e}"),
                                }
                            }
                            Err(e) => this.tone_status = format!("temporary_directory error: {e}"),
                        }
                        cx.notify();
                    })))
                    .child(row().flex_wrap()
                        .child(button("Pause tone", cx.listener(|this, _, _window, cx| {
                            if let Some(a) = this.tone_audio.as_ref() { let _ = a.pause(); }
                            this.tone_playing = false;
                            cx.notify();
                        })))
                        .child(button("Resume tone", cx.listener(|this, _, _window, cx| {
                            if let Some(a) = this.tone_audio.as_ref() { let _ = a.play(); }
                            this.tone_playing = true;
                            cx.notify();
                        })))
                        .child(button("Stop tone", cx.listener(|this, _, _window, cx| {
                            if let Some(a) = this.tone_audio.as_ref() { let _ = a.stop(); }
                            this.tone_playing = false;
                            cx.notify();
                        }))))
                    .child(kv("playing (local)", self.tone_playing.to_string()))
                    .child(note(CMTIME_CRASH_NOTE))
                    .child(note(self.tone_status.clone())),
            )
            .child(
                section("audio: remote file")
                    .child(note(REMOTE_MP3_URL))
                    .child(button("Create + play remote", cx.listener(|this, _, _window, cx| {
                        match AudioPlayer::new() {
                            Ok(player) => {
                                match player.set_url(REMOTE_MP3_URL) {
                                    Ok(_) => {
                                        let _ = player.play();
                                        this.remote_playing = true;
                                        this.remote_status = "playing remote file".into();
                                        gallery_log::push("media: remote audio playing");
                                    }
                                    Err(e) => this.remote_status = format!("set_url error: {e}"),
                                }
                                this.remote_audio = Some(player);
                            }
                            Err(e) => this.remote_status = format!("AudioPlayer::new error: {e}"),
                        }
                        cx.notify();
                    })))
                    .child(row().flex_wrap()
                        .child(button("Pause remote", cx.listener(|this, _, _window, cx| {
                            if let Some(a) = this.remote_audio.as_ref() { let _ = a.pause(); }
                            this.remote_playing = false;
                            cx.notify();
                        })))
                        .child(button("Resume remote", cx.listener(|this, _, _window, cx| {
                            if let Some(a) = this.remote_audio.as_ref() { let _ = a.play(); }
                            this.remote_playing = true;
                            cx.notify();
                        })))
                        .child(button("Stop remote", cx.listener(|this, _, _window, cx| {
                            if let Some(a) = this.remote_audio.as_ref() { let _ = a.stop(); }
                            this.remote_playing = false;
                            cx.notify();
                        }))))
                    .child(kv("playing (local)", self.remote_playing.to_string()))
                    .child(note(CMTIME_CRASH_NOTE))
                    .child(note(self.remote_status.clone())),
            )
            .child(
                section("media_session")
                    .child(note(
                        "gpui_mobile's media_session package only implements Android \
                         (see crates/gpui_mobile/src/packages/media_session/mod.rs: the ios module \
                         doesn't exist, and every function's `#[cfg(not(target_os = \"android\"))]` \
                         branch just returns Ok(()) unconditionally). On iOS these calls succeed but \
                         are silent no-ops — no lock-screen media info actually appears.",
                    ))
                    .child(button("init / set_metadata / set_playback_state", cx.listener(|this, _, _window, cx| {
                        let mut lines = Vec::new();
                        lines.push(format!("init() -> {:?}", media_session::init()));
                        lines.push(format!(
                            "set_metadata() -> {:?}",
                            media_session::set_metadata("GPUI Gallery Tone", "gpui_mobile", 3000)
                        ));
                        lines.push(format!(
                            "set_playback_state() -> {:?}",
                            media_session::set_playback_state(true, 0, 1.0)
                        ));
                        this.session_log = lines;
                        gallery_log::push("media: media_session calls issued (iOS: no-op stub)");
                        cx.notify();
                    })))
                    .children(self.session_log.iter().cloned().map(super::common::mono)),
            )
    }
}
