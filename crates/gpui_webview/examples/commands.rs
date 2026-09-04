//! Tauri-style example: native Rust "commands" invoked from page JS through
//! `window.invoke(cmd, payload)`, plus a native GPUI side panel that pushes
//! events into the page. This mirrors the shape of a Tauri app: web UI in
//! the webview, privileged native logic in Rust, IPC in both directions.

use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicI64, Ordering},
};

use gpui::{
    App, Bounds, ClickEvent, Context, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_webview::{IpcRequest, IpcResult, WebViewHandle, ok, webview};
use serde::Deserialize;
use serde_json::json;

/// `greet`'s payload, decoded from JSON via `IpcRequest::payload`.
#[derive(Deserialize)]
struct GreetArgs {
    #[serde(default)]
    name: String,
}

/// Server-side state a Tauri app would normally keep behind its commands.
struct AppState {
    counter: AtomicI64,
}

/// Run one native command and return its result, the way a Tauri
/// `#[tauri::command]` function would.
fn run_command(state: &AppState, request: &IpcRequest) -> IpcResult {
    match request.cmd.as_ref() {
        "greet" => {
            let args: GreetArgs = request.payload().map_err(|err| err.to_string())?;
            let name = if args.name.is_empty() { "world" } else { &args.name };
            ok(json!({ "message": format!("Hello, {name}! This reply came from native Rust.") }))
        }
        "increment_counter" => {
            let value = state.counter.fetch_add(1, Ordering::SeqCst) + 1;
            ok(json!({ "value": value }))
        }
        "get_counter" => ok(json!({ "value": state.counter.load(Ordering::SeqCst) })),
        "system_info" => ok(json!({
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "cwd": std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default(),
        })),
        other => Err(format!("unknown command: {other}")),
    }
}

struct CommandsDemo {
    state: Rc<AppState>,
    webview_handle: Rc<RefCell<Option<WebViewHandle>>>,
}

impl CommandsDemo {
    fn new() -> Self {
        Self {
            state: Rc::new(AppState {
                counter: AtomicI64::new(0),
            }),
            webview_handle: Rc::new(RefCell::new(None)),
        }
    }

    /// Push a native-originated event into the page, the way a Tauri app
    /// emits events from Rust with `app.emit()`.
    fn broadcast_from_native(&self) {
        let Some(handle) = self.webview_handle.borrow().as_ref().cloned() else {
            return;
        };
        let value = self.state.counter.load(Ordering::SeqCst);
        let script = format!(
            "window.dispatchEvent(new CustomEvent('native-event', {{ detail: {{ value: {value} }} }}));"
        );
        handle.evaluate_javascript(&script, None);
    }
}

impl Render for CommandsDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle_cell = self.webview_handle.clone();
        let state = self.state.clone();

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x1e1e2e))
            .child(
                // Native GPUI side panel, standing in for a Tauri app's
                // native window chrome / menus / tray integration.
                div()
                    .flex()
                    .flex_col()
                    .w(px(220.0))
                    .h_full()
                    .p_3()
                    .gap_2()
                    .bg(rgb(0x181825))
                    .text_color(rgb(0xcdd6f4))
                    .child("Native side (GPUI)")
                    .child(
                        div()
                            .id("bump")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0xa6e3a1))
                            .text_color(rgb(0x1e1e2e))
                            .cursor_pointer()
                            .child("Emit native-event")
                            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, _cx| {
                                this.state.counter.fetch_add(1, Ordering::SeqCst);
                                this.broadcast_from_native();
                            })),
                    ),
            )
            .child(
                webview("commands-content")
                    .html(PAGE_HTML)
                    .devtools(true)
                    .on_ipc_message(move |request, handle, _window, _cx| {
                        let result = run_command(&state, request);
                        handle.reply(request, result);
                    })
                    .on_create_handle(move |handle, _window, _cx| {
                        *handle_cell.borrow_mut() = Some(handle);
                    })
                    .flex_1(),
            )
    }
}

const PAGE_HTML: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<style>
  body { font-family: sans-serif; background: #1e1e2e; color: #cdd6f4; margin: 0; padding: 24px; }
  h1 { font-size: 18px; }
  button { background: #89b4fa; color: #1e1e2e; border: none; border-radius: 6px; padding: 8px 14px; cursor: pointer; margin-right: 8px; }
  input { padding: 6px 8px; border-radius: 6px; border: 1px solid #45475a; background: #313244; color: #cdd6f4; }
  pre { background: #11111b; padding: 12px; border-radius: 6px; white-space: pre-wrap; }
</style>
</head>
<body>
  <h1>Web content (page JS calling native Rust commands)</h1>

  <p>
    <input id="name" placeholder="Your name" />
    <button onclick="greet()">invoke('greet')</button>
  </p>

  <p>
    <button onclick="bump()">invoke('increment_counter')</button>
    <button onclick="sysinfo()">invoke('system_info')</button>
  </p>

  <pre id="log">Ready.</pre>

  <script>
    function log(text) {
      document.getElementById('log').textContent = text;
    }

    async function greet() {
      const name = document.getElementById('name').value;
      try {
        const result = await window.invoke('greet', { name });
        log(result.message);
      } catch (err) {
        log('Error: ' + err);
      }
    }

    async function bump() {
      const result = await window.invoke('increment_counter', {});
      log('Counter is now ' + result.value);
    }

    async function sysinfo() {
      const result = await window.invoke('system_info', {});
      log(JSON.stringify(result, null, 2));
    }

    // Native -> page event, pushed by the GPUI side panel via evaluate_javascript.
    window.addEventListener('native-event', (event) => {
      log('Received native-event from Rust: ' + JSON.stringify(event.detail));
    });
  </script>
</body>
</html>"#;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1000.), px(700.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| CommandsDemo::new()),
        )
        .unwrap();
        cx.activate(true);
    });
}
