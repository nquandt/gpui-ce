use gpui::SharedString;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// A `window.invoke(cmd, payload)` call made by page JS, decoded from the
/// wire envelope `{"id", "cmd", "payload"}` the injected bridge script sends.
///
/// Obtained via [`crate::WebView::on_ipc_message`]. Reply with
/// [`crate::WebViewHandle::reply`] (or [`crate::WebViewHandle::reply_ok`] /
/// [`crate::WebViewHandle::reply_err`]) using the same `id` to settle the
/// promise `invoke()` returned on the page.
#[derive(Debug, Clone, Deserialize)]
pub struct IpcRequest {
    pub id: u64,
    pub cmd: SharedString,
    #[serde(default)]
    pub payload: Value,
}

impl IpcRequest {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        serde_json::from_str(raw).ok()
    }

    /// Deserialize `payload` into a typed command argument struct, the way a
    /// Tauri command's parameters are decoded from its invoke payload.
    pub fn payload<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}

/// The outcome of handling one [`IpcRequest`], to be sent back with
/// [`crate::WebViewHandle::reply`].
pub type IpcResult = Result<Value, String>;

/// Build the `window.__gpuiIpcResolve(id, ok, value)` call that settles the
/// promise a page's `invoke()` call is waiting on. `id` and `ok` are emitted
/// as literals (safe: a `u64` and a `bool`); `value` always goes through
/// `serde_json::to_string`, so nothing here embeds unescaped user data into
/// the script text.
pub(crate) fn resolve_script(id: u64, result: &IpcResult) -> String {
    let (ok, value): (bool, Value) = match result {
        Ok(value) => (true, value.clone()),
        Err(message) => (false, Value::String(message.clone())),
    };
    let encoded = serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string());
    format!("window.__gpuiIpcResolve({id}, {ok}, {encoded});")
}

/// Convenience for building an `Ok` [`IpcResult`] from any serializable value.
pub fn ok<T: Serialize>(value: T) -> IpcResult {
    serde_json::to_value(value).map_err(|err| err.to_string())
}
