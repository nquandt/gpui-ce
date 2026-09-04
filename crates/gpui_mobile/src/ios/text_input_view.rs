//! `GPUITextInputView`: a real `UITextInput` conformer bridging UIKit's text
//! input system to GPUI's `PlatformInputHandler` / `EntityInputHandler`.
//!
//! This view is an invisible (alpha ~0) subview positioned over the focused
//! text element. It keeps `userInteractionEnabled` on (UIKit refuses to make
//! a non-interactive view first responder) but does not override any touch
//! handling, so `UIResponder`'s default implementation forwards touches up
//! the responder chain to the Metal view underneath. It exists purely so
//! `becomeFirstResponder` gives UIKit a first responder that conforms to
//! `UITextInput`, which is what makes the software keyboard, autocorrect,
//! predictive text, and IME composition talk to us instead of being
//! silently unavailable.
//!
//! All offsets exchanged with `PlatformInputHandler` are UTF-16 code unit
//! offsets (see `crates/gpui/src/platform.rs`), matching what `NSString`
//! (and therefore `UITextPosition`/`UITextRange` for our purposes) uses.

use super::window::IosWindow;
use objc2::runtime::{AnyClass, AnyObject, AnyProtocol, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use std::ffi::c_void;
use std::sync::Once;

use super::cg_types::{ObjcCGPoint, ObjcCGRect};

const GPUI_WINDOW_IVAR: &str = "gpui_window_ptr";
const OFFSET_IVAR: &str = "_gpui_offset";
const START_IVAR: &str = "_gpui_start";
const END_IVAR: &str = "_gpui_end";

static TEXT_POSITION_CLASS_REGISTERED: Once = Once::new();
static TEXT_RANGE_CLASS_REGISTERED: Once = Once::new();
static TEXT_INPUT_VIEW_CLASS_REGISTERED: Once = Once::new();

// ── GPUITextPosition ──────────────────────────────────────────────────────

/// Registers `GPUITextPosition`, a trivial `UITextPosition` subclass that
/// just wraps a UTF-16 offset. `UITextPosition` is documented as an opaque
/// abstract class — UIKit never inspects its contents, only compares/uses
/// the ones our own `UITextInput` methods hand back — so a bare offset is
/// sufficient.
fn register_text_position_class() -> &'static AnyClass {
    TEXT_POSITION_CLASS_REGISTERED.call_once(|| {
        let superclass = class!(UITextPosition);
        let mut decl = ClassBuilder::new(c"GPUITextPosition", superclass).unwrap();
        decl.add_ivar::<isize>(c"_gpui_offset");
        decl.register();
    });
    class!(GPUITextPosition)
}

fn make_position(offset: isize) -> *mut AnyObject {
    unsafe {
        let cls = register_text_position_class();
        let obj: *mut AnyObject = msg_send![cls, alloc];
        let obj: *mut AnyObject = msg_send![obj, init];
        #[allow(deprecated)]
        {
            *(*obj).get_mut_ivar::<isize>(OFFSET_IVAR) = offset;
        }
        // UIKit expects these accessor-style results to be autoreleased;
        // returning the +1 from alloc/init would leak one object per query.
        msg_send![obj, autorelease]
    }
}

/// Reads the UTF-16 offset out of a `GPUITextPosition`. Returns `None` for
/// nil (UIKit passes nil for "no position" in a number of callbacks).
unsafe fn position_offset(pos: *mut AnyObject) -> Option<isize> {
    if pos.is_null() {
        return None;
    }
    #[allow(deprecated)]
    unsafe {
        Some(*(*pos).get_ivar::<isize>(OFFSET_IVAR))
    }
}

// ── GPUITextRange ─────────────────────────────────────────────────────────

fn register_text_range_class() -> &'static AnyClass {
    TEXT_RANGE_CLASS_REGISTERED.call_once(|| {
        let superclass = class!(UITextRange);
        let mut decl = ClassBuilder::new(c"GPUITextRange", superclass).unwrap();
        decl.add_ivar::<isize>(c"_gpui_start");
        decl.add_ivar::<isize>(c"_gpui_end");

        unsafe extern "C" fn is_empty(this: *mut AnyObject, _sel: Sel) -> Bool {
            #[allow(deprecated)]
            unsafe {
                let start = *(*this).get_ivar::<isize>(START_IVAR);
                let end = *(*this).get_ivar::<isize>(END_IVAR);
                Bool::new(start == end)
            }
        }

        unsafe extern "C" fn start(this: *mut AnyObject, _sel: Sel) -> *mut AnyObject {
            #[allow(deprecated)]
            unsafe {
                let start = *(*this).get_ivar::<isize>(START_IVAR);
                make_position(start)
            }
        }

        unsafe extern "C" fn end(this: *mut AnyObject, _sel: Sel) -> *mut AnyObject {
            #[allow(deprecated)]
            unsafe {
                let end = *(*this).get_ivar::<isize>(END_IVAR);
                make_position(end)
            }
        }

        unsafe {
            decl.add_method(
                sel!(isEmpty),
                is_empty as unsafe extern "C" fn(*mut AnyObject, Sel) -> Bool,
            );
            decl.add_method(
                sel!(start),
                start as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(end),
                end as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
        }

        decl.register();
    });
    class!(GPUITextRange)
}

