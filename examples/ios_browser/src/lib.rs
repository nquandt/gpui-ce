#![cfg(target_os = "ios")]

use std::cell::Cell;
use std::rc::Rc;

use gpui::{App, ClickEvent, WindowOptions, div, prelude::*, px, rgb};
use gpui_mobile::components::platform_view_element::platform_view_element;
use gpui_mobile::packages::text_field::{create_text_field, is_focused, set_on_submit, set_text_on, TextFieldHandle};
use gpui_mobile::packages::webview::{WebViewHandle, WebViewSettings, load_url, set_on_url_changed};
use gpui_mobile::safe_area_insets;

const HOME_URL: &str = "https://www.rust-lang.org";
const URL_BAR_HEIGHT: f32 = 48.0;

struct BrowserView {
    webview: WebViewHandle,
    url_field: TextFieldHandle,
    pending_go: Rc<Cell<bool>>,
}

impl BrowserView {
    fn new() -> Self {
        let (top, _bottom, _left, _right) = safe_area_insets();
        let webview = load_url(
            HOME_URL,
            &WebViewSettings {
                top_offset: top + URL_BAR_HEIGHT,
                ..Default::default()
            },
        )
        .expect("failed to create webview");
        let url_field =
            create_text_field(HOME_URL, "Enter a URL").expect("failed to create url field");

        let this = Self {
            webview,
            url_field,
            pending_go: Rc::new(Cell::new(false)),
        };
        this.wire_url_changed();
        this.wire_submit();
        this
    }

    /// Push URL changes from in-page navigation (following a link,
    /// redirects, etc) back into the URL bar, unless the user is actively
    /// editing it. Captures a clone of the field's `Arc<PlatformViewHandle>`
    /// rather than a pointer into `self`, since this callback is invoked
    /// from a native delegate outside of GPUI's update cycle and may
    /// outlive any particular borrow of `self`.
    fn wire_url_changed(&self) {
        let Some(field_handle) = self.url_field.platform_view_handle() else {
            return;
        };
        set_on_url_changed(Some(Box::new(move |url: &str| {
            if !is_focused(&field_handle) {
                set_text_on(&field_handle, url);
            }
        })));
    }

    fn wire_submit(&self) {
        let pending_go = self.pending_go.clone();
        set_on_submit(Some(Box::new(move || {
            pending_go.set(true);
        })));
    }

    fn go(&mut self, cx: &mut gpui::Context<Self>) {
        self.url_field.resign_focus();
        let text = self.url_field.text().unwrap_or_default();
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let has_scheme = text.split_once("://").is_some_and(|(scheme, _)| {
            scheme
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        });
        let url = if has_scheme {
            text
        } else {
            format!("https://{text}")
        };
        let (top, _bottom, _left, _right) = safe_area_insets();
        match load_url(
            &url,
            &WebViewSettings {
                top_offset: top + URL_BAR_HEIGHT,
                ..Default::default()
            },
        ) {
            Ok(new_webview) => {
                self.webview = new_webview;
                self.wire_url_changed();
            }
            Err(e) => log::error!("failed to load url {url}: {e}"),
        }
        cx.notify();
    }
}

impl Render for BrowserView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        if self.pending_go.replace(false) {
            self.go(cx);
        }

        let (top, _bottom, _left, _right) = safe_area_insets();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e2e))
            .child(
                // URL bar row: native text field + go button. Padded by the
                // actual safe-area top inset so it clears the notch /
                // Dynamic Island.
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .h(px(top + URL_BAR_HEIGHT))
                    .pt(px(top))
                    .px_3()
                    .bg(rgb(0x313244))
                    .child(
                        self.url_field
                            .platform_view_handle()
                            .map(platform_view_element)
                            .unwrap_or_else(div)
                            .flex_1()
                            .h(px(36.0)),
                    )
                    .child(
                        div()
                            .id("go")
                            .px_4()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0xa6e3a1))
                            .text_color(rgb(0x1e1e2e))
                            .font_weight(gpui::FontWeight::BOLD)
                            .cursor_pointer()
                            .child("Go")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.go(cx);
                            })),
                    ),
            )
            .child(
                self.webview
                    .platform_view_handle()
                    .map(platform_view_element)
                    .unwrap_or_else(div)
                    .flex_1(),
            )
    }
}

/// C entry point called from the iOS app delegate after `gpui_ios_initialize()`.
///
/// Registers the browser view as the root window content, then hands off to
/// `gpui_mobile`'s iOS run loop.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_browser_main() {
    gpui_mobile::ios::ffi::set_app_callback(Box::new(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| BrowserView::new())
        })
        .expect("failed to open browser window");
        cx.activate(true);
    }));

    gpui_mobile::ios::ffi::gpui_ios_run_demo();
}
