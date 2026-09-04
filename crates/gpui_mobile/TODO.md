# gpui_mobile long-term TODOs

## Native `PlatformInputHandler` / IME support for GPUI-drawn text inputs

**Status (iOS): implemented.** `ios/text_input_view.rs`'s `GPUITextInputView`
now conforms to `UITextInput` (+ `UIKeyInput` + `UITextInputTraits`), driven
by `PlatformInputHandler`/`EntityInputHandler` the same way `gpui_macos`'s
`NSTextInputClient` implementation drives it on macOS. Any `Render`-based
view built the normal way (`window.handle_input(&focus_handle,
ElementInputHandler::new(bounds, view), cx)`) now gets real native text
editing on iOS: system selection/caret placement, IME/marked-text
composition, keyboard attribute configuration (autocorrect,
autocapitalization, spellchecking, return-key label), and hardware-keyboard
input via `pressesBegan:`/`pressesEnded:`. See `examples/ios_text_input` for
an end-to-end demo built on `gpui_ce_elements`'s `EditableTextState`.

What's implemented:

- `GPUITextPosition`/`GPUITextRange` (`UITextPosition`/`UITextRange`
  subclasses wrapping UTF-16 offsets) and the full `UITextInput` method set:
  document extents, `textInRange:`/`replaceRange:withText:`, selected/marked
  text range get+set, `setMarkedText:selectedRange:`/`unmarkText`, position
  arithmetic (`positionFromPosition:offset:`,
  `positionFromPosition:inDirection:offset:`, `comparePosition:toPosition:`,
  `offsetFromPosition:toPosition:`, `positionWithinRange:farthestInDirection:`,
  `characterRangeByExtendingPosition:inDirection:`), writing-direction
  no-ops, `firstRectForRange:`/`caretRectForPosition:` (converted from
  window coordinates into the input view's coordinate space via
  `convertRect:fromView:`), `closestPositionToPoint:` (+ `withinRange:`),
  `characterRangeAtPoint:`, `inputDelegate` storage, and a lazily-created
  `UITextInputStringTokenizer`.
- `IosWindow::with_input_handler`/`has_real_input_handler`: a take-then-
  restore accessor (mirroring `gpui_macos`'s `with_input_handler`) so
  UIKit re-entrancy (e.g. a synchronous `selectedTextRange` query from
  inside a callback we're already driving) sees "no handler" instead of
  double-borrowing, and a `cx.update` failure (GPUI mid-update) is already
  absorbed as `None`/no-op by `PlatformInputHandler`'s own methods.
- `handle_text_input`/`handle_delete_backward`/`insertText:`: when a real
  input handler is set, text is routed through it (`replace_text_in_range`);
  a literal `"\n"` is dispatched as an `enter` `KeyDown` through the input
  callback first (so keymaps/`on_action` see it) and only inserted as text
  if not `default_prevented`; backspace computes the current selection and
  deletes it, or deletes one code unit before the caret — two if the
  preceding unit is a UTF-16 low surrogate, so a surrogate pair is never
  split. The legacy global-callback bridge (`crate::dispatch_text_input` +
  synthetic per-char `KeyDown`) only runs as a fallback when no input
  handler is set, so `components::material::text_input` and
  `examples/ios_browser` are unaffected.
- Hardware keyboards: `pressesBegan:`/`pressesEnded:` on
  `GPUITextInputView` map `UIPress.key.keyCode`/`.modifierFlags` through
  `ios/text_input.rs`'s existing HID table. Printable, unmodified keys are
  forwarded to `super` so UIKit still turns them into `insertText:` (IME/
  autocorrect keep working); everything else (arrows, enter, escape,
  backspace, tab, or any key with a modifier) goes through GPUI
  `KeyDown`/`KeyUp` directly. The `gpui_ios_handle_key_event` FFI entry
  point keeps working unchanged for hosts that call it directly.
- `PlatformWindow` mobile hooks: `set_text_input_configuration` maps
  `TextInputConfiguration` onto `UITextAutocorrectionType`/
  `UITextAutocapitalizationType`/`UITextSpellCheckingType`/
  `UITextSmartQuotesType`/`UITextSmartDashesType`/`UIReturnKeyType`, and
  calls `reloadInputViews` if the view is first responder;
  `show_soft_keyboard`/`hide_soft_keyboard` and
  `text_input_state_changed` (`FocusGained`/`FocusLost` → deferred
  `becomeFirstResponder`/`resignFirstResponder` via
  `performSelector:withObject:afterDelay:0`, matching the existing
  re-entrancy-avoidance pattern; `SelectionChanged`/`ContentChanged` →
  `inputDelegate` `selectionWillChange:`/`selectionDidChange:` and
  `textWillChange:`/`textDidChange:`); `update_ime_position` now actually
  repositions the (still transparent; touches fall through to the Metal
  view via the responder chain) text input view's frame over the focused
  element's bounds so autocorrect
  bubbles, the predictive-text bar, and dictation UI are placed correctly.
- `KeyboardType` (`lib.rs`) is unchanged and still layered on top via
  `show_keyboard_with_type` for choosing email/URL/number pads, since
  `TextInputConfiguration` has no keyboard-type field; the chosen type is
  re-applied on every `becomeFirstResponder`.
- The legacy `set_text_input_callback`/`dispatch_text_input` mechanism in
  `lib.rs` is kept (only `set_text_input_callback`, the public entry point,
  is marked `#[deprecated]` pointing at `EntityInputHandler`) as a fallback
  for callers that haven't migrated — `components::material::text_input`
  and `examples/ios_browser`'s native `UITextField` platform view both
  still work unchanged.

**What remains (iOS):**

- System selection handles / magnifier loupe via `UITextInteraction` —
  `selectionRectsForRange:` currently returns an empty array, so UIKit
  cannot draw its own handles or the loupe; drag-to-select from the
  system's own gesture recognizers doesn't work as a result (selection can
  still be set programmatically via `setSelectedTextRange:` / read via
  `selectedTextRange`, and predictive text / autocorrect / IME composition
  do work).
- Drag-to-select through GPUI's own touch state machine: `handle_touches`
  (`ios/window.rs`) defers `MouseDown` until finger-up to disambiguate taps
  from scroll gestures (see `TouchState`); a real press-and-drag text
  selection gesture needs that state machine to recognize "long-press or
  drag starting inside a focused text element" as a distinct case that
  begins immediately rather than waiting for lift-off.
- `positionFromPosition:inDirection:offset:`'s up/down directions are
  no-ops (no line-layout information is available at this layer) — vertical
  cursor movement from the system tokenizer/extension gestures won't move
  lines; horizontal movement and extension work.
- `crates/gpui/examples/input.rs` still hasn't been ported to run
  unmodified on iOS as an end-to-end acceptance test; `examples/ios_text_input`
  (built on `gpui_ce_elements::EditableTextState`) is the current stand-in.
- Android: `android/window.rs`'s `InputConnection`/`EditorInfo` equivalent
  of this work has not been investigated or started.

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