fn make_range(start: isize, end: isize) -> *mut AnyObject {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    unsafe {
        let cls = register_text_range_class();
        let obj: *mut AnyObject = msg_send![cls, alloc];
        let obj: *mut AnyObject = msg_send![obj, init];
        #[allow(deprecated)]
        {
            *(*obj).get_mut_ivar::<isize>(START_IVAR) = start;
            *(*obj).get_mut_ivar::<isize>(END_IVAR) = end;
        }
        // See `make_position`: hand UIKit an autoreleased object.
        msg_send![obj, autorelease]
    }
}

unsafe fn range_offsets(range: *mut AnyObject) -> Option<(isize, isize)> {
    if range.is_null() {
        return None;
    }
    #[allow(deprecated)]
    unsafe {
        let start = *(*range).get_ivar::<isize>(START_IVAR);
        let end = *(*range).get_ivar::<isize>(END_IVAR);
        Some((start, end))
    }
}

fn clamp_range(start: isize, end: isize, len: isize) -> std::ops::Range<usize> {
    let s = start.clamp(0, len) as usize;
    let e = end.clamp(0, len) as usize;
    if s <= e { s..e } else { e..s }
}

// ── helpers to reach the IosWindow from a view ─────────────────────────────

unsafe fn window_from_view(this: *mut AnyObject) -> Option<&'static IosWindow> {
    #[allow(deprecated)]
    unsafe {
        let ptr: *mut c_void = *(*this).get_ivar(GPUI_WINDOW_IVAR);
        if ptr.is_null() {
            None
        } else {
            Some(&*(ptr as *const IosWindow))
        }
    }
}

fn utf16_len(window: &IosWindow) -> usize {
    window
        .with_input_handler(|h| h.text_length_utf16())
        .flatten()
        .unwrap_or(0)
}

// ── GPUITextInputView ──────────────────────────────────────────────────────

