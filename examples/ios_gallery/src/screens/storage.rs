//! "Storage": `shared_preferences` set/get/remove round trips for
//! string/int/bool, `path_provider` directories with exists/writable
//! checks, a Documents file write+read-back, and a keychain
//! write→read→delete round trip via `cx.write_credentials` et al.

use super::ScreenDescriptor;
use super::common::{button, kv, mono, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::packages::{path_provider, shared_preferences::SharedPreferences};

const PREF_STRING_KEY: &str = "gallery.storage.demo_string";
const PREF_INT_KEY: &str = "gallery.storage.demo_int";
const PREF_BOOL_KEY: &str = "gallery.storage.demo_bool";
const KEYCHAIN_URL: &str = "gallery.storage.demo";

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "storage",
        title: "Storage",
        category: "Window & system",
        blurb: "preferences, files, keychain credentials",
        build,
    }
}

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| StorageScreen {
        file_result: None,
        keychain_result: None,
    })
    .into()
}

struct StorageScreen {
    file_result: Option<String>,
    keychain_result: Option<String>,
}

fn prefs_table() -> Vec<(&'static str, String)> {
    let prefs = SharedPreferences::instance();
    vec![
        (
            "string",
            prefs
                .get_string(PREF_STRING_KEY)
                .unwrap_or_else(|| "(unset)".into()),
        ),
        (
            "int",
            prefs
                .get_int(PREF_INT_KEY)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "(unset)".into()),
        ),
        (
            "bool",
            prefs
                .get_bool(PREF_BOOL_KEY)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "(unset)".into()),
        ),
    ]
}

fn directory_row(
    label: &'static str,
    result: Result<std::path::PathBuf, String>,
) -> impl IntoElement {
    match result {
        Ok(path) => {
            let exists = path.exists();
            let writable = exists
                && std::fs::metadata(&path)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false);
            kv(
                label,
                format!(
                    "{} (exists: {exists}, writable: {writable})",
                    path.display()
                ),
            )
        }
        Err(e) => kv(label, format!("error: {e}")),
    }
}

impl Render for StorageScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = SharedPreferences::instance();

        div()
            .id("storage-scroll")
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
                    .child("Storage"),
            )
            .child(
                section("shared_preferences")
                    .children(prefs_table().into_iter().map(|(k, v)| kv(k, v)))
                    .child(
                        row()
                            .flex_wrap()
                            .child(button(
                                "Set all",
                                cx.listener(|_this, _, _window, cx| {
                                    let prefs = SharedPreferences::instance();
                                    let _ = prefs.set_string(PREF_STRING_KEY, "hello gallery");
                                    let _ = prefs.set_int(PREF_INT_KEY, 42);
                                    let _ = prefs.set_bool(PREF_BOOL_KEY, true);
                                    gallery_log::push("storage: shared_preferences set all");
                                    cx.notify();
                                }),
                            ))
                            .child(button(
                                "Remove all",
                                cx.listener(|_this, _, _window, cx| {
                                    let prefs = SharedPreferences::instance();
                                    let _ = prefs.remove(PREF_STRING_KEY);
                                    let _ = prefs.remove(PREF_INT_KEY);
                                    let _ = prefs.remove(PREF_BOOL_KEY);
                                    gallery_log::push("storage: shared_preferences removed all");
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(note(format!(
                        "contains_key(string) -> {}",
                        prefs.contains_key(PREF_STRING_KEY)
                    ))),
            )
            .child(
                section("path_provider directories")
                    .child(directory_row(
                        "temporary",
                        path_provider::temporary_directory(),
                    ))
                    .child(directory_row(
                        "documents",
                        path_provider::documents_directory(),
                    ))
                    .child(directory_row("cache", path_provider::cache_directory()))
                    .child(directory_row("support", path_provider::support_directory())),
            )
            .child(
                section("Write + read back a Documents file")
                    .child(button(
                        "Write & read gallery-storage.txt",
                        cx.listener(|this, _, _window, cx| {
                            let result = (|| -> Result<String, String> {
                                let dir = path_provider::documents_directory()?;
                                let path = dir.join("gallery-storage.txt");
                                let contents = format!(
                                    "gallery storage test @ {:?}",
                                    std::time::SystemTime::now()
                                );
                                std::fs::write(&path, &contents).map_err(|e| e.to_string())?;
                                let read_back =
                                    std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                                Ok(format!(
                                    "wrote+read {} ({size} bytes): \"{read_back}\"",
                                    path.display()
                                ))
                            })();
                            this.file_result = Some(match result {
                                Ok(s) => s,
                                Err(e) => format!("error: {e}"),
                            });
                            gallery_log::push("storage: documents file round trip");
                            cx.notify();
                        }),
                    ))
                    .child(match &self.file_result {
                        Some(r) => mono(r.clone()),
                        None => note("(not run yet)"),
                    }),
            )
            .child(
                section("Keychain credentials round trip")
                    .child(button(
                        "write -> read -> delete",
                        cx.listener(|_this, _, _window, cx| {
                            let write =
                                cx.write_credentials(KEYCHAIN_URL, "gallery-user", b"gallery-pass");
                            gallery_log::push("storage: keychain write_credentials invoked");
                            cx.spawn(async move |this, cx| {
                                let write_result = write.await;
                                let read_task =
                                    this.update(cx, |_this, cx| cx.read_credentials(KEYCHAIN_URL));
                                let Ok(read_task) = read_task else {
                                    return;
                                };
                                let read_result = read_task.await;
                                let delete_task = this
                                    .update(cx, |_this, cx| cx.delete_credentials(KEYCHAIN_URL));
                                let Ok(delete_task) = delete_task else {
                                    return;
                                };
                                let delete_result = delete_task.await;
                                this.update(cx, |this, cx| {
                                    let summary = format!(
                                        "write: {} | read: {} | delete: {}",
                                        match &write_result {
                                            Ok(()) => "Ok".to_string(),
                                            Err(e) => format!("Err({e})"),
                                        },
                                        match &read_result {
                                            Ok(Some((user, pass))) => format!(
                                                "Ok(user={user}, pass={} bytes)",
                                                pass.len()
                                            ),
                                            Ok(None) => "Ok(None)".to_string(),
                                            Err(e) => format!("Err({e})"),
                                        },
                                        match &delete_result {
                                            Ok(()) => "Ok".to_string(),
                                            Err(e) => format!("Err({e})"),
                                        },
                                    );
                                    this.keychain_result = Some(summary.clone());
                                    gallery_log::push(format!(
                                        "storage: keychain round trip: {summary}"
                                    ));
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        }),
                    ))
                    .child(match &self.keychain_result {
                        Some(r) => mono(r.clone()),
                        None => note("(not run yet)"),
                    }),
            )
    }
}
