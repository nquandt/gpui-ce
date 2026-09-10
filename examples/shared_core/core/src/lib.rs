//! The shared application core.
//!
//! Everything in this crate is platform-neutral. It talks to the host only
//! through GPUI services: `cx.http_client()`, `cx.key_value_store()` and the
//! `AssetSource`. The desktop and web hosts differ only in how they construct
//! the `Application` and hand control to it.

use anyhow::Result;
use gpui::file_picker::FilePickerOptions;
use gpui::http_client::HttpRequest;
use gpui::key_value_store::KeyValueStoreExt as _;
use gpui::{
    App, AssetSource, Bounds, ClickEvent, Context, FontWeight, SharedString, Task, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size, svg,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

/// Embeds every asset the app needs, so no target depends on a file system
/// or on installed fonts. The web backend starts with an empty font database,
/// so embedding the font is required there and harmless everywhere else.
pub struct Assets;

const FONT_REGULAR: &[u8] =
    include_bytes!("../../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf");
const FONT_SEMIBOLD: &[u8] =
    include_bytes!("../../../../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf");
const LOGO_SVG: &[u8] = include_bytes!("../assets/logo.svg");

const FONT_FAMILY: &str = "IBM Plex Sans";

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(match path {
            "logo.svg" => Some(Cow::Borrowed(LOGO_SVG)),
            _ => None,
        })
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec!["logo.svg".into()])
    }
}

/// Register fonts and anything else the app needs before the first window.
/// Call this from every host right after the application starts.
pub fn init(cx: &mut App) {
    if let Err(error) = cx.text_system().add_fonts(vec![
        Cow::Borrowed(FONT_REGULAR),
        Cow::Borrowed(FONT_SEMIBOLD),
    ]) {
        log::error!("failed to register embedded fonts: {error:#}");
    }
}

// ---------------------------------------------------------------------------
// Persisted state
// ---------------------------------------------------------------------------

const SETTINGS_KEY: &str = "demo.settings";

#[derive(Serialize, Deserialize, Default, Clone)]
struct Settings {
    count: i32,
    url: String,
}

impl Settings {
    fn load(cx: &App) -> Self {
        let mut settings = cx
            .key_value_store()
            .get_json::<Settings>(SETTINGS_KEY)
            .unwrap_or_else(|error| {
                log::warn!("could not read settings: {error:#}");
                None
            })
            .unwrap_or_default();
        if settings.url.is_empty() {
            settings.url = "https://api.github.com/zen".to_owned();
        }
        settings
    }

    fn save(&self, cx: &App) {
        if let Err(error) = cx.key_value_store().set_json(SETTINGS_KEY, self) {
            log::warn!("could not save settings: {error:#}");
        }
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

enum Fetch {
    Idle,
    Running,
    Done { status: String, body: String },
    Failed(String),
}

pub struct Demo {
    settings: Settings,
    fetch: Fetch,
    picked: Option<String>,
    _fetch_task: Option<Task<()>>,
    _pick_task: Option<Task<()>>,
}

impl Demo {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            settings: Settings::load(cx),
            fetch: Fetch::Idle,
            picked: None,
            _fetch_task: None,
            _pick_task: None,
        }
    }

    fn pick_file(&mut self, cx: &mut Context<Self>) {
        let picker = cx.file_picker();
        let options = FilePickerOptions::default()
            .accept("image/*")
            .accept(".txt");
        self._pick_task = Some(cx.spawn(async move |this, cx| {
            let outcome = picker.pick_files(options).await;
            this.update(cx, |this, cx| {
                this.picked = Some(match outcome {
                    Ok(files) if files.is_empty() => "No file chosen.".to_owned(),
                    Ok(files) => {
                        let file = &files[0];
                        format!(
                            "{} ({} bytes{})",
                            file.name,
                            file.bytes.len(),
                            file.mime_type
                                .as_deref()
                                .map(|mime| format!(", {mime}"))
                                .unwrap_or_default()
                        )
                    }
                    Err(error) => format!("Picker failed: {error:#}"),
                });
                cx.notify();
            })
            .ok();
        }));
    }

    fn adjust(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.settings.count += delta;
        self.settings.save(cx);
        cx.notify();
    }

    fn start_fetch(&mut self, cx: &mut Context<Self>) {
        let client = cx.http_client();
        let request = HttpRequest::get(&self.settings.url).header("Accept", "text/plain");
        self.fetch = Fetch::Running;
        cx.notify();

        self._fetch_task = Some(cx.spawn(async move |this, cx| {
            let outcome = client.send(request).await;
            this.update(cx, |this, cx| {
                this.fetch = match outcome {
                    Ok(response) => Fetch::Done {
                        status: response.status.to_string(),
                        body: response.text().lines().next().unwrap_or("").to_owned(),
                    },
                    Err(error) => Fetch::Failed(format!("{error:#}")),
                };
                cx.notify();
            })
            .ok();
        }));
    }
}

const BG: u32 = 0x1e1e2e;
const SURFACE: u32 = 0x313244;
const SURFACE_HOVER: u32 = 0x45475a;
const TEXT: u32 = 0xcdd6f4;
const DIM: u32 = 0xa6adc8;
const ACCENT: u32 = 0x89b4fa;
const GREEN: u32 = 0xa6e3a1;
const RED: u32 = 0xf38ba8;

fn button(
    id: &'static str,
    label: impl Into<SharedString>,
    cx: &mut Context<Demo>,
    on_click: impl Fn(&mut Demo, &mut Context<Demo>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_4()
        .py_2()
        .rounded_md()
        .bg(rgb(SURFACE))
        .text_color(rgb(TEXT))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label.into())
        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| on_click(this, cx)))
}

impl Render for Demo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.settings.count;
        let (fetch_color, fetch_text) = match &self.fetch {
            Fetch::Idle => (DIM, "Press Fetch to call the API.".to_owned()),
            Fetch::Running => (DIM, "Fetching...".to_owned()),
            Fetch::Done { status, body } => (GREEN, format!("{status}: {body}")),
            Fetch::Failed(error) => (RED, error.clone()),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_6()
            .bg(rgb(BG))
            .font_family(FONT_FAMILY)
            .text_color(rgb(TEXT))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(svg().path("logo.svg").size_8().text_color(rgb(ACCENT)))
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("One core, every host"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(div().text_color(rgb(DIM)).child("Persisted counter"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(button("dec", "-", cx, |this, cx| this.adjust(-1, cx)))
                            .child(
                                div()
                                    .text_3xl()
                                    .min_w_16()
                                    .text_center()
                                    .child(count.to_string()),
                            )
                            .child(button("inc", "+", cx, |this, cx| this.adjust(1, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .max_w(px(520.))
                    .child(div().text_color(rgb(DIM)).child(self.settings.url.clone()))
                    .child(button("fetch", "Fetch", cx, |this, cx| {
                        this.start_fetch(cx)
                    }))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(fetch_color))
                            .child(fetch_text),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(button("pick", "Open file...", cx, |this, cx| {
                        this.pick_file(cx)
                    }))
                    .child(
                        div().text_sm().text_color(rgb(DIM)).child(
                            self.picked
                                .clone()
                                .unwrap_or_else(|| "Pick an image or .txt file.".to_owned()),
                        ),
                    ),
            )
    }
}

/// Open the main window. Every host calls this from its launch callback.
pub fn open_main_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(640.), px(480.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |_, cx| cx.new(Demo::new),
    )
    .expect("failed to open window");
    cx.activate(true);
}
