# Cross-Platform GPUI-CE Setup Guide

Build Rust GUI apps that run on **Windows**, **macOS**, **Linux**, **iOS**, and **Android** from a single codebase using [gpui-ce](https://github.com/gpui-ce/gpui-ce) and [gpui-mobile](https://github.com/itsbalamurali/gpui-mobile).

## Overview

GPUI-CE is a community fork of Zed's GPU-accelerated UI framework. It supports desktop platforms natively (via `gpui_platform`) and mobile platforms via `gpui_mobile` (iOS via Metal/wgpu, Android via Vulkan/wgpu).

Your application has three layers:

```
┌─────────────────────────────────────────────────┐
│  Shared UI Code (Render trait, components)      │  ← platform-agnostic
├────────────────┬───────────────┬────────────────┤
│  gpui_platform │  gpui_mobile  │  gpui_mobile   │  ← platform entry points
│  (desktop)     │  (iOS)        │  (Android)     │
├────────────────┼───────────────┼────────────────┤
│  Win/macOS/    │  UIKit +      │  NDK +         │  ← OS specifics
│  Linux/Wayland │  Metal        │  Vulkan        │
└────────────────┴───────────────┴────────────────┘
```

## Prerequisites

### All Platforms

- **Rust** 1.75+ (stable)
- **Git**

### Desktop

| Platform | Requirements |
|----------|-------------|
| **Windows** | MSVC build tools (Visual Studio Build Tools or full VS), Windows SDK |
| **macOS** | Xcode + `xcode-select --install` |
| **Linux** | `pkg-config`, `libfontconfig-dev`, `libwayland-dev`, `libxkbcommon-dev`, `libvulkan-dev` |

```bash
# Linux (Ubuntu/Debian)
sudo apt install build-essential pkg-config libfontconfig-dev \
  libwayland-dev libxkbcommon-dev libvulkan-dev
```

### iOS

- **macOS** with **Xcode 15+** installed
- **XcodeGen**: `brew install xcodegen`
- Apple Developer account (for device signing)

```bash
rustup target add aarch64-apple-ios
# For simulator:
rustup target add aarch64-apple-ios-sim
```

### Android

- **Android SDK** (API 26+)
- **Android NDK** r25+ (install via Android Studio → SDK Manager → SDK Tools → NDK)
- **cargo-ndk**: `cargo install cargo-ndk`

```bash
rustup target add aarch64-linux-android
```

Set `ANDROID_NDK_HOME` or let cargo-ndk auto-detect from `$HOME/Library/Android/sdk/ndk/<latest>/`.

## Project Structure

```
my-gpui-app/
├── Cargo.toml                          # workspace root
│
├── crates/
│   ├── app-core/                       # shared UI code (all platforms)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # components, state, views
│   │       └── screens/
│   │           ├── mod.rs
│   │           ├── home.rs
│   │           └── counter.rs
│   │
│   ├── app-desktop/                    # desktop binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   │
│   ├── app-ios/                        # iOS static library + binary
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   └── lib.rs
│   │   └── project.yml                 # XcodeGen spec
│   │
│   └── app-android/                    # Android cdylib
│       ├── Cargo.toml
│       ├── .cargo/config.toml
│       └── src/
│           └── lib.rs
│
├── gradle/                             # Android Gradle project
│   ├── build.gradle.kts
│   ├── settings.gradle.kts
│   └── app/
│       ├── build.gradle.kts
│       └── src/main/
│           ├── AndroidManifest.xml
│           └── jniLibs/
```

## Step 1: Workspace Root

```toml
# Cargo.toml
[workspace]
members = [
    "crates/app-core",
    "crates/app-desktop",
    "crates/app-ios",
    "crates/app-android",
]
resolver = "3"

[workspace.dependencies]
gpui = { path = "../../crates/gpui/", default-features = false }
gpui_platform = { path = "../../crates/gpui_platform/" }
gpui_mobile = { path = "../../crates/gpui_mobile/" }
```

> **Note:** When depending on gpui-ce from within the workspace, use path dependencies
> pointing to the gpui-ce `crates/` directory. When using gpui-ce as an external
> dependency in a separate project, use `gpui = { package = "gpui-ce", version = "0.3" }`.

## Step 2: Shared UI Code (`app-core`)

This is where all your views, components, and application state live. The code is
**fully platform-agnostic** — no `#[cfg]` attributes needed.

```toml
# crates/app-core/Cargo.toml
[package]
name = "app-core"
version = "0.1.0"
edition = "2024"

[dependencies]
gpui = { workspace = true }
```

```rust
// crates/app-core/src/lib.rs
use gpui::{
    App, ClickEvent, Context, Render, Window, div, prelude::*, px, rgb,
};

// ═══════════════════════════════════════════════════════════════
// Counter — the core view shared across all platforms
// ═══════════════════════════════════════════════════════════════

pub struct Counter {
    count: i32,
}

impl Counter {
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

impl Render for Counter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.count;

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .size_full()
            .bg(rgb(0x1e1e2e))
            .child(
                div()
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xcdd6f4))
                    .child(format!("{count}")),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .id("decrement")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x45475a))
                            .text_color(rgb(0xcdd6f4))
                            .text_lg()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x585b70)))
                            .child("-")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.count -= 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("increment")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0xa6e3a1))
                            .text_color(rgb(0x1e1e2e))
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x94e2d5)))
                            .child("+")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.count += 1;
                                cx.notify();
                            })),
                    ),
            )
    }
}

// ═══════════════════════════════════════════════════════════════
// Helper: open the main window with the Counter view
// ═══════════════════════════════════════════════════════════════

pub fn open_main_window(cx: &mut App) {
    cx.open_window(
        gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(
                gpui::Bounds::centered(None, gpui::size(px(400.), px(300.)), cx),
            )),
            ..Default::default()
        },
        |_, cx| cx.new(|_| Counter::new()),
    )
    .unwrap();
    cx.activate(true);
}
```

Key patterns:
- **`impl Render`** — all views implement this trait; it's called every frame
- **`div()`** — the universal layout element (like a `<div>` in HTML)
- **`.child()`** — attach child elements or text
- **`.on_click(cx.listener(...))`** — handle events with access to `&mut Self`
- **`cx.notify()`** — tell GPUI to re-render after state changes

## Step 3: Desktop Entry Point

```toml
# crates/app-desktop/Cargo.toml
[package]
name = "app-desktop"
version = "0.1.0"
edition = "2024"

[dependencies]
app-core = { path = "../app-core/" }
gpui = { workspace = true }
gpui_platform = { workspace = true }
```

```rust
// crates/app-desktop/src/main.rs
use gpui::{App};

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        app_core::open_main_window(cx);
    });
}
```

That's it. `gpui_platform::application()` returns a `gpui::Application` configured
for the current desktop OS (Windows/macOS/Linux). The `run()` method blocks until
the app exits.

### Running

```bash
cargo run -p app-desktop
```

## Step 4: iOS Entry Point

```toml
# crates/app-ios/Cargo.toml
[package]
name = "app-ios"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["staticlib", "rlib"]

[dependencies]
app-core = { path = "../app-core/" }
gpui = { workspace = true }
gpui_mobile = { workspace = true }

[target.'cfg(target_os = "ios")'.dependencies]
objc2 = "0.6"
```

```rust
// crates/app-ios/src/lib.rs
extern crate gpui_mobile;

use gpui::{prelude::*, App, WindowOptions};

// ═══════════════════════════════════════════════════════════════
// Logger — routes Rust log::info! to NSLog (visible in devicectl)
// ═══════════════════════════════════════════════════════════════

struct NsLogLogger;

impl log::Log for NsLogLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool { true }
    fn log(&self, record: &log::Record) {
        let msg = format!("[{}] {}: {}", record.level(), record.target(), record.args());
        nslog(&msg);
    }
    fn flush(&self) {}
}

fn nslog(msg: &str) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    unsafe {
        extern "C" { fn NSLog(fmt: *mut AnyObject, ...); }
        let c_msg = std::ffi::CString::new(msg).unwrap_or_default();
        let ns_msg: *mut AnyObject = msg_send![class!(NSString), alloc];
        let ns_msg: *mut AnyObject = msg_send![ns_msg, initWithUTF8String: c_msg.as_ptr()];
        let c_fmt = std::ffi::CString::new("%@").unwrap_or_default();
        let ns_fmt: *mut AnyObject = msg_send![class!(NSString), alloc];
        let ns_fmt: *mut AnyObject = msg_send![ns_fmt, initWithUTF8String: c_fmt.as_ptr()];
        NSLog(ns_fmt, ns_msg);
    }
}

// ═══════════════════════════════════════════════════════════════
// iOS entry point — called from main.m before gpui_ios_run_demo()
// ═══════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_register_app() {
    let _ = log::set_logger(&NsLogLogger).map(|()| log::set_max_level(log::LevelFilter::Info));
    std::panic::set_hook(Box::new(|info| {
        nslog(&format!("PANIC: {info}"));
    }));

    gpui_mobile::ios::ffi::set_app_callback(Box::new(|cx: &mut App| {
        app_core::open_main_window(cx);
    }));
}

// ═══════════════════════════════════════════════════════════════
// Binary entry point (optional — for `cargo run` during dev)
// ═══════════════════════════════════════════════════════════════

pub fn ios_main() {
    gpui_ios_register_app();
    gpui_mobile::ios::ffi::run_app();
}
```

### XcodeGen Spec

Create `crates/app-ios/project.yml` for Xcode project generation:

```yaml
name: AppIOS
options:
  bundleIdPrefix: com.example
  deploymentTarget:
    iOS: "13.0"
targets:
  AppIOS:
    type: application
    platform: iOS
    sources:
      - name: main.m
      - path: ../../target/aarch64-apple-ios/debug/libapp_ios.a
        buildSettings:
          OTHER_LDFLAGS: ["-force_load"]
    settings:
      PRODUCT_NAME: AppIOS
      INFOPLIST_FILE: Info.plist
      LD_RUNPATH_SEARCH_PATHS: ["$(inherited)"]
```

### Building for iOS

```bash
# Device
rustup target add aarch64-apple-ios
cargo build --target aarch64-apple-ios -p app-ios
cd crates/app-ios && xcodegen generate --spec project.yml
xcodebuild -project AppIOS.xcodeproj -scheme AppIOS build

# Simulator
rustup target add aarch64-apple-ios-sim
cargo build --target aarch64-apple-ios-sim -p app-ios
```

## Step 5: Android Entry Point

```toml
# crates/app-android/Cargo.toml
[package]
name = "app-android"
version = "0.1.0"
edition = "2024"

[lib]
name = "app_android"
crate-type = ["cdylib"]

[dependencies]
app-core = { path = "../app-core/" }
gpui = { workspace = true }
gpui_mobile = { workspace = true }

[target.'cfg(target_os = "android")'.dependencies]
android-activity = { version = "0.6", features = ["native-activity"] }
android_logger = "0.15"
log = "0.4"
```

```toml
# crates/app-android/.cargo/config.toml
[build]
target = "aarch64-linux-android"

[target.aarch64-linux-android]
linker = "aarch64-linux-android35-clang"
```

```rust
// crates/app-android/src/lib.rs
extern crate gpui_mobile;

use gpui::{prelude::*, App, WindowOptions};
use gpui_mobile::android::jni;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("my-app"),
    );
    jni::install_panic_hook();

    let _platform = jni::init_platform(&app);
    let shared = jni::shared_platform().expect("platform not initialised");

    gpui::Application::with_platform(shared.into_rc()).run(|cx: &mut App| {
        app_core::open_main_window(cx);
    });
}
```

### Building for Android

```bash
# Build native .so
cargo ndk -t arm64-v8a -o gradle/app/src/main/jniLibs build -p app-android

# Build APK
cd gradle && ./gradlew assembleDebug

# Install & run
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.example.myapp/android.app.NativeActivity
```

## Platform-Specific APIs

When building for mobile, `gpui_mobile` provides APIs that are no-ops on desktop:

### Safe Area Insets

```rust
// Get safe area insets (top, bottom, left, right) in logical points
let (top, bottom, left, right) = gpui_mobile::safe_area_insets();

// Use in your render:
div()
    .pt(px(top))
    .pb(px(bottom))
    .child(content)
```

### Software Keyboard

```rust
// Show/hide the software keyboard
gpui_mobile::show_keyboard();
gpui_mobile::hide_keyboard();

// Show with a specific keyboard type
gpui_mobile::show_keyboard_with_type(gpui_mobile::KeyboardType::NumberPad);

// Query current keyboard height (for layout adjustments)
let height = gpui_mobile::keyboard_height();
```

### Text Input Callback

```rust
gpui_mobile::set_text_input_callback(Some(Box::new(|text: &str| {
    // Handle text from the software keyboard
})));

// Clear the callback
gpui_mobile::set_text_input_callback(None);
```

### Status Bar Styling

```rust
gpui_mobile::set_system_chrome(&gpui_mobile::SystemChromeStyle {
    status_bar_color: Some(0x1e1e2e),       // Catppuccin base
    status_bar_style: gpui_mobile::StatusBarContentStyle::Light,
    navigation_bar_color: Some(0x1e1e2e),
});
```

## Building & Running — Quick Reference

| Platform | Command |
|----------|---------|
| **Windows** | `cargo run -p app-desktop` |
| **macOS** | `cargo run -p app-desktop` |
| **Linux** | `cargo run -p app-desktop` |
| **iOS device** | `cargo build --target aarch64-apple-ios -p app-ios` |
| **iOS simulator** | `cargo build --target aarch64-apple-ios-sim -p app-ios` |
| **Android** | `cargo ndk -t arm64-v8a -o gradle/app/src/main/jniLibs build -p app-android` |

## Troubleshooting

### "font-kit" or system font panics on Android

GPUI tries to resolve system fonts that don't exist on Android. Ensure `gpui_mobile` loads fonts from `/system/fonts/` during platform init. If you see `.SystemUIFont` panics, the font system hasn't been initialized.

### Black screen on Android

- Ensure `window.request_frame()` is called every event loop iteration
- Check that `Application::with_platform(...).run(...)` is used (not `Application::new().run()`)
- Verify the `InitWindow` lifecycle event was processed before opening windows

### `ANativeActivity_onCreate` not found

The `.so` doesn't export the symbol. Ensure:
1. `crate-type = ["cdylib"]` in Cargo.toml
2. The lib name in `build.gradle.kts` matches the Cargo lib name
3. The `AndroidManifest.xml` placeholder matches

### iOS: linker errors with font-kit

Font-kit requires system frameworks. Ensure your Xcode project links against `CoreText`, `CoreGraphics`, and `CoreFoundation`. XcodeGen handles this when the static lib is added to the target.

### Deadlock on Android

The `request_frame` callback must not hold the window lock while invoking the GPUI paint cycle. The gpui-mobile implementation takes the callback out of the lock before invocation.

### Windows: "windows-manifest" feature

On Windows, gpui needs the `windows-manifest` feature for proper DPI awareness. This is enabled by default in gpui-ce but must be explicitly enabled if using `default-features = false`:

```toml
gpui = { path = "../gpui/", default-features = false, features = ["windows-manifest"] }
```
