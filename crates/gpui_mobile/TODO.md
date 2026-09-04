# gpui_mobile long-term TODOs

## Native `PlatformInputHandler` / IME support for GPUI-drawn text inputs

**Status:** not started. Current workaround: `examples/ios_browser` sidesteps
this entirely by embedding a real native `UITextField` as a platform view
(`packages::text_field`) instead of a GPUI-rendered text input.

**The gap:** GPUI's real text-input story is `EntityInputHandler` /
`ElementInputHandler` (see `crates/gpui/src/input.rs`, and the reference
implementation at `crates/gpui/examples/input.rs`, formerly Zed's editor
input handling). Any `Render`-based view implements `EntityInputHandler`
(`text_for_range`, `selected_text_range`, `marked_text_range`,
`replace_text_in_range`, `replace_and_mark_text_in_range`,
`bounds_for_range`, `character_index_for_point`, etc.), calls
`window.handle_input(&focus_handle, ElementInputHandler::new(bounds, view), cx)`
during paint, and from then on the platform's `PlatformInputHandler`
(`crates/gpui/src/platform.rs`) is responsible for bridging that to the
OS's real text-input APIs — on macOS that's `NSTextInputClient`, on iOS it
would be UIKit's `UITextInput` protocol.

`gpui_mobile`'s iOS window (`ios/window.rs`) implements
`set_input_handler`/`take_input_handler` (just storage — see line ~1443) but
nothing in the crate ever drives `UITextInput` from it. Instead there's a
parallel, much more primitive bridge:

- `GPUITextInputView` (`ios/window.rs`) implements only the minimal
  `UIKeyInput` protocol (`insertText:`, `deleteBackward`, `hasText`) — no
  `UITextInput` conformance at all (no marked text / IME composition, no
  selection ranges reported to the system, no caret/selection rects for
  the system to draw handles or the input-method candidate UI over).
- Typed characters flow through a single global callback
  (`set_text_input_callback`/`dispatch_text_input` in `lib.rs`) as raw
  strings, or in `PlatformInput::KeyDown` events — not through
  `EntityInputHandler` at all.
- This means **any** GPUI-drawn text input on mobile (not just our one URL
  bar) has no real cursor placement from touch, no drag-to-select, no
  double-tap word selection, no system selection handles/magnifier loupe,
  and no IME/marked-text support (predictive text insertion works but
  composing input like Pinyin/Kana does not). It's "type characters into a
  string," not "text editing."

**What the real fix looks like:**

1. Make `GPUITextInputView` actually conform to `UITextInput`
   (`objc2::runtime::AnyProtocol::get(c"UITextInput")`), implementing at
   minimum: `selectedTextRange` / `setSelectedTextRange:`,
   `markedTextRange`, `setMarkedText:selectedRange:`, `unmarkText`,
   `textInRange:`, `replaceRange:withText:`, `positionFromPosition:offset:`,
   `comparePosition:toPosition:`, `firstRectForRange:` /
   `caretRectForPosition:`, `closestPositionToPoint:`. These map directly
   onto `PlatformInputHandler`'s existing methods
   (`crates/gpui/src/platform.rs`) — this is bridging work, not designing a
   new API.
2. Wire `IosWindow::set_input_handler`/`take_input_handler` so that when
   GPUI focuses a view implementing `EntityInputHandler`, the iOS layer
   calls those `UITextInput` methods through to the stored
   `PlatformInputHandler`, the same way `gpui_macos`'s
   `NSTextInputClient` implementation does today (good reference: whatever
   macOS platform file implements `NSTextInputClient` — check
   `crates/gpui_macos/src` for the exact file).
3. Retire (or keep only as a Android/older-fallback path) the
   `UIKeyInput`-only bridge and the global `set_text_input_callback` /
   `dispatch_text_input` mechanism in `lib.rs` once `EntityInputHandler`
   flows correctly — they were a stopgap.
4. `crates/gpui/examples/input.rs` is the right end-to-end example to
   validate against: get that example running unmodified on iOS (it's
   currently desktop-only) as the acceptance bar.
5. Do the same investigation for Android (`android/window.rs`) — same gap
   likely exists via `InputConnection`/`EditorInfo`, which is Android's
   equivalent of `UITextInput`.

**Why this matters:** once this lands, *every* GPUI text input (search
bars, forms, chat composers, code editors — anything built the normal
`Render` + `EntityInputHandler` way) gets real native text editing on
mobile, not just one hand-rolled native `UITextField` per screen. The
`packages::text_field` native-`UITextField` embedding
(`packages/text_field/`, `ios/platform_view.rs`'s `"text_field"` platform
view type) can stay as a lighter-weight option for simple single-line
inputs even after this lands, but shouldn't be the *only* way to get
working text selection.

## Other known gaps (lower priority, noted in code)

- `packages/webview/ios.rs`: `evaluate_javascript`, `go_back`, `reload`,
  `stop_loading` are now wired up to the real `WKWebView*` behind the
  `WebViewHandle`, using the same
  `handle.platform_handle.as_ref()?.inner().as_any().downcast_ref::<IosPlatformView>()`
  pattern `packages::text_field` uses to reach its native `UITextField`
  (see `native_webview_ptr()` in `packages/webview/ios.rs`). Specifically:
  `evaluate_javascript` calls `evaluateJavaScript:completionHandler:` with
  a `block2::RcBlock` that logs (but does not propagate) a JS-side error;
  `go_back`/checks `canGoBack` before calling `goBack`; `reload` calls
  `reload`; `stop_loading` calls `stopLoading`. All four return
  `Err("No active WebView")` if the platform view has been disposed or
  isn't an `IosPlatformView` (e.g. called before the view is inserted).
  Not yet done: a `go_forward` entry point (Android's `webview/android.rs`
  helper class has no `goForward` counterpart either, so adding one means
  touching both platforms' helper surface, not just iOS), and
  `evaluate_javascript`'s result value (the completion handler currently
  discards the JS expression's return value rather than surfacing it to
  the caller).
