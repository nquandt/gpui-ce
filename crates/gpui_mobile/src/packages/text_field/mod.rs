//! A native single-line text field for input that needs full native text
//! editing (cursor placement, drag/double-tap selection, copy/paste,
//! autocorrect).
//!
//! GPUI-drawn text bars can't provide any of that — they're just styled
//! `div`s — so this embeds a real `UITextField` (iOS) via the platform
//! view system instead.
//!
//! Feature-gated behind `text_field`.

use crate::platform_view::{
    PlatformViewBounds, PlatformViewHandle, PlatformViewParams, PlatformViewRegistry,
};
use std::sync::Arc;

/// Register the "text_field" platform view factory.
fn ensure_factory_registered() {
    let registry = PlatformViewRegistry::global();
    if !registry.has_factory("text_field") {
        #[cfg(target_os = "ios")]
        {
            use crate::ios::platform_view::IosPlatformViewFactory;
            registry.register(
                "text_field",
                Box::new(IosPlatformViewFactory::new("text_field")),
            );
        }
    }
}

/// Create a native text field with the given initial text and placeholder.
pub fn create_text_field(text: &str, placeholder: &str) -> Result<TextFieldHandle, String> {
    ensure_factory_registered();

    let mut creation_params = std::collections::HashMap::new();
    creation_params.insert("text".to_string(), text.to_string());
    creation_params.insert("placeholder".to_string(), placeholder.to_string());

    let params = PlatformViewParams {
        bounds: PlatformViewBounds::default(),
        creation_params,
    };

    let handle = PlatformViewRegistry::global().create_view("text_field", params)?;
    Ok(TextFieldHandle {
        platform_handle: Some(Arc::new(handle)),
    })
}

/// Opaque handle to a native text field instance.
#[derive(Debug)]
pub struct TextFieldHandle {
    platform_handle: Option<Arc<PlatformViewHandle>>,
}

impl TextFieldHandle {
    /// Get the platform view handle for embedding in a GPUI element.
    pub fn platform_view_handle(&self) -> Option<Arc<PlatformViewHandle>> {
        self.platform_handle.clone()
    }

    /// Set the field's displayed text.
    pub fn set_text(&self, text: &str) {
        if let Some(handle) = &self.platform_handle {
            set_text_on(handle, text);
        }
    }

    /// Get the field's current text.
    pub fn text(&self) -> Option<String> {
        text_of(self.platform_handle.as_ref()?)
    }

    /// Whether the field currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.platform_handle
            .as_ref()
            .map(|h| is_focused(h))
            .unwrap_or(false)
    }

    /// Dismiss the keyboard if this field has focus.
    pub fn resign_focus(&self) {
        if let Some(handle) = &self.platform_handle {
            resign_focus(handle);
        }
    }
}

#[cfg(target_os = "ios")]
fn with_native<R>(
    handle: &PlatformViewHandle,
    f: impl FnOnce(&crate::ios::platform_view::IosPlatformView) -> R,
) -> Option<R> {
    handle
        .inner()
        .as_any()
        .downcast_ref::<crate::ios::platform_view::IosPlatformView>()
        .map(f)
}

/// Set the `text` property on a "text_field" platform view, given its
/// [`PlatformViewHandle`] directly (rather than the owning [`TextFieldHandle`],
/// which callbacks stored elsewhere may not have access to).
pub fn set_text_on(handle: &PlatformViewHandle, text: &str) {
    #[cfg(target_os = "ios")]
    {
        with_native(handle, |v| v.set_text(text));
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (handle, text);
    }
}

/// Read the `text` property from a "text_field" platform view handle.
pub fn text_of(handle: &PlatformViewHandle) -> Option<String> {
    #[cfg(target_os = "ios")]
    {
        with_native(handle, |v| v.text()).flatten()
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = handle;
        None
    }
}

/// Whether a "text_field" platform view handle currently has keyboard focus.
pub fn is_focused(handle: &PlatformViewHandle) -> bool {
    #[cfg(target_os = "ios")]
    {
        with_native(handle, |v| v.is_first_responder()).unwrap_or(false)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = handle;
        false
    }
}

/// Dismiss the keyboard for a "text_field" platform view handle, if focused.
pub fn resign_focus(handle: &PlatformViewHandle) {
    #[cfg(target_os = "ios")]
    {
        with_native(handle, |v| v.resign_first_responder());
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = handle;
    }
}

impl Drop for TextFieldHandle {
    fn drop(&mut self) {
        if let Some(h) = self.platform_handle.take() {
            h.dispose();
        }
    }
}

type TextChangedCallback = Box<dyn FnMut(&str)>;
type SubmitCallback = Box<dyn FnMut()>;

thread_local! {
    /// Global callback invoked whenever the active text field's text changes.
    static TEXT_CHANGED_CALLBACK: std::cell::RefCell<Option<TextChangedCallback>> =
        std::cell::RefCell::new(None);
    /// Global callback invoked when the user taps the keyboard's return key.
    static SUBMIT_CALLBACK: std::cell::RefCell<Option<SubmitCallback>> =
        std::cell::RefCell::new(None);
}

/// Register a callback for text changes in the active text field.
///
/// Only one callback can be active at a time. Call with `None` to clear it.
pub fn set_on_text_changed(callback: Option<TextChangedCallback>) {
    TEXT_CHANGED_CALLBACK.with(|cb| {
        *cb.borrow_mut() = callback;
    });
}

/// Register a callback for the return-key ("submit") action.
///
/// Only one callback can be active at a time. Call with `None` to clear it.
pub fn set_on_submit(callback: Option<SubmitCallback>) {
    SUBMIT_CALLBACK.with(|cb| {
        *cb.borrow_mut() = callback;
    });
}

/// Dispatch a text change to the registered callback.
///
/// Called internally by the platform layer's control-event target.
pub(crate) fn dispatch_text_changed(text: &str) {
    TEXT_CHANGED_CALLBACK.with(|cb| {
        if let Some(callback) = cb.borrow_mut().as_mut() {
            callback(text);
        }
    });
    crate::TEXT_INPUT_DIRTY.store(true, std::sync::atomic::Ordering::Release);
}

/// Dispatch the submit action to the registered callback.
///
/// Called internally by the platform layer's control-event target.
pub(crate) fn dispatch_submit() {
    SUBMIT_CALLBACK.with(|cb| {
        if let Some(callback) = cb.borrow_mut().as_mut() {
            callback();
        }
    });
    crate::TEXT_INPUT_DIRTY.store(true, std::sync::atomic::Ordering::Release);
}
