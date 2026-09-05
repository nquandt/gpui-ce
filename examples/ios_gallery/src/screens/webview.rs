//! "Webview": a WKWebView platform view plus a native text-field URL bar
//! (mirrors `examples/ios_browser`), evaluate_javascript, and the
//! URL-changed callback readout.

use super::ScreenDescriptor;
use super::common::{button, kv, note, row, section};
use crate::log as gallery_log;
use gpui::{AnyView, App, Context, Window, div, prelude::*, px, rgb};
use gpui_mobile::components::platform_view_element::platform_view_element;
use gpui_mobile::packages::text_field::{
    TextFieldHandle, create_text_field, is_focused, set_on_submit, set_text_on,
};
use gpui_mobile::packages::webview::{self, WebViewHandle, WebViewSettings, set_on_url_changed};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Mutex;

pub fn descriptor() -> ScreenDescriptor {
    ScreenDescriptor {
        id: "webview",
        title: "Webview",
        category: "Hardware & media",
        blurb: "WKWebView, URL bar, evaluate_javascript",
        build,
    }
}

const HOME_URL: &str = "https://example.com";

/// Shared with the URL-changed native callback, which runs outside of
/// GPUI's update cycle.
static LAST_URL_CHANGE: Mutex<Option<String>> = Mutex::new(None);

fn build(_window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_cx| {
        let webview = webview::load_url(HOME_URL, &WebViewSettings::default());
        let url_field = create_text_field(HOME_URL, "Enter a URL");
        let this = WebviewScreen {
            webview: webview.ok(),
            url_field: url_field.ok(),
            status: String::new(),
            js_result: String::new(),
            pending_go: Rc::new(Cell::new(false)),
        };
        this.wire_url_changed();
        this.wire_submit();
        this
    })
    .into()
}

struct WebviewScreen {
    webview: Option<WebViewHandle>,
    url_field: Option<TextFieldHandle>,
    status: String,
    js_result: String,
    pending_go: Rc<Cell<bool>>,
}

impl WebviewScreen {
    fn wire_url_changed(&self) {
        let field_handle = self
            .url_field
            .as_ref()
            .and_then(|f| f.platform_view_handle());
        set_on_url_changed(Some(Box::new(move |url: &str| {
            *LAST_URL_CHANGE.lock().unwrap() = Some(url.to_string());
            if let Some(field_handle) = field_handle.as_ref()
                && !is_focused(field_handle)
            {
                set_text_on(field_handle, url);
            }
        })));
    }

    fn wire_submit(&self) {
        let pending_go = self.pending_go.clone();
        set_on_submit(Some(Box::new(move || {
            pending_go.set(true);
        })));
    }

    fn go(&mut self, cx: &mut Context<Self>) {
        let Some(field) = self.url_field.as_ref() else {
            return;
        };
        field.resign_focus();
        let text = field.text().unwrap_or_default().trim().to_string();
        if text.is_empty() {
            return;
        }
        let has_scheme = text.split_once("://").is_some_and(|(scheme, _)| {
            scheme
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
        });
        let url = if has_scheme {
            text
        } else {
            format!("https://{text}")
        };
        match webview::load_url(&url, &WebViewSettings::default()) {
            Ok(new_webview) => {
                self.webview = Some(new_webview);
                self.wire_url_changed();
                self.status = format!("loaded {url}");
                gallery_log::push(format!("webview: loaded {url}"));
            }
            Err(e) => self.status = format!("load_url error: {e}"),
        }
        cx.notify();
    }
}

impl Render for WebviewScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pending_go.replace(false) {
            self.go(cx);
        }
        let last_url_change = LAST_URL_CHANGE.lock().unwrap().clone();

        div()
            .id("webview-scroll")
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(div().text_color(rgb(0xffffff)).text_size(px(20.0)).child("Webview"))
            .child(note(
                "A native WKWebView with a native text-field URL bar, mirroring examples/ios_browser.",
            ))
            .child(
                section("address bar").child(
                    row()
                        .child(
                            self.url_field
                                .as_ref()
                                .and_then(|f| f.platform_view_handle())
                                .map(platform_view_element)
                                .unwrap_or_else(div)
                                .flex_1()
                                .h(px(36.0)),
                        )
                        .child(button(
                            "Go",
                            cx.listener(|this, _, _window, cx| this.go(cx)),
                        )),
                ),
            )
            .child(
                section("webview")
                    .child(
                        div()
                            .w_full()
                            .h(px(400.0))
                            .bg(rgb(0x111111))
                            .child(
                                self.webview
                                    .as_ref()
                                    .and_then(|w| w.platform_view_handle())
                                    .map(platform_view_element)
                                    .unwrap_or_else(|| div().size_full()),
                            ),
                    )
                    .child(row().flex_wrap()
                        .child(button(
                            "Back",
                            cx.listener(|this, _, _window, cx| {
                                if let Some(w) = this.webview.as_ref() {
                                    this.status = match webview::go_back(w) {
                                        Ok(()) => "went back".into(),
                                        Err(e) => format!("go_back error: {e}"),
                                    };
                                }
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Reload",
                            cx.listener(|this, _, _window, cx| {
                                if let Some(w) = this.webview.as_ref() {
                                    this.status = match webview::reload(w) {
                                        Ok(()) => "reloaded".into(),
                                        Err(e) => format!("reload error: {e}"),
                                    };
                                }
                                cx.notify();
                            }),
                        ))
                        .child(button(
                            "Stop",
                            cx.listener(|this, _, _window, cx| {
                                if let Some(w) = this.webview.as_ref() {
                                    this.status = match webview::stop_loading(w) {
                                        Ok(()) => "stopped".into(),
                                        Err(e) => format!("stop_loading error: {e}"),
                                    };
                                }
                                cx.notify();
                            }),
                        )))
                    .child(note(self.status.clone())),
            )
            .child(
                section("evaluate_javascript")
                    .child(note(
                        "evaluate_javascript() returns Result<(), String> only — the JS \
                         completion value is not surfaced by gpui_mobile's webview package \
                         (see packages/webview/mod.rs's signature), so we can only show whether \
                         the call itself succeeded, not document.title's value.",
                    ))
                    .child(button(
                        "evaluate_javascript(\"document.title\")",
                        cx.listener(|this, _, _window, cx| {
                            if let Some(w) = this.webview.as_ref() {
                                this.js_result = match webview::evaluate_javascript(w, "document.title") {
                                    Ok(()) => "call succeeded (return value not surfaced)".into(),
                                    Err(e) => format!("error: {e}"),
                                };
                                gallery_log::push(format!("webview: evaluate_javascript -> {}", this.js_result));
                            }
                            cx.notify();
                        }),
                    ))
                    .child(note(self.js_result.clone())),
            )
            .child(
                section("URL-changed callback")
                    .child(kv(
                        "last url change",
                        last_url_change.unwrap_or_else(|| "(none yet)".into()),
                    ))
                    .child(note(
                        "Fires on in-page navigation (following a link, redirects). The text \
                         field mirrors it via set_text_on when the field isn't focused.",
                    )),
            )
    }
}
