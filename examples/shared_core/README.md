# Shared core example

One GPUI app core, two hosts. The `core` crate holds every view, all state and
all assets, and never touches a platform API. Each host crate only builds the
`Application` and hands control to the core.

```
core/      app-core    views, state, embedded font and SVG   (all targets)
desktop/   app-desktop native window via gpui_platform        (Win/macOS/Linux)
web/       app-web     wasm32 canvas via gpui_web + trunk     (browser)
```

The core demonstrates the three services a real app needs on every target:

- **HTTP** through `cx.http_client()` and `HttpRequest`.
- **Persistence** through `cx.key_value_store()` (a JSON file on desktop,
  `localStorage` on the web).
- **Assets** through an `AssetSource` that embeds the font and an SVG.
- **File picking** through `cx.file_picker()`, which returns bytes so the
  browser's `<input type=file>` and native dialogs look the same to the core.

## Desktop

```bash
cd examples/shared_core
cargo run -p app-desktop
```

## Web

Requires `trunk` (`cargo install trunk --locked`) and a nightly toolchain with
`rust-src`; `web/rust-toolchain.toml` selects it.

```bash
cd examples/shared_core/web
trunk serve --release
```

Open `http://127.0.0.1:8080/`. Add `?backend=webgl` to force the WebGL2 path.
Use `--release`: the debug build logs at DEBUG level and is slow enough to look
frozen.

## Adding a mobile host

Follow `docs/cross-platform-setup.md`. The iOS host installs the
`NSURLSession` HTTP client and the `NSUserDefaults` key-value store itself, so
the core runs unchanged.