/// Registers `GPUITextInputView`, a full `UITextInput` (+ `UIKeyInput` +
/// `UITextInputTraits`) conformer.
pub(crate) fn register_text_input_view_class() -> &'static AnyClass {
    TEXT_INPUT_VIEW_CLASS_REGISTERED.call_once(|| {
        let superclass = class!(UIView);
        let mut decl = ClassBuilder::new(c"GPUITextInputView", superclass).unwrap();

        for proto_name in [c"UIKeyInput", c"UITextInput", c"UITextInputTraits"] {
            if let Some(protocol) = AnyProtocol::get(proto_name) {
                decl.add_protocol(protocol);
            }
        }

        decl.add_ivar::<*mut c_void>(c"gpui_window_ptr");
        decl.add_ivar::<isize>(c"_keyboardType");
        decl.add_ivar::<isize>(c"_autocorrectionType");
        decl.add_ivar::<isize>(c"_autocapitalizationType");
        decl.add_ivar::<isize>(c"_spellCheckingType");
        decl.add_ivar::<isize>(c"_smartQuotesType");
        decl.add_ivar::<isize>(c"_smartDashesType");
        decl.add_ivar::<isize>(c"_smartInsertDeleteType");
        decl.add_ivar::<isize>(c"_returnKeyType");
        decl.add_ivar::<*mut c_void>(c"_inputDelegate");
        decl.add_ivar::<*mut c_void>(c"_tokenizer");

        // ---- UIKeyInput ----

        unsafe extern "C" fn has_text(this: *mut AnyObject, _sel: Sel) -> Bool {
            unsafe {
                match window_from_view(this) {
                    Some(window) if window.has_real_input_handler() => {
                        Bool::new(utf16_len(window) > 0)
                    }
                    Some(_) => Bool::YES, // legacy fallback path
                    None => Bool::NO,
                }
            }
        }

        unsafe extern "C" fn insert_text(this: *mut AnyObject, _sel: Sel, text: *mut AnyObject) {
            unsafe {
                if text.is_null() {
                    return;
                }
                if let Some(window) = window_from_view(this) {
                    window.handle_text_input(text);
                }
            }
        }

        unsafe extern "C" fn delete_backward(this: *mut AnyObject, _sel: Sel) {
            unsafe {
                if let Some(window) = window_from_view(this) {
                    window.handle_delete_backward();
                }
            }
        }

        unsafe extern "C" fn can_become_first_responder(_this: *mut AnyObject, _sel: Sel) -> Bool {
            Bool::YES
        }

        // ---- UITextInputTraits ----
        macro_rules! isize_prop {
            ($get:ident, $set:ident, $ivar:literal) => {
                #[allow(deprecated)]
                unsafe extern "C" fn $get(this: *mut AnyObject, _sel: Sel) -> isize {
                    unsafe { *(*this).get_ivar::<isize>($ivar) }
                }
                #[allow(deprecated)]
                unsafe extern "C" fn $set(this: *mut AnyObject, _sel: Sel, val: isize) {
                    unsafe {
                        *(*this).get_mut_ivar::<isize>($ivar) = val;
                    }
                }
            };
        }
        isize_prop!(get_keyboard_type, set_keyboard_type, "_keyboardType");
        isize_prop!(
            get_autocorrection_type,
            set_autocorrection_type,
            "_autocorrectionType"
        );
        isize_prop!(
            get_autocapitalization_type,
            set_autocapitalization_type,
            "_autocapitalizationType"
        );
        isize_prop!(
            get_spell_checking_type,
            set_spell_checking_type,
            "_spellCheckingType"
        );
        isize_prop!(
            get_smart_quotes_type,
            set_smart_quotes_type,
            "_smartQuotesType"
        );
        isize_prop!(
            get_smart_dashes_type,
            set_smart_dashes_type,
            "_smartDashesType"
        );
        isize_prop!(
            get_smart_insert_delete_type,
            set_smart_insert_delete_type,
            "_smartInsertDeleteType"
        );
        isize_prop!(get_return_key_type, set_return_key_type, "_returnKeyType");

        unsafe extern "C" fn keyboard_appearance(_this: *mut AnyObject, _sel: Sel) -> isize {
            0 // UIKeyboardAppearanceDefault
        }
        unsafe extern "C" fn set_keyboard_appearance(
            _this: *mut AnyObject,
            _sel: Sel,
            _val: isize,
        ) {
        }
        unsafe extern "C" fn is_secure_text_entry(_this: *mut AnyObject, _sel: Sel) -> Bool {
            Bool::NO
        }
        unsafe extern "C" fn set_secure_text_entry(_this: *mut AnyObject, _sel: Sel, _v: Bool) {}

        // ---- UITextInput: document extents ----

        unsafe extern "C" fn beginning_of_document(
            _this: *mut AnyObject,
            _sel: Sel,
        ) -> *mut AnyObject {
            make_position(0)
        }
        unsafe extern "C" fn end_of_document(this: *mut AnyObject, _sel: Sel) -> *mut AnyObject {
            unsafe {
                let len = window_from_view(this).map(utf16_len).unwrap_or(0);
                make_position(len as isize)
            }
        }

        unsafe extern "C" fn text_in_range(
            this: *mut AnyObject,
            _sel: Sel,
            range: *mut AnyObject,
        ) -> *mut AnyObject {
            unsafe {
                let Some(window) = window_from_view(this) else {
                    return std::ptr::null_mut();
                };
                let Some((start, end)) = range_offsets(range) else {
                    return std::ptr::null_mut();
                };
                let len = utf16_len(window) as isize;
                let r = clamp_range(start, end, len);
                let mut adjusted = None;
                let text = window
                    .with_input_handler(|h| h.text_for_range(r, &mut adjusted))
                    .flatten();
                match text {
                    Some(s) => {
                        let cstr = std::ffi::CString::new(s).unwrap_or_default();
                        msg_send![class!(NSString), stringWithUTF8String: cstr.as_ptr()]
                    }
                    None => msg_send![class!(NSString), stringWithUTF8String: c"".as_ptr()],
                }
            }
        }

        unsafe extern "C" fn replace_range_with_text(
            this: *mut AnyObject,
            _sel: Sel,
            range: *mut AnyObject,
            text: *mut AnyObject,
        ) {
            unsafe {
                let Some(window) = window_from_view(this) else {
                    return;
                };
                let len = utf16_len(window) as isize;
                let r = range_offsets(range).map(|(s, e)| clamp_range(s, e, len));
                let s = nsstring_to_string(text);
                window.with_input_handler(|h| h.replace_text_in_range(r, &s));
            }
        }

        // ---- selection / marked text ----

        unsafe extern "C" fn selected_text_range(
            this: *mut AnyObject,
            _sel: Sel,
        ) -> *mut AnyObject {
            unsafe {
                let Some(window) = window_from_view(this) else {
                    return make_range(0, 0);
                };
                let sel = window
                    .with_input_handler(|h| h.selected_text_range(true))
                    .flatten();
                match sel {
                    Some(s) => make_range(s.range.start as isize, s.range.end as isize),
                    None => make_range(0, 0),
                }
            }
        }

        unsafe extern "C" fn set_selected_text_range(
            this: *mut AnyObject,
            _sel: Sel,
            range: *mut AnyObject,
        ) {
            unsafe {
                let Some(window) = window_from_view(this) else {
                    return;
                };
                let len = utf16_len(window) as isize;
                if let Some((s, e)) = range_offsets(range) {
                    let r = clamp_range(s, e, len);
                    window.with_input_handler(|h| h.set_selected_text_range(r));
                }
            }
        }

        unsafe extern "C" fn marked_text_range(this: *mut AnyObject, _sel: Sel) -> *mut AnyObject {
            unsafe {
                let Some(window) = window_from_view(this) else {
                    return std::ptr::null_mut();
                };
                let marked = window
                    .with_input_handler(|h| h.marked_text_range())
                    .flatten();
                match marked {
                    Some(r) => make_range(r.start as isize, r.end as isize),
                    None => std::ptr::null_mut(),
                }
            }
        }

        unsafe extern "C" fn marked_text_style(_this: *mut AnyObject, _sel: Sel) -> *mut AnyObject {
            std::ptr::null_mut()
        }
        unsafe extern "C" fn set_marked_text_style(
            _this: *mut AnyObject,
            _sel: Sel,
            _style: *mut AnyObject,
        ) {
        }

        unsafe extern "C" fn set_marked_text(
            this: *mut AnyObject,
            _sel: Sel,
            text: *mut AnyObject,
            selected_range: ObjcNSRange,
        ) {
            unsafe {
                let Some(window) = window_from_view(this) else {
                    return;
                };
                let s = nsstring_to_string(text);
                // NSNotFound is NSIntegerMax, not usize::MAX.
                let new_sel = if selected_range.location >= isize::MAX as usize {
                    None
                } else {
                    Some(selected_range.location..(selected_range.location + selected_range.length))
                };
                window.with_input_handler(|h| h.replace_and_mark_text_in_range(None, &s, new_sel));
            }
        }

        unsafe extern "C" fn unmark_text(this: *mut AnyObject, _sel: Sel) {
            unsafe {
                if let Some(window) = window_from_view(this) {
                    window.with_input_handler(|h| h.unmark_text());
                }
            }
        }

        // ---- position/range arithmetic ----

        unsafe extern "C" fn text_range_from_to(
            _this: *mut AnyObject,
            _sel: Sel,
            from: *mut AnyObject,
            to: *mut AnyObject,
        ) -> *mut AnyObject {
            unsafe {
                match (position_offset(from), position_offset(to)) {
                    (Some(a), Some(b)) => make_range(a, b),
                    _ => std::ptr::null_mut(),
                }
            }
        }

        unsafe extern "C" fn position_from_offset(
            this: *mut AnyObject,
            _sel: Sel,
            pos: *mut AnyObject,
            offset: isize,
        ) -> *mut AnyObject {
            unsafe {
                let Some(base) = position_offset(pos) else {
                    return std::ptr::null_mut();
                };
                let len = window_from_view(this).map(utf16_len).unwrap_or(0) as isize;
                let new_offset = base + offset;
                if new_offset < 0 || new_offset > len {
                    std::ptr::null_mut()
                } else {
                    make_position(new_offset)
                }
            }
        }

        unsafe extern "C" fn position_from_in_direction_offset(
            this: *mut AnyObject,
            _sel: Sel,
            pos: *mut AnyObject,
            direction: isize,
            offset: isize,
        ) -> *mut AnyObject {
            unsafe {
                let Some(base) = position_offset(pos) else {
                    return std::ptr::null_mut();
                };
                let len = window_from_view(this).map(utf16_len).unwrap_or(0) as isize;
                // UITextLayoutDirection: 0=right,1=left,2=up,3=down.
                // We have no line-layout information here, so up/down are
                // treated as no-ops (return the same position); left/right
                // walk by `offset` code units.
                let delta = match direction {
                    0 => offset,
                    1 => -offset,
                    _ => 0,
                };
                let new_offset = (base + delta).clamp(0, len);
                make_position(new_offset)
            }
        }

        unsafe extern "C" fn compare_position(
            _this: *mut AnyObject,
            _sel: Sel,
            a: *mut AnyObject,
            b: *mut AnyObject,
        ) -> isize {
            unsafe {
                match (position_offset(a), position_offset(b)) {
                    (Some(a), Some(b)) => match a.cmp(&b) {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    },
                    _ => 0,
                }
            }
        }

        unsafe extern "C" fn offset_from_to(
            _this: *mut AnyObject,
            _sel: Sel,
            from: *mut AnyObject,
            to: *mut AnyObject,
        ) -> isize {
            unsafe {
                match (position_offset(from), position_offset(to)) {
                    (Some(a), Some(b)) => b - a,
                    _ => 0,
                }
            }
        }

        unsafe extern "C" fn position_within_range_farthest(
            _this: *mut AnyObject,
            _sel: Sel,
            range: *mut AnyObject,
            direction: isize,
        ) -> *mut AnyObject {
            unsafe {
                let Some((s, e)) = range_offsets(range) else {
                    return std::ptr::null_mut();
                };
                // direction: 1=left/up -> farthest is start; 0=right/down (or
                // anything else) -> farthest is end.
                if direction == 1 {
                    make_position(s)
                } else {
                    make_position(e)
                }
            }
        }

        unsafe extern "C" fn character_range_by_extending(
            _this: *mut AnyObject,
            _sel: Sel,
            pos: *mut AnyObject,
            _direction: isize,
        ) -> *mut AnyObject {
            unsafe {
                match position_offset(pos) {
                    Some(o) => make_range(o, o),
                    None => std::ptr::null_mut(),
                }
            }
        }

        unsafe extern "C" fn base_writing_direction(
            _this: *mut AnyObject,
            _sel: Sel,
            _pos: *mut AnyObject,
            _direction: isize,
        ) -> isize {
            0 // NSWritingDirectionLeftToRight
        }
        unsafe extern "C" fn set_base_writing_direction(
            _this: *mut AnyObject,
            _sel: Sel,
            _direction: isize,
            _range: *mut AnyObject,
        ) {
        }

        // ---- geometry ----

        unsafe extern "C" fn first_rect_for_range(
            this: *mut AnyObject,
            _sel: Sel,
            range: *mut AnyObject,
        ) -> ObjcCGRect {
            unsafe { rect_for_offsets(this, range_offsets(range)) }
        }

        unsafe extern "C" fn caret_rect_for_position(
            this: *mut AnyObject,
            _sel: Sel,
            pos: *mut AnyObject,
        ) -> ObjcCGRect {
            unsafe { rect_for_offsets(this, position_offset(pos).map(|o| (o, o))) }
        }

        unsafe extern "C" fn selection_rects_for_range(
            _this: *mut AnyObject,
            _sel: Sel,
            _range: *mut AnyObject,
        ) -> *mut AnyObject {
            unsafe { msg_send![class!(NSArray), array] }
        }

        unsafe extern "C" fn closest_position_to_point(
            this: *mut AnyObject,
            _sel: Sel,
            point: ObjcCGPoint,
        ) -> *mut AnyObject {
            unsafe { position_for_point(this, point) }
        }

        unsafe extern "C" fn closest_position_to_point_within_range(
            this: *mut AnyObject,
            _sel: Sel,
            point: ObjcCGPoint,
            range: *mut AnyObject,
        ) -> *mut AnyObject {
            unsafe {
                let pos = position_for_point(this, point);
                let Some(o) = position_offset(pos) else {
                    return pos;
                };
                if let Some((s, e)) = range_offsets(range) {
                    let clamped = o.clamp(s.min(e), s.max(e));
                    make_position(clamped)
                } else {
                    pos
                }
            }
        }

        unsafe extern "C" fn character_range_at_point(
            this: *mut AnyObject,
            _sel: Sel,
            point: ObjcCGPoint,
        ) -> *mut AnyObject {
            unsafe {
                let pos = position_for_point(this, point);
                match position_offset(pos) {
                    Some(o) => make_range(o, o),
                    None => std::ptr::null_mut(),
                }
            }
        }

        // ---- delegate / tokenizer ----

        unsafe extern "C" fn input_delegate(this: *mut AnyObject, _sel: Sel) -> *mut AnyObject {
            #[allow(deprecated)]
            unsafe {
                *(*this).get_ivar::<*mut c_void>("_inputDelegate") as *mut AnyObject
            }
        }
        unsafe extern "C" fn set_input_delegate(
            this: *mut AnyObject,
            _sel: Sel,
            delegate: *mut AnyObject,
        ) {
            unsafe {
                #[allow(deprecated)]
                let old: *mut c_void = *(*this).get_ivar("_inputDelegate");
                if !old.is_null() {
                    let _: () = msg_send![old as *mut AnyObject, release];
                }
                let retained: *mut AnyObject = msg_send![delegate, retain];
                #[allow(deprecated)]
                {
                    *(*this).get_mut_ivar::<*mut c_void>("_inputDelegate") =
                        retained as *mut c_void;
                }
            }
        }

        // ---- hardware keyboard (UIPress) ----

        fn is_printable_key(code: u32) -> bool {
            // Letters, digits, punctuation, space — excludes enter (0x28),
            // escape (0x29), backspace (0x2A) and tab (0x2B), which UIKit
            // never turns into `insertText:` on its own.
            matches!(code, 0x04..=0x27 | 0x2C..=0x38)
        }

        unsafe fn handle_presses(
            this: *mut AnyObject,
            presses: *mut AnyObject,
            is_down: bool,
        ) -> bool {
            unsafe {
                let all: *mut AnyObject = msg_send![presses, allObjects];
                let count: usize = msg_send![all, count];
                let mut pass_to_super = false;
                for i in 0..count {
                    let press: *mut AnyObject = msg_send![all, objectAtIndex: i];
                    let key: *mut AnyObject = msg_send![press, key];
                    if key.is_null() {
                        continue;
                    }
                    let key_code: u32 = msg_send![key, keyCode];
                    let modifier_flags: usize = msg_send![key, modifierFlags];
                    let modifiers =
                        super::text_input::modifier_flags_to_modifiers(modifier_flags as u32);
                    if is_printable_key(key_code)
                        && !modifiers.control
                        && !modifiers.platform
                        && !modifiers.alt
                    {
                        pass_to_super = true;
                    } else if let Some(window) = window_from_view(this) {
                        window.handle_key_event(key_code, modifier_flags as u32, is_down);
                    }
                }
                pass_to_super
            }
        }

        unsafe extern "C" fn presses_began(
            this: *mut AnyObject,
            _sel: Sel,
            presses: *mut AnyObject,
            event: *mut AnyObject,
        ) {
            unsafe {
                if handle_presses(this, presses, true) {
                    let superclass = class!(UIView);
                    let _: () =
                        msg_send![super(this, superclass), pressesBegan: presses, withEvent: event];
                }
            }
        }

        unsafe extern "C" fn presses_ended(
            this: *mut AnyObject,
            _sel: Sel,
            presses: *mut AnyObject,
            event: *mut AnyObject,
        ) {
            unsafe {
                if handle_presses(this, presses, false) {
                    let superclass = class!(UIView);
                    let _: () =
                        msg_send![super(this, superclass), pressesEnded: presses, withEvent: event];
                }
            }
        }

        unsafe extern "C" fn tokenizer(this: *mut AnyObject, _sel: Sel) -> *mut AnyObject {
            unsafe {
                #[allow(deprecated)]
                let existing: *mut c_void = *(*this).get_ivar("_tokenizer");
                if !existing.is_null() {
                    return existing as *mut AnyObject;
                }
                let tok_cls = class!(UITextInputStringTokenizer);
                let tok: *mut AnyObject = msg_send![tok_cls, alloc];
                let tok: *mut AnyObject = msg_send![tok, initWithTextInput: this];
                #[allow(deprecated)]
                {
                    *(*this).get_mut_ivar::<*mut c_void>("_tokenizer") = tok as *mut c_void;
                }
                tok
            }
        }

        unsafe {
            decl.add_method(
                sel!(hasText),
                has_text as unsafe extern "C" fn(*mut AnyObject, Sel) -> Bool,
            );
            decl.add_method(
                sel!(insertText:),
                insert_text as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
            decl.add_method(
                sel!(deleteBackward),
                delete_backward as unsafe extern "C" fn(*mut AnyObject, Sel),
            );
            decl.add_method(
                sel!(canBecomeFirstResponder),
                can_become_first_responder as unsafe extern "C" fn(*mut AnyObject, Sel) -> Bool,
            );

            decl.add_method(
                sel!(keyboardType),
                get_keyboard_type as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setKeyboardType:),
                set_keyboard_type as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(autocorrectionType),
                get_autocorrection_type as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setAutocorrectionType:),
                set_autocorrection_type as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(autocapitalizationType),
                get_autocapitalization_type as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setAutocapitalizationType:),
                set_autocapitalization_type as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(spellCheckingType),
                get_spell_checking_type as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setSpellCheckingType:),
                set_spell_checking_type as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(smartQuotesType),
                get_smart_quotes_type as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setSmartQuotesType:),
                set_smart_quotes_type as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(smartDashesType),
                get_smart_dashes_type as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setSmartDashesType:),
                set_smart_dashes_type as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(smartInsertDeleteType),
                get_smart_insert_delete_type as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setSmartInsertDeleteType:),
                set_smart_insert_delete_type as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(returnKeyType),
                get_return_key_type as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setReturnKeyType:),
                set_return_key_type as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(keyboardAppearance),
                keyboard_appearance as unsafe extern "C" fn(*mut AnyObject, Sel) -> isize,
            );
            decl.add_method(
                sel!(setKeyboardAppearance:),
                set_keyboard_appearance as unsafe extern "C" fn(*mut AnyObject, Sel, isize),
            );
            decl.add_method(
                sel!(isSecureTextEntry),
                is_secure_text_entry as unsafe extern "C" fn(*mut AnyObject, Sel) -> Bool,
            );
            decl.add_method(
                sel!(setSecureTextEntry:),
                set_secure_text_entry as unsafe extern "C" fn(*mut AnyObject, Sel, Bool),
            );

            decl.add_method(
                sel!(beginningOfDocument),
                beginning_of_document
                    as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(endOfDocument),
                end_of_document as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(textInRange:),
                text_in_range
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(replaceRange:withText:),
                replace_range_with_text
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
            decl.add_method(
                sel!(selectedTextRange),
                selected_text_range as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(setSelectedTextRange:),
                set_selected_text_range
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
            decl.add_method(
                sel!(markedTextRange),
                marked_text_range as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(markedTextStyle),
                marked_text_style as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(setMarkedTextStyle:),
                set_marked_text_style as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
            decl.add_method(
                sel!(setMarkedText:selectedRange:),
                set_marked_text
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, ObjcNSRange),
            );
            decl.add_method(
                sel!(unmarkText),
                unmark_text as unsafe extern "C" fn(*mut AnyObject, Sel),
            );
            decl.add_method(
                sel!(textRangeFromPosition:toPosition:),
                text_range_from_to
                    as unsafe extern "C" fn(
                        *mut AnyObject,
                        Sel,
                        *mut AnyObject,
                        *mut AnyObject,
                    ) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(positionFromPosition:offset:),
                position_from_offset
                    as unsafe extern "C" fn(
                        *mut AnyObject,
                        Sel,
                        *mut AnyObject,
                        isize,
                    ) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(positionFromPosition:inDirection:offset:),
                position_from_in_direction_offset
                    as unsafe extern "C" fn(
                        *mut AnyObject,
                        Sel,
                        *mut AnyObject,
                        isize,
                        isize,
                    ) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(comparePosition:toPosition:),
                compare_position
                    as unsafe extern "C" fn(
                        *mut AnyObject,
                        Sel,
                        *mut AnyObject,
                        *mut AnyObject,
                    ) -> isize,
            );
            decl.add_method(
                sel!(offsetFromPosition:toPosition:),
                offset_from_to
                    as unsafe extern "C" fn(
                        *mut AnyObject,
                        Sel,
                        *mut AnyObject,
                        *mut AnyObject,
                    ) -> isize,
            );
            decl.add_method(
                sel!(positionWithinRange:farthestInDirection:),
                position_within_range_farthest
                    as unsafe extern "C" fn(
                        *mut AnyObject,
                        Sel,
                        *mut AnyObject,
                        isize,
                    ) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(characterRangeByExtendingPosition:inDirection:),
                character_range_by_extending
                    as unsafe extern "C" fn(
                        *mut AnyObject,
                        Sel,
                        *mut AnyObject,
                        isize,
                    ) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(baseWritingDirectionForPosition:inDirection:),
                base_writing_direction
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, isize) -> isize,
            );
            decl.add_method(
                sel!(setBaseWritingDirection:forRange:),
                set_base_writing_direction
                    as unsafe extern "C" fn(*mut AnyObject, Sel, isize, *mut AnyObject),
            );
            decl.add_method(
                sel!(firstRectForRange:),
                first_rect_for_range
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject) -> ObjcCGRect,
            );
            decl.add_method(
                sel!(caretRectForPosition:),
                caret_rect_for_position
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject) -> ObjcCGRect,
            );
            decl.add_method(
                sel!(selectionRectsForRange:),
                selection_rects_for_range
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(closestPositionToPoint:),
                closest_position_to_point
                    as unsafe extern "C" fn(*mut AnyObject, Sel, ObjcCGPoint) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(closestPositionToPoint:withinRange:),
                closest_position_to_point_within_range
                    as unsafe extern "C" fn(
                        *mut AnyObject,
                        Sel,
                        ObjcCGPoint,
                        *mut AnyObject,
                    ) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(characterRangeAtPoint:),
                character_range_at_point
                    as unsafe extern "C" fn(*mut AnyObject, Sel, ObjcCGPoint) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(inputDelegate),
                input_delegate as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(setInputDelegate:),
                set_input_delegate as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
            decl.add_method(
                sel!(tokenizer),
                tokenizer as unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
            decl.add_method(
                sel!(pressesBegan:withEvent:),
                presses_began
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
            decl.add_method(
                sel!(pressesEnded:withEvent:),
                presses_ended
                    as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
        }

        decl.register();
    });
    class!(GPUITextInputView)
}

