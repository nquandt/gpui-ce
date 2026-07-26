# GPUI WebView Integration Spec

## Status

Proposed — July 2026

## Summary

Add a cross-platform desktop `WebView` element to GPUI-CE via a new `gpui_webview` crate, using [wry](https://github.com/tauri-apps/wry) as the webview backend. The element renders as a native child view (HWND on Windows, NSView on macOS) embedded inside GPUI's layout system, with OS-level compositing. Feature-gated behind `webview`.

---

## Goals

1. Provide a `WebView` element that can be composed in any GPUI element tree, just like `div`, `img`, or `canvas`.
2. Use wry so the underlying browser engine is platform-native (WebView2 on Windows, WKWebView on macOS/iOS).
3. Embed the webview as a **native child view** of GPUI's window — no texture copy, no offscreen rendering.
4. Forward GPUI input events (mouse, keyboard) to the webview when it has focus.
5. Keep wry as an **optional, feature-gated** dependency so users who don't need a webview don't pay the compile-time cost.
6. Support Windows, macOS, and iOS initially. Linux (via GTK) is deferred to a follow-up.

## Non-Goals

- Rendering wry content to a GPU texture for fully-GPU-composited overlays (follow-up work).
- Linux support in the initial release (wry on Linux requires GTK event loop integration which conflicts with GPUI's calloop-based approach).
- Two-way JS ↔ Rust binding API (users can use wry's `evaluate_javascript` and IPC channels directly).

---

## Architecture

### Crate Layout

```
crates/
  gpui_webview/           # New crate
    Cargo.toml
    src/
      lib.rs              # Public API, feature gates
      webview.rs          # WebView element (Element trait impl)
      webview_handle.rs   # WebViewHandle — control handle for load/eval/etc.
      platform/
        mod.rs            # Platform dispatch
        windows.rs        # Windows: HWND child window via wry
        macos.rs          # macOS: NSView subview via wry
        ios.rs            # iOS: WKWebView (bridge to gpui_mobile or direct wry)
```

### Dependency Graph

```
gpui_webview
 ├── gpui (workspace dep)
 ├── wry (optional, behind "webview" feature)
 └── raw-window-handle (already in gpui's dependency tree)
```

The workspace `Cargo.toml` gains:

```toml
[workspace.dependencies]
wry = "0.55"
```

The `gpui_webview/Cargo.toml`:

```toml
[package]
name = "gpui_webview"
version = "0.1.0"
edition = "2024"

[features]
default = ["webview"]
webview = ["dep:wry"]

[dependencies]
gpui = { workspace = true }
wry = { workspace = true, optional = true }
raw-window-handle = { workspace = true }
```

---

## Public API

### `WebView` Element

The primary user-facing type. Follows the same patterns as `Img` and `Surface`.

```rust
use gpui_webview::{WebView, WebViewHandle};

// In a View's render() method:
fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    div().child(
        WebView::new("my-webview")
            .url("https://example.com")
            .on_navigation(|url| {
                println!("Navigated to: {url}");
                true // allow navigation
            })
            .on_page_load(|title, url| {
                println!("Page loaded: {title} at {url}");
            })
    )
}
```

#### Constructor and Configuration

```rust
impl WebView {
    /// Create a new WebView element with a unique ID.
    pub fn new(id: impl Into<SharedString>) -> Self;

    /// Set the initial URL to load.
    pub fn url(mut self, url: &str) -> Self;

    /// Set initial HTML content.
    pub fn html(mut self, html: &str) -> Self;

    /// Set whether the webview background is transparent.
    pub fn transparent(mut self, transparent: bool) -> Self;

    /// Set whether the webview is dev-tools-enabled.
    pub fn devtools(mut self, enabled: bool) -> Self;

    /// Set zoom factor.
    pub fn zoom(mut self, factor: f32) -> Self;

    /// Callback fired when navigation is attempted. Return false to block.
    pub fn on_navigation(mut self, callback: impl FnMut(&str) -> bool + 'static) -> Self;

    /// Callback fired when a page finishes loading.
    pub fn on_page_load(mut self, callback: impl FnMut(&str, &str) + 'static) -> Self;

    /// Callback fired when the webview requests focus.
    pub fn on_focus(mut self, callback: impl FnMut(&mut Window, &mut App) + 'static) -> Self;
}
```

#### `Styled` and `Layout`

`WebView` implements `Styled`, so it participates in GPUI's CSS-like layout:

```rust
div()
    .child(
        WebView::new("editor")
            .url("https://monaco-editor.com")
            .w(px(800))
            .h(px(600))
    )
```

The element occupies the bounds assigned by Taffy layout. On resize, the underlying wry webview is resized to match.

### `WebViewHandle`

A handle for controlling the webview after creation. Obtained via the `on_create_handle` callback or by querying the element tree.

```rust
impl WebViewHandle {
    /// Navigate to a URL.
    pub fn load_url(&self, url: &str);

    /// Navigate to a data URL with HTML content.
    pub fn load_html(&self, html: &str);

    /// Navigate back.
    pub fn go_back(&self);

    /// Navigate forward.
    pub fn go_forward(&self);

    /// Reload the current page.
    pub fn reload(&self);

    /// Stop loading.
    pub fn stop_loading(&self);

    /// Execute JavaScript in the webview.
    pub fn evaluate_javascript(&self, script: &str, callback: Option<Box<dyn FnOnce(Result<String>)>>);

    /// Get the current page title.
    pub fn title(&self) -> SharedString;

    /// Get the current page URL.
    pub fn url(&self) -> SharedString;

    /// Set whether the webview is visible (for lazy rendering).
    pub fn set_visible(&self, visible: bool);

    /// Update the webview's bounds (called automatically during layout).
    pub fn set_bounds(&self, bounds: Bounds<Pixels>);
}
```

### Builder Function (Convenience)

```rust
/// Shorthand for creating a WebView element.
pub fn webview(id: impl Into<SharedString>) -> WebView {
    WebView::new(id)
}
```

---

## Element Lifecycle

The `WebView` element implements `gpui::Element`. Its lifecycle follows the standard three-phase pattern.

### Phase 1: `request_layout()`

Registers with Taffy for layout. No children — the webview is a leaf node.

```rust
fn request_layout(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    window: &mut Window,
    cx: &mut App,
) -> (LayoutId, Self::RequestLayoutState) {
    let style = Style {
        size: self.size,        // e.g., Size { width: Auto, height: Auto }
        ..Style::default()
    };
    let layout_id = window.request_layout(style, [], cx);
    (layout_id, ())
}
```

### Phase 2: `prepaint()`

Receives the computed bounds from Taffy. Creates the native webview on first prepaint (lazy initialization). On subsequent prepaints, updates the webview's bounds if they changed.

```rust
fn prepaint(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    _request_layout: &mut Self::RequestLayoutState,
    window: &mut Window,
    cx: &mut App,
) -> Self::PrepaintState {
    if self.handle.is_none() {
        // First time: create the native webview as a child of the GPUI window
        self.handle = Some(create_platform_webview(window, &self.config, cx));
    }

    // Update bounds if changed
    if self.last_bounds != bounds {
        self.handle.as_ref().unwrap().set_bounds(bounds);
        self.last_bounds = bounds;
    }

    // Sync visibility with GPUI's clipping
    let visible = window.is_visible_in_viewport(bounds);
    self.handle.as_ref().unwrap().set_visible(visible);

    ()
}
```

### Phase 3: `paint()`

No-op for the element itself — the native webview renders independently via the OS compositor. The element only needs to ensure the webview is positioned correctly (done in prepaint).

```rust
fn paint(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    _bounds: Bounds<Pixels>,
    _request_layout: &mut Self::RequestLayoutState,
    _prepaint: &mut Self::PrepaintState,
    _window: &mut Window,
    _cx: &mut App,
) {
    // Native webview renders itself. Nothing to paint into the scene.
}
```

---

## Platform Implementation

### Common Interface

```rust
// crates/gpui_webview/src/platform/mod.rs

pub(crate) trait PlatformWebView: Send {
    fn set_bounds(&self, bounds: Bounds<Pixels>);
    fn set_visible(&self, visible: bool);
    fn load_url(&self, url: &str);
    fn load_html(&self, html: &str);
    fn go_back(&self);
    fn go_forward(&self);
    fn reload(&self);
    fn stop_loading(&self);
    fn evaluate_javascript(&self, script: &str, callback: Option<Box<dyn FnOnce(Result<String>)>>);
    fn title(&self) -> SharedString;
    fn url(&self) -> SharedString;
    fn devtools_enabled(&self) -> bool;
    fn set_devtools_enabled(&self, enabled: bool);
}

pub(crate) fn create_platform_webview(
    window: &Window,
    config: &WebViewConfig,
    cx: &mut App,
) -> Box<dyn PlatformWebView>;
```

### Windows (`platform/windows.rs`)

Uses `wry::webview::WebViewBuilder::build_as_child()`.

```rust
use wry::webview::{WebView, WebViewBuilder};
use wry::Rect;

pub(crate) struct WindowsWebView {
    webview: WebView,
    parent_hwnd: HWND,
}

impl WindowsWebView {
    pub fn new(window: &Window, config: &WebViewConfig) -> Self {
        let hwnd = window.get_raw_handle(); // PlatformWindow::get_raw_handle()

        let mut builder = WebViewBuilder::new()
            .with_bounds(Rect {
                position: (config.bounds.origin.x.0, config.bounds.origin.y.0).into(),
                size: (config.bounds.size.width.0, config.bounds.size.height.0).into(),
            });

        if let Some(url) = &config.url {
            builder = builder.with_url(url);
        } else if let Some(html) = &config.html {
            builder = builder.with_html(html);
        }

        if config.transparent {
            builder = builder.with_transparent(true);
        }

        if config.devtools {
            builder = builder.with_devtools(true);
        }

        let webview = builder.build_as_child(&RawHwnd(hwnd)).unwrap();

        WindowsWebView { webview, parent_hwnd: hwnd }
    }
}

impl PlatformWebView for WindowsWebView {
    fn set_bounds(&self, bounds: Bounds<Pixels>) {
        // Win32: SetWindowPos on the child HWND
        unsafe {
            SetWindowPos(
                self.webview.hwnd(),
                None,
                bounds.origin.x.0 as i32,
                bounds.origin.y.0 as i32,
                bounds.size.width.0 as i32,
                bounds.size.height.0 as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    fn set_visible(&self, visible: bool) {
        let cmd = if visible { SW_SHOW } else { SW_HIDE };
        unsafe { ShowWindow(self.webview.hwnd(), cmd); }
    }

    // ... other methods delegate to wry's WebView API
}
```

**Key detail:** `wry` on Windows uses WebView2 (Edge Chromium). The `build_as_child` call creates a child HWND with the `WS_CHILD` style. The OS handles compositing — no texture copy needed.

**Raw window handle bridge:** wry's `build_as_child` requires `&W: HasWindowHandle`. GPUI's `Window` already implements `HasWindowHandle` (delegates to `platform_window`). We need a small wrapper to also expose `get_raw_handle() -> HWND` which is a GPUI-specific trait method (not part of `raw-window-handle`).

```rust
/// Bridge type that implements HasWindowHandle for use with wry.
struct RawHwnd(HWND);

unsafe impl HasRawWindowHandle for RawHwnd {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32WindowHandle::new(NonZeroIsize::new(self.0 .0 as isize).unwrap());
        RawWindowHandle::Win32(handle)
    }
}
```

Actually, since GPUI uses `raw-window-handle` 0.6 which uses the `HasWindowHandle` trait (not the older `HasRawWindowHandle`), and `Window` already implements `HasWindowHandle`, we can pass `window` directly to `build_as_child`:

```rust
let webview = builder.build_as_child(window).unwrap();
```

### macOS (`platform/macos.rs`)

Uses `wry::webview::WebViewBuilder::build_as_child()`.

```rust
use wry::webview::{WebView, WebViewBuilder};

pub(crate) struct MacosWebView {
    webview: WebView,
}

impl MacosWebView {
    pub fn new(window: &Window, config: &WebViewConfig) -> Self {
        let mut builder = WebViewBuilder::new()
            .with_bounds(Rect {
                position: (config.bounds.origin.x.0, config.bounds.origin.y.0).into(),
                size: (config.bounds.size.width.0, config.bounds.size.height.0).into(),
            });

        if let Some(url) = &config.url {
            builder = builder.with_url(url);
        } else if let Some(html) = &config.html {
            builder = builder.with_html(html);
        }

        if config.transparent {
            builder = builder.with_transparent(true);
        }

        if config.devtools {
            builder = builder.with_devtools(true);
        }

        // Window implements HasWindowHandle (returns AppKitWindowHandle wrapping NSView)
        let webview = builder.build_as_child(window).unwrap();

        MacosWebView { webview }
    }
}
```

**Key detail:** On macOS, `build_as_child` creates a new `WKWebView` and adds it as a subview of the parent's content view (the NSView returned by `HasWindowHandle`). GPUI's `HasWindowHandle` returns the `native_view` (the custom `GPUIView`), so wry will add the WKWebView as a subview of that. This means the webview sits alongside (not on top of) GPUI's Metal rendering view.

**Z-ordering consideration:** The WKWebView subview is added via `addSubview:positioned:relativeTo:`. By default it's added at the end of the subview list (on top). We may need to insert it below the Metal rendering view to avoid covering GPUI-rendered content, or use a layered approach where the webview occupies a specific region and GPUI avoids rendering underneath it.

**Alternative approach for macOS:** Instead of `build_as_child(window)`, we could use `build_as_child` targeting the `contentView` directly and manage the subview order explicitly. This requires accessing the NSView hierarchy, which GPUI already does (see the blur view insertion pattern in `gpui_macos/src/window.rs:1434`).

### iOS (`platform/ios.rs`)

iOS is handled differently. Two options:

**Option A: Bridge to existing `gpui_mobile` webview**

The `gpui_mobile` crate already has a working iOS `WKWebView` implementation via `PlatformViewRegistry`. The `gpui_webview` crate could delegate to it on iOS:

```rust
#[cfg(target_os = "ios")]
pub(crate) struct IosWebView {
    handle: Arc<PlatformViewHandle>,
}

#[cfg(target_os = "ios")]
impl IosWebView {
    pub fn new(window: &Window, config: &WebViewConfig) -> Self {
        let mut params = HashMap::new();
        if let Some(url) = &config.url {
            params.insert("url".to_string(), url.clone());
        }
        let view = PlatformViewRegistry::global()
            .create_view("webview", PlatformViewParams {
                bounds: PlatformViewBounds { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
                creation_params: params,
            });
        IosWebView { handle: Arc::new(PlatformViewHandle::new(view)) }
    }
}
```

This requires `gpui_webview` to depend on `gpui_mobile` when `target_os = "ios"`, which may be undesirable.

**Option B: Use wry directly on iOS**

Wry supports iOS via `WKWebView`. Use `WebViewBuilder::build_as_child(window)` on iOS as well. This is cleaner but means wry handles both desktop and iOS, and we don't need the `gpui_mobile` webview codepath for new code.

**Recommendation:** Option B — use wry uniformly across all platforms. This avoids duplicating webview logic and gives us a single code path. The existing `gpui_mobile` webview remains for backward compatibility but new code uses `gpui_webview`.

---

## Event Loop Integration

### The Problem

Wry does not run its own event loop. It hooks into the host application's event loop to process web content (network requests, JS timers, message pump). On each platform:

- **Windows:** WebView2 uses the Win32 message pump. GPUI's Windows backend already runs a standard `GetMessage`/`DispatchMessage` loop, so WebView2 events are processed automatically. **No additional integration needed.**
- **macOS:** WKWebView uses the Cocoa run loop (`NSRunLoop`). GPUI's macOS backend runs `NSApplication::run`, which processes the run loop. **No additional integration needed.** WKWebView timers and network callbacks fire within the existing run loop.
- **iOS:** Same as macOS — WKWebView works within the existing run loop.

### Conclusion

Event loop integration is **not a problem** on the target platforms. Wry's underlying web engines (WebView2, WKWebView) are designed to work within the host application's existing event loop. No GTK dependency exists on Windows/macOS/iOS.

---

## Input Forwarding

### Mouse Events

When the GPUI window receives mouse events, it needs to determine whether the cursor is over the webview and, if so, forward the event to wry.

**Approach: Hit-test based forwarding**

1. During `prepaint()`, the `WebView` element registers its bounds as a hit-test region.
2. GPUI's input system already performs hit-testing (see `window.rs:hit_test`). When a mouse event lands on the webview's bounds, the event is routed to the webview element.
3. The webview element forwards the event to wry via its native input API.

However, there's a subtlety: **the native webview is a child HWND/NSView, so the OS already routes mouse events to it directly.** On Windows, child HWNDs receive `WM_MOUSEMOVE`, `WM_LBUTTONDOWN`, etc. via the normal Win32 message dispatch. On macOS, the WKWebView NSView receives mouse events via the responder chain.

**This means mouse input forwarding may already work automatically** because the webview is a native child view. The OS handles routing. We only need to worry about:

- **Focus management:** When the user clicks the webview, GPUI needs to know the webview has focus so it doesn't forward keyboard events to its own text input system.
- **Cursor changes:** The webview may want to change the cursor (e.g., to a text cursor over editable content). wry handles this natively on its child view.

### Keyboard Events

Keyboard events are trickier. GPUI's text input system captures keyboard events at the window level and routes them to the focused element. If the webview has focus, keyboard events should go to the webview instead.

**Approach: Focus-based routing**

1. When the webview receives focus (via click or tab), it calls a callback to notify GPUI.
2. GPUI marks the webview element as focused.
3. GPUI's input system checks if the focused element is a webview and, if so, does not dispatch keyboard events to its own text input system.
4. The native webview receives keyboard events directly from the OS (since it's a child view with input focus).

**On Windows:** When the child HWND has focus, `WM_KEYDOWN`/`WM_KEYUP` messages go directly to the child HWND via `TranslateMessage`/`DispatchMessage`. GPUI's parent HWND won't receive them. This works automatically.

**On macOS:** The WKWebView NSView becomes the first responder when clicked. `keyDown:`/`keyUp:` events go to the responder chain, which is the WKWebView. GPUI's `GPUIView` won't receive them. This works automatically.

**Conclusion:** Input forwarding **mostly works automatically** because the webview is a native child view. The main integration point is **focus state tracking** — GPUI needs to know when the webview has focus to avoid conflicting text input handling.

---

## Scene Graph Integration

### No New Scene Primitive Needed

Unlike the `Surface` element (which inserts `PaintSurface` into the scene for GPU rendering), the `WebView` element **does not need a new scene primitive**. The native webview renders itself independently via the OS compositor. GPUI's scene graph only needs to know about the webview for:

1. **Hit-testing:** The webview's bounds should be included in hit-test results. This is handled by the `Element` trait's standard hit-test integration (any element with bounds participates in hit-testing).
2. **Clipping:** If the webview is clipped by a parent container, we need to set the clip region on the native webview. wry supports this via bounds — we just set the webview's bounds to the visible region.

### Z-Ordering

The webview is a native child view that composites independently. Its z-order relative to GPUI-rendered content is determined by the subview ordering:

- **macOS:** The WKWebView is added as a subview of the content view. By default it's on top of GPUI's Metal view. To render GPUI content *on top of* the webview, we'd need to add a transparent overlay view. For the initial implementation, the webview occupies a specific region and GPUI content is not rendered on top of it.
- **Windows:** The child HWND is a sibling of the DirectX rendering surface. Z-order is managed by `HWND_TOP`/`HWND_BOTTOM` in `SetWindowPos`. The webview is on top by default.

**Recommendation for v1:** The webview occupies a rectangular region in the layout. No GPUI content is rendered on top of it. This is the simplest model and covers the primary use case (embedding a full web page or web app).

---

## Lifecycle Management

### Creation

The webview is created lazily during the first `prepaint()` call, when bounds are known. This avoids creating a zero-size webview.

### Destruction

When the `WebView` element is removed from the element tree (i.e., the parent view no longer renders it), the element is dropped. The `PlatformWebView` trait object's `Drop` implementation calls `wry::WebView::destroy()` (or simply drops the `WebView`, which cleans up the native view).

```rust
impl Drop for WindowsWebView {
    fn drop(&mut self) {
        // wry's WebView::drop() handles cleanup
        // The child HWND is destroyed automatically
    }
}
```

### Visibility

When the element is scrolled off-screen or hidden by a parent, `set_visible(false)` is called, which hides the native view via `ShowWindow(SW_HIDE)` (Windows) or `setHidden:YES` (macOS). This stops the webview from rendering and consuming resources.

### Resizing

When GPUI's layout changes (window resize, flex reflow), the element's bounds change. During `prepaint()`, if bounds differ from the last frame, `set_bounds()` is called, which resizes the native webview.

---

## Configuration

### `WebViewConfig`

Internal configuration struct passed to the platform constructor:

```rust
pub(crate) struct WebViewConfig {
    pub id: SharedString,
    pub url: Option<String>,
    pub html: Option<String>,
    pub transparent: bool,
    pub devtools: bool,
    pub zoom: f32,
    pub bounds: Bounds<Pixels>,
}
```

### Feature Flags

```toml
# gpui_webview/Cargo.toml
[features]
default = ["webview"]
webview = ["dep:wry"]
```

When the `webview` feature is disabled, the crate exports stub types that panic on construction. This allows downstream crates to reference the types without pulling in wry, but they'll get a clear error at runtime if they try to create a webview.

---

## Testing Strategy

### Unit Tests

- Test that `WebView` element can be created and configured.
- Test that `request_layout()` returns a valid `LayoutId`.
- Test bounds calculation with various style configurations.

### Integration Tests

- Create a GPUI test window, insert a `WebView` element, verify the native webview is created.
- Test navigation: `load_url()` triggers page load callback.
- Test resize: changing the element's size resizes the native webview.
- Test visibility: scrolling off-screen hides the webview.

### Manual Tests

- Load a real website (e.g., `https://example.com`) and verify rendering.
- Test JavaScript execution via `evaluate_javascript()`.
- Test dev tools toggle.
- Test focus switching between GPUI text input and webview.
- Test window resize with webview present.
- Test multiple webviews in the same window.

---

## Migration Path

### Existing Mobile WebView

The `gpui_mobile` crate's webview implementation (via `PlatformViewRegistry`) remains unchanged. It continues to serve iOS/Android apps that use the mobile platform layer.

New desktop apps (and new iOS apps using wry) should use `gpui_webview` instead.

A future follow-up could unify the two by having `gpui_webview` wrap the mobile implementation on iOS/Android, but this is out of scope for v1.

### Existing `Surface` Element

The `Surface` element remains unchanged. It serves a different purpose (rendering external GPU textures like video frames) and is not related to webview embedding.

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| **wry version drift** — wry releases frequently; API changes could break gpui_webview | Medium | Pin wry version in workspace deps. Update deliberately. |
| **macOS z-ordering** — WKWebView subview may cover GPUI content | Medium | For v1, design layouts so webview occupies its own region. Document the constraint. |
| **macOS Metal + WKWebView compositing** — two separate rendering systems in one window | Low | This is a well-established pattern (see Electron, Tauri). The OS compositor handles it. |
| **iOS wry support maturity** — wry's iOS support is newer than desktop | Medium | Test thoroughly on iOS. Fall back to gpui_mobile webview if needed. |
| **Input focus conflicts** — GPUI text input vs webview text input | Medium | Track focus state explicitly. Disable GPUI IME when webview has focus. |
| **wry binary size** — WebView2/WebKit frameworks add to binary size | Low | Feature-gated. Users opt in. |
| **WebView2 runtime requirement** — Windows requires Edge WebView2 runtime | Low | Modern Windows 10/11 ships with it. Can bundle installer for older systems. |

---

## Implementation Phases

### Phase 1: Core Element + Windows

1. Create `crates/gpui_webview/` crate with workspace integration.
2. Implement `WebView` element with `Element` trait.
3. Implement `WindowsWebView` platform backend.
4. Wire up feature gates.
5. Add to workspace members.
6. Manual testing with example app.

### Phase 2: macOS

1. Implement `MacosWebView` platform backend.
2. Test subview integration with GPUI's NSView hierarchy.
3. Verify z-ordering and input handling.

### Phase 3: iOS

1. Implement `IosWebView` platform backend using wry.
2. Test WKWebView integration on device.
3. Compare with existing `gpui_mobile` webview for parity.

### Phase 4: Polish

1. Focus management integration with GPUI's text input system.
2. Cursor change forwarding.
3. Scroll wheel event handling.
4. Context menu integration.
5. Clipboard integration (copy/paste between GPUI and webview).

---

## Example Usage

### Basic Web Browser

```rust
use gpui::*;
use gpui_webview::webview;

struct BrowserView {
    url: SharedString,
}

impl Render for BrowserView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .size_full()
            .child(
                // URL bar (GPUI text input)
                div()
                    .h(px(40))
                    .bg(gpui::white())
                    .child(text(&self.url))
            )
            .child(
                // Web content (native webview)
                webview("browser-content")
                    .url(&self.url)
                    .flex_1()
            )
    }
}
```

### Embedded Documentation Panel

```rust
fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .flex_row()
        .size_full()
        .child(
            // Code editor (GPUI-rendered)
            div().flex_1().child(self.editor.clone())
        )
        .child(
            // Live preview (webview)
            webview("preview")
                .url("http://localhost:3000")
                .w(px(400))
                .border_l_1()
        )
}
```

---

## Open Questions

1. **Subview ordering on macOS:** Should the WKWebView be inserted below the Metal view (so GPUI content renders on top) or above (so the webview is always visible)? The answer depends on the use case. For v1, "above" is simpler and covers the full-webview case.

2. **Multiple webviews:** How should multiple webviews in the same window share input focus? wry handles this natively (each has its own NSView/HWND), but GPUI needs to track which one is focused.

3. **Print/screenshot support:** Should the webview support being screenshotted or printed? wry has `print()` and screenshot capabilities that could be exposed.

4. **Custom schemes:** Should we expose wry's custom scheme handler for serving local content to the webview (e.g., `gpui://` URLs)?

5. **IPC:** Should we provide a typed Rust ↔ JS IPC channel beyond wry's raw `evaluate_javascript` and `with_ipc_handler`?
