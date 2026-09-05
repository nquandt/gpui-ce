//! The Report screen: summarizes device info, every screen's recorded
//! verdict + notes, and the recent event log, and offers ways to export it.

use super::ScreenDescriptor;
use super::common::{button, kv, mono, note, row, section};
use crate::{log as gallery_log, store};
use gpui::{AnyView, App, ClipboardItem, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::packages::{device_info, package_info, path_provider, share};

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "report",
        title: "Report",
        category: "Meta",
        blurb: "export verdicts, notes and event log",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| ReportScreen {
        status: String::new(),
    })
    .into()
}

struct ReportScreen {
    status: String,
}

fn device_summary() -> String {
    let device = device_info::get_device_info();
    let package = package_info::get_package_info();
    let mut out = String::new();
    match package {
        Ok(p) => out.push_str(&format!(
            "{} {} (build {}) — {}",
            p.app_name, p.version, p.build_number, p.package_name
        )),
        Err(e) => out.push_str(&format!("package_info error: {e}")),
    }
    out.push('\n');
    match device {
        Ok(d) => out.push_str(&format!(
            "{} · {} · iOS {}{}",
            d.device_name,
            d.model,
            d.os_version,
            if d.is_physical_device {
                ""
            } else {
                " (simulator)"
            }
        )),
        Err(e) => out.push_str(&format!("device_info error: {e}")),
    }
    out
}

fn build_markdown() -> String {
    let mut md = String::new();
    md.push_str("# GPUI Gallery Report\n\n");
    md.push_str(&format!("_{}_\n\n", chrono_stamp()));
    for line in device_summary().lines() {
        md.push_str(&format!("- {line}\n"));
    }
    md.push_str("\n| Screen | Verdict | Notes |\n");
    md.push_str("| --- | --- | --- |\n");
    for screen in super::all() {
        if screen.id == "report" {
            continue;
        }
        let verdict = store::load_verdict(screen.id);
        let notes = store::load_notes(screen.id);
        let notes = notes.replace('\n', " ").replace('|', "\\|");
        md.push_str(&format!(
            "| {} | {} {} | {} |\n",
            screen.title,
            verdict.glyph(),
            verdict.label(),
            notes
        ));
    }
    md.push_str("\n## Event log (last 200)\n\n```\n");
    for line in gallery_log::last(200) {
        md.push_str(&line);
        md.push('\n');
    }
    md.push_str("```\n");
    md
}

fn chrono_stamp() -> String {
    // No chrono dependency; a coarse wall-clock stamp is enough for a
    // manual-testing report.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("generated at unix time {}", now.as_secs())
}

impl Render for ReportScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let summary = device_summary();
        let event_lines = gallery_log::last(200);
        let event_text = event_lines.join("\n");

        div()
            .id("report_root")
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .text_color(rgb(0xffffff))
                    .text_size(px(20.0))
                    .child("Report"),
            )
            .child(note(
                "Export the recorded verdicts, notes and event log for every screen.",
            ))
            .child(section("Device").children(summary.lines().map(|l| kv("", l.to_string()))))
            .child(
                section("Screens").children(
                    super::all()
                        .into_iter()
                        .filter(|s| s.id != "report")
                        .map(|s| {
                            let verdict = store::load_verdict(s.id);
                            let notes = store::load_notes(s.id);
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .py_1()
                                .child(row().child(mono(format!(
                                    "{} {}",
                                    verdict.glyph(),
                                    s.title
                                ))))
                                .child(if notes.is_empty() {
                                    note("(no notes)")
                                } else {
                                    note(notes)
                                })
                        }),
                ),
            )
            .child(
                section("Event log (last 200)").child(
                    div()
                        .id("report_event_log")
                        .h(px(220.0))
                        .overflow_y_scroll()
                        .child(mono(event_text)),
                ),
            )
            .child(if self.status.is_empty() {
                div()
            } else {
                div().child(note(self.status.clone()))
            })
            .child(
                row()
                    .flex_wrap()
                    .child(button(
                        "Copy report",
                        cx.listener(|this, _, _window, cx| {
                            let md = build_markdown();
                            cx.write_to_clipboard(ClipboardItem::new_string(md));
                            this.status = "Copied report to clipboard.".into();
                            gallery_log::push("report: copied to clipboard");
                            cx.notify();
                        }),
                    ))
                    .child(button(
                        "Share…",
                        cx.listener(|this, _, _window, cx| {
                            let md = build_markdown();
                            match share::share_text(&md, Some("GPUI Gallery Report")) {
                                Ok(()) => this.status = "Share sheet opened.".into(),
                                Err(e) => this.status = format!("Share failed: {e}"),
                            }
                            gallery_log::push("report: share invoked");
                            cx.notify();
                        }),
                    ))
                    .child(button(
                        "Save to Documents",
                        cx.listener(|this, _, _window, cx| {
                            let md = build_markdown();
                            match path_provider::documents_directory() {
                                Ok(dir) => {
                                    let path = dir.join("gallery-report.md");
                                    match std::fs::write(&path, md) {
                                        Ok(()) => {
                                            this.status = format!("Saved to {}", path.display());
                                        }
                                        Err(e) => this.status = format!("Write failed: {e}"),
                                    }
                                }
                                Err(e) => this.status = format!("documents_directory error: {e}"),
                            }
                            gallery_log::push("report: saved to documents");
                            cx.notify();
                        }),
                    ))
                    .child(button(
                        "Reset all verdicts",
                        cx.listener(|this, _, _window, cx| {
                            let ids: Vec<&'static str> =
                                super::all().iter().map(|s| s.id).collect();
                            store::reset_all(&ids);
                            gallery_log::clear();
                            this.status =
                                "All verdicts, notes and the event log were reset.".into();
                            gallery_log::push("report: reset all");
                            cx.notify();
                        }),
                    )),
            )
    }
}
