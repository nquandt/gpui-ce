# ios_gallery

A "feature gallery" test app for the `gpui_mobile` iOS backend: one screen
per feature area, each with a pass/fail verdict and free-text notes you
record while testing on a real device or simulator, plus a **Report** screen
that exports everything you recorded.

This crate provides the app shell — navigation, header, bottom
verdict/notes bar, the screen registry, and persistence. Individual screens
(other than Report) are filled in incrementally; until then each shows a
"Not implemented yet" placeholder and is still fully navigable.

## Building for the simulator

```sh
cargo build -p ios_gallery --target aarch64-apple-ios-sim
cd examples/ios_gallery/ios_app
xcodegen generate
xcodebuild -scheme GPUIGallery -sdk iphonesimulator \
  -destination 'platform=iOS Simulator,name=iPhone 17' \
  -configuration Debug build \
  CODE_SIGN_IDENTITY="" CODE_SIGNING_REQUIRED=NO CODE_SIGNING_ALLOWED=NO
xcrun simctl boot "iPhone 17"   # ignore "already booted"
xcrun simctl install booted <path-to>/GPUIGallery.app
xcrun simctl launch booted com.gpuice.iosgallery
```

## Building for a device

```sh
cargo build -p ios_gallery --target aarch64-apple-ios
```

Then open `examples/ios_gallery/ios_app/GPUIGallery.xcodeproj` in Xcode
(generate it first with `xcodegen generate`, since `*.xcodeproj` is
gitignored), select your development team under Signing & Capabilities,
plug in a device, and hit Run.

## Opening a specific screen

Two ways to jump straight to a screen, useful for manual testing and
simulator automation:

- **Launch argument**: `xcrun simctl launch booted com.gpuice.iosgallery --screen buttons`
- **Deep link**: `xcrun simctl openurl booted gpuigallery://screen/buttons`
  (the simulator will show a one-time "Open in GPUIGallery?" confirmation —
  tap Open)

Both funnel through `shell::request_navigation`, polled once per frame on
the main thread, so it's safe to call from the deep-link handler (which
runs off the GPUI thread).

## Recording verdicts and notes

Every non-home screen has a bottom bar with a `Works` / `Partial` / `Broken`
segmented control (a screen you haven't touched shows `Untested`) and a
`Notes` toggle that reveals a free-text field. Both are persisted
immediately through `gpui_mobile::packages::shared_preferences`, keyed by
`gallery.<screen id>.verdict` and `gallery.<screen id>.notes`, so they
survive relaunches.

## The Report screen

`screens/report.rs` shows a device summary (from `device_info` +
`package_info`), a table of every screen's recorded verdict and notes, and
the last 200 lines of the in-app event log (`src/log.rs` — a 500-entry ring
buffer that the shell and screens write to). From there you can:

- **Copy report** — copies the markdown report to the clipboard
- **Share…** — opens the share sheet with the markdown report
- **Save to Documents** — writes `gallery-report.md` to the app's Documents
  directory (shows up in the Files app since `UIFileSharingEnabled` and
  `LSSupportsOpeningDocumentsInPlace` are set) and shows the saved path
- **Reset all verdicts** — clears every screen's verdict, notes, and the
  event log

## Screen registry

Screens live one-per-file under `src/screens/`, each exposing a
`descriptor() -> ScreenDescriptor` (id, title, category, blurb, and a
`build` function that constructs the screen's `AnyView`). `src/screens/mod.rs`
lists them in the fixed display order used on the home screen, and
`src/screens/common.rs` has small shared helpers (`section`, `row`,
`button`, `label`, `mono`, `kv`, `note`) that keep screens visually
consistent.

| id | title | category | file |
| --- | --- | --- | --- |
| `buttons` | Buttons & taps | Core UI | `screens/buttons.rs` |
| `text_input` | Text input | Core UI | `screens/text_input.rs` |
| `scrolling` | Scrolling & lists | Core UI | `screens/scrolling.rs` |
| `gestures` | Gestures & touch | Core UI | `screens/gestures.rs` |
| `typography` | Typography | Core UI | `screens/typography.rs` |
| `colors_shapes` | Colors & shapes | Core UI | `screens/colors_shapes.rs` |
| `images` | Images & SVG | Core UI | `screens/images.rs` |
| `animations` | Animations | Core UI | `screens/animations.rs` |
| `layout_insets` | Layout & insets | Window & system | `screens/layout_insets.rs` |
| `appearance` | Appearance | Window & system | `screens/appearance.rs` |
| `clipboard` | Clipboard | Window & system | `screens/clipboard.rs` |
| `dialogs` | Dialogs & pickers | Window & system | `screens/dialogs.rs` |
| `haptics` | Haptics & vibration | Window & system | `screens/haptics.rs` |
| `lifecycle` | Lifecycle & events | Window & system | `screens/lifecycle.rs` |
| `storage` | Storage | Window & system | `screens/storage.rs` |
| `performance` | Performance | Window & system | `screens/performance.rs` |
| `device_info` | Device info | Hardware & media | `screens/device_info.rs` |
| `sensors` | Sensors | Hardware & media | `screens/sensors.rs` |
| `camera` | Camera | Hardware & media | `screens/camera.rs` |
| `media` | Video & audio | Hardware & media | `screens/media.rs` |
| `microphone` | Microphone | Hardware & media | `screens/microphone.rs` |
| `security_notify` | Security & notifications | Hardware & media | `screens/security_notify.rs` |
| `webview` | WebView | Hardware & media | `screens/webview.rs` |
| `location_maps` | Location & maps | Hardware & media | `screens/location_maps.rs` |
| `components` | Components | Hardware & media | `screens/components.rs` |
| `report` | Report | Meta | `screens/report.rs` |