/// `NSRange`-shaped struct for the objc2 calling convention. Distinct from
/// any `NSRange` re-export so we control its `Encode` impl locally.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ObjcNSRange {
    pub location: usize,
    pub length: usize,
}

unsafe impl objc2::encode::Encode for ObjcNSRange {
    const ENCODING: objc2::encode::Encoding = objc2::encode::Encoding::Struct(
        "_NSRange",
        &[
            <usize as objc2::encode::Encode>::ENCODING,
            <usize as objc2::encode::Encode>::ENCODING,
        ],
    );
}
unsafe impl objc2::encode::RefEncode for ObjcNSRange {
    const ENCODING_REF: objc2::encode::Encoding =
        objc2::encode::Encoding::Pointer(&<Self as objc2::encode::Encode>::ENCODING);
}

unsafe fn nsstring_to_string(s: *mut AnyObject) -> String {
    unsafe {
        if s.is_null() {
            return String::new();
        }
        let utf8: *const i8 = msg_send![s, UTF8String];
        if utf8.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .into_owned()
    }
}

/// Computes the `CGRect` (in this view's own coordinate space) for the
/// document range spanning `offsets` (UTF-16), by asking the input handler
/// for window-space bounds and converting from the metal view's coordinate
/// space (the same space GPUI's `Bounds<Pixels>` are reported in) into this
/// (possibly repositioned) view's local space.
unsafe fn rect_for_offsets(this: *mut AnyObject, offsets: Option<(isize, isize)>) -> ObjcCGRect {
    unsafe {
        let Some(window) = window_from_view(this) else {
            return ObjcCGRect::new(0.0, 0.0, 0.0, 0.0);
        };
        let Some((s, e)) = offsets else {
            return ObjcCGRect::new(0.0, 0.0, 0.0, 0.0);
        };
        let len = utf16_len(window) as isize;
        let range = clamp_range(s, e, len);
        let bounds = window
            .with_input_handler(|h| h.bounds_for_range(range))
            .flatten();
        let Some(bounds) = bounds else {
            return ObjcCGRect::new(0.0, 0.0, 0.0, 0.0);
        };
        let rect_in_metal_view = ObjcCGRect::new(
            f64::from(bounds.origin.x),
            f64::from(bounds.origin.y),
            f64::from(bounds.size.width).max(1.0),
            f64::from(bounds.size.height),
        );
        let superview: *mut AnyObject = msg_send![this, superview];
        if superview.is_null() {
            rect_in_metal_view
        } else {
            msg_send![this, convertRect: rect_in_metal_view, fromView: superview]
        }
    }
}

unsafe fn position_for_point(this: *mut AnyObject, point: ObjcCGPoint) -> *mut AnyObject {
    unsafe {
        let Some(window) = window_from_view(this) else {
            return make_position(0);
        };
        let superview: *mut AnyObject = msg_send![this, superview];
        let point_in_metal_view: ObjcCGPoint = if superview.is_null() {
            point
        } else {
            msg_send![this, convertPoint: point, toView: superview]
        };
        let gpui_point = gpui::point(
            gpui::px(point_in_metal_view.x as f32),
            gpui::px(point_in_metal_view.y as f32),
        );
        let idx = window
            .with_input_handler(|h| h.character_index_for_point(gpui_point))
            .flatten();
        make_position(idx.unwrap_or(0) as isize)
    }
}
