//! "Dialogs & pickers": exercises `window.prompt`, `cx.prompt_for_paths`,
//! `cx.prompt_for_new_path`, `cx.open_with_system`, the `share` package,
//! `cx.open_url`, and the `url_launcher`/`maps_launcher` packages. Every
//! result is rendered on screen so a tester never has to read logs.

use super::ScreenDescriptor;
use super::common::{button, mono, note, row, section};
use crate::log as gallery_log;
use gpui::{
    AnyView, App, Context, PathPromptOptions, PromptLevel, Window, div, prelude::*, px, rgb,
};
use gpui_mobile::packages::{maps_launcher, share, url_launcher};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "dialogs",
        title: "Dialogs & pickers",
        category: "Window & system",
        blurb: "prompt(), file picker, open with, share",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| DialogsScreen {
        prompt_result: None,
        paths_result: None,
        new_path_result: None,
        open_with_result: None,
        share_result: None,
        open_url_result: None,
        launcher_result: None,
    })
    .into()
}

struct DialogsScreen {
    prompt_result: Option<String>,
    paths_result: Option<String>,
    new_path_result: Option<String>,
    open_with_result: Option<String>,
    share_result: Option<String>,
    open_url_result: Option<String>,
    launcher_result: Option<String>,
}

fn prompt_row(
    cx: &mut Context<DialogsScreen>,
    label: &'static str,
    level: PromptLevel,
    answers: &'static [&'static str],
) -> impl IntoElement {
    button(
        label,
        cx.listener(move |_this, _, window, cx| {
            let receiver = window.prompt(level, label, None, answers, cx);
            gallery_log::push(format!("dialogs: prompt '{label}' shown"));
            cx.spawn(async move |this, cx| {
                let result = receiver.await;
                this.update(cx, |this, cx| {
                    this.prompt_result = Some(match result {
                        Ok(index) => format!(
                            "'{label}' -> answer #{index} ('{}')",
                            answers.get(index).unwrap_or(&"?")
                        ),
                        Err(_) => format!("'{label}' -> dismissed without an answer"),
                    });
                    gallery_log::push(format!(
                        "dialogs: prompt result: {}",
                        this.prompt_result.clone().unwrap_or_default()
                    ));
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }),
    )
}

impl Render for DialogsScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("dialogs-scroll")
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
                    .child("Dialogs & pickers"),
            )
            .child(note(
                "Tap each button below; the result of the last action in \
                 each section is shown underneath it.",
            ))
            .child(
                section("window.prompt (Info/Warning/Critical)")
                    .child(
                        row()
                            .flex_wrap()
                            .child(prompt_row(
                                cx,
                                "Info: one answer",
                                PromptLevel::Info,
                                &["OK"],
                            ))
                            .child(prompt_row(
                                cx,
                                "Warning: two answers",
                                PromptLevel::Warning,
                                &["Cancel", "Continue"],
                            ))
                            .child(prompt_row(
                                cx,
                                "Critical: three answers",
                                PromptLevel::Critical,
                                &["Cancel", "Delete", "Delete All"],
                            )),
                    )
                    .child(match &self.prompt_result {
                        Some(r) => mono(r.clone()),
                        None => note("(no prompt answered yet)"),
                    }),
            )
            .child(
                section("cx.prompt_for_paths (files, multiple, directories)")
                    .child(button(
                        "Pick files",
                        cx.listener(|_this, _, _window, cx| {
                            let receiver = cx.prompt_for_paths(PathPromptOptions {
                                files: true,
                                directories: false,
                                multiple: true,
                                prompt: Some("Pick files".into()),
                            });
                            gallery_log::push("dialogs: prompt_for_paths (files) shown");
                            cx.spawn(async move |this, cx| {
                                let result = receiver.await;
                                this.update(cx, |this, cx| {
                                    this.paths_result = Some(describe_paths_result(result));
                                    gallery_log::push(format!(
                                        "dialogs: prompt_for_paths result: {}",
                                        this.paths_result.clone().unwrap_or_default()
                                    ));
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        }),
                    ))
                    .child(button(
                        "Pick a directory",
                        cx.listener(|_this, _, _window, cx| {
                            let receiver = cx.prompt_for_paths(PathPromptOptions {
                                files: false,
                                directories: true,
                                multiple: false,
                                prompt: Some("Pick a directory".into()),
                            });
                            gallery_log::push("dialogs: prompt_for_paths (directories) shown");
                            cx.spawn(async move |this, cx| {
                                let result = receiver.await;
                                this.update(cx, |this, cx| {
                                    this.paths_result = Some(describe_paths_result(result));
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        }),
                    ))
                    .child(match &self.paths_result {
                        Some(r) => mono(r.clone()),
                        None => note("(nothing picked yet)"),
                    }),
            )
            .child(
                section("cx.prompt_for_new_path")
                    .child(button(
                        "Prompt for new path",
                        cx.listener(|_this, _, _window, cx| {
                            let dir = gpui_mobile::packages::path_provider::documents_directory()
                                .unwrap_or_else(|_| std::env::temp_dir());
                            let receiver = cx.prompt_for_new_path(&dir, Some("gallery-note.txt"));
                            gallery_log::push("dialogs: prompt_for_new_path shown");
                            cx.spawn(async move |this, cx| {
                                let result = receiver.await;
                                this.update(cx, |this, cx| {
                                    this.new_path_result = Some(match result {
                                        Ok(Ok(Some(path))) => format!("-> {}", path.display()),
                                        Ok(Ok(None)) => "-> cancelled".to_string(),
                                        Ok(Err(e)) => format!("-> error: {e}"),
                                        Err(_) => "-> channel dropped".to_string(),
                                    });
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        }),
                    ))
                    .child(match &self.new_path_result {
                        Some(r) => mono(r.clone()),
                        None => note("(not invoked yet)"),
                    }),
            )
            .child(
                section("cx.open_with_system")
                    .child(note(
                        "Writes a small text file to the temp directory, then \
                         asks the system to open it (Files/Quick Look share sheet).",
                    ))
                    .child(button(
                        "Write + open with system",
                        cx.listener(|this, _, _window, cx| {
                            let dir = std::env::temp_dir();
                            let path = dir.join("gallery-open-with-system.txt");
                            match std::fs::write(&path, "Hello from the GPUI gallery.\n") {
                                Ok(()) => {
                                    cx.open_with_system(&path);
                                    this.open_with_result =
                                        Some(format!("Opened {}", path.display()));
                                }
                                Err(e) => {
                                    this.open_with_result = Some(format!("Write failed: {e}"));
                                }
                            }
                            gallery_log::push("dialogs: open_with_system invoked");
                            cx.notify();
                        }),
                    ))
                    .child(match &self.open_with_result {
                        Some(r) => mono(r.clone()),
                        None => note("(not invoked yet)"),
                    }),
            )
            .child(
                section("share package (share_text)")
                    .child(button(
                        "Share…",
                        cx.listener(|this, _, _window, cx| {
                            this.share_result = Some(
                                match share::share_text(
                                    "Shared from the GPUI gallery.",
                                    Some("Gallery"),
                                ) {
                                    Ok(()) => "Share sheet opened.".to_string(),
                                    Err(e) => format!("Error: {e}"),
                                },
                            );
                            gallery_log::push("dialogs: share_text invoked");
                            cx.notify();
                        }),
                    ))
                    .child(match &self.share_result {
                        Some(r) => mono(r.clone()),
                        None => note("(not invoked yet)"),
                    }),
            )
            .child(
                section("cx.open_url")
                    .child(button(
                        "Open example.com",
                        cx.listener(|this, _, _window, cx| {
                            cx.open_url("https://example.com");
                            this.open_url_result =
                                Some("open_url(\"https://example.com\") called.".into());
                            gallery_log::push("dialogs: open_url invoked");
                            cx.notify();
                        }),
                    ))
                    .child(match &self.open_url_result {
                        Some(r) => mono(r.clone()),
                        None => note("(not invoked yet)"),
                    }),
            )
            .child(
                section("url_launcher / maps_launcher package variants")
                    .child(
                        row()
                            .flex_wrap()
                            .child(button(
                                "tel:",
                                cx.listener(|this, _, _window, cx| {
                                    run_launcher(this, cx, "tel:+15555550123");
                                }),
                            ))
                            .child(button(
                                "mailto:",
                                cx.listener(|this, _, _window, cx| {
                                    run_launcher(this, cx, "mailto:test@example.com");
                                }),
                            ))
                            .child(button(
                                "sms:",
                                cx.listener(|this, _, _window, cx| {
                                    run_launcher(this, cx, "sms:+15555550123");
                                }),
                            ))
                            .child(button(
                                "Maps (query)",
                                cx.listener(|this, _, _window, cx| {
                                    this.launcher_result =
                                        Some(match maps_launcher::open_query("Cupertino, CA") {
                                            Ok(opened) => {
                                                format!("maps_launcher::open_query -> {opened}")
                                            }
                                            Err(e) => format!("maps_launcher error: {e}"),
                                        });
                                    gallery_log::push("dialogs: maps_launcher::open_query invoked");
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(match &self.launcher_result {
                        Some(r) => mono(r.clone()),
                        None => note("(not invoked yet)"),
                    }),
            )
    }
}

fn run_launcher(this: &mut DialogsScreen, cx: &mut Context<DialogsScreen>, url: &str) {
    this.launcher_result = Some(match url_launcher::launch_url(url) {
        Ok(opened) => format!("url_launcher::launch_url({url}) -> {opened}"),
        Err(e) => format!("url_launcher error for {url}: {e}"),
    });
    gallery_log::push(format!("dialogs: url_launcher invoked for {url}"));
    cx.notify();
}

fn describe_paths_result<E>(
    result: Result<gpui::Result<Option<Vec<std::path::PathBuf>>>, E>,
) -> String {
    match result {
        Ok(Ok(Some(paths))) if !paths.is_empty() => paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        Ok(Ok(Some(_))) => "-> empty selection".to_string(),
        Ok(Ok(None)) => "-> cancelled".to_string(),
        Ok(Err(e)) => format!("-> error: {e}"),
        Err(_) => "-> channel dropped".to_string(),
    }
}
