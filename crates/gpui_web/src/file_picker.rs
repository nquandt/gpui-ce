//! `<input type="file">`-backed [`FilePicker`] for the browser.
//!
//! A hidden input is appended to the document, clicked, and removed once the
//! user has chosen or cancelled. Browsers only open the dialog inside a user
//! activation window, so call `pick_files` from a click handler or shortly
//! after one. Each chosen `File` is read with `arrayBuffer()`.

use anyhow::{Result, anyhow};
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use gpui::file_picker::{FilePicker, FilePickerOptions, PickedFile};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;

/// File picker built on a hidden file input.
#[derive(Default)]
pub struct DomFilePicker;

impl FilePicker for DomFilePicker {
    fn pick_files(
        &self,
        options: FilePickerOptions,
    ) -> LocalBoxFuture<'static, Result<Vec<PickedFile>>> {
        Box::pin(async move { pick(options).await })
    }
}

async fn pick(options: FilePickerOptions) -> Result<Vec<PickedFile>> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| anyhow!("file picker needs the main thread"))?;
    let body = document
        .body()
        .ok_or_else(|| anyhow!("document has no body"))?;

    let input: web_sys::HtmlInputElement = document
        .create_element("input")
        .map_err(|error| anyhow!("failed to create input: {error:?}"))?
        .dyn_into()
        .map_err(|_| anyhow!("element is not an input"))?;
    input.set_type("file");
    input.set_multiple(options.multiple);
    if !options.accept.is_empty() {
        let accept = options
            .accept
            .iter()
            .map(|kind| kind.as_ref())
            .collect::<Vec<&str>>()
            .join(",");
        input.set_accept(&accept);
    }
    let _ = input.style().set_property("display", "none");
    body.append_child(&input)
        .map_err(|error| anyhow!("failed to attach input: {error:?}"))?;

    // Resolve on `change` (files chosen) or `cancel` (dialog dismissed).
    let (sender, receiver) = oneshot::channel::<()>();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let make_listener = || {
        let sender = sender.clone();
        Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
            if let Some(sender) = sender.borrow_mut().take() {
                let _ = sender.send(());
            }
        })
    };
    let on_change = make_listener();
    let on_cancel = make_listener();
    let _ = input.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
    let _ = input.add_event_listener_with_callback("cancel", on_cancel.as_ref().unchecked_ref());

    input.click();
    let result = receiver.await;

    let _ = input.remove_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
    let _ = input.remove_event_listener_with_callback("cancel", on_cancel.as_ref().unchecked_ref());
    input.remove();
    result.map_err(|_| anyhow!("file picker was dropped"))?;

    let Some(list) = input.files() else {
        return Ok(Vec::new());
    };
    let mut files = Vec::with_capacity(list.length() as usize);
    for index in 0..list.length() {
        let Some(file) = list.get(index) else {
            continue;
        };
        let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
            .await
            .map_err(|error| anyhow!("reading {}: {error:?}", file.name()))?;
        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
        let mime_type = file.type_();
        files.push(PickedFile {
            name: file.name(),
            mime_type: (!mime_type.is_empty()).then_some(mime_type),
            path: None,
            bytes,
        });
    }
    Ok(files)
}
