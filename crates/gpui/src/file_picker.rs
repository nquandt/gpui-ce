//! A platform-neutral way to let the user choose files.
//!
//! Native file dialogs return paths, and the browser has no paths at all. So
//! this service returns the file *contents*: a [`PickedFile`] carries the name,
//! the MIME type when the host knows it, the path when one exists, and the
//! bytes. App code that works with bytes then runs unchanged on desktop,
//! web and mobile.
//!
//! Picking is a UI operation and always happens on the main thread, so the
//! future is not `Send`. Await it from `cx.spawn`, which runs on the
//! foreground executor:
//!
//! ```ignore
//! let picker = cx.file_picker();
//! cx.spawn(async move |this, cx| {
//!     let files = picker.pick_files(FilePickerOptions::default()).await?;
//!     // ...
//! })
//! ```

use crate::{PathPromptOptions, Platform, SharedString};
use anyhow::Result;
use futures::future::LocalBoxFuture;
use std::path::PathBuf;
use std::rc::Rc;

/// One file chosen by the user, with its contents loaded.
#[derive(Debug, Clone)]
pub struct PickedFile {
    /// The file name without directory, as the host reported it.
    pub name: String,
    /// The MIME type, when the host knows it (browsers do; desktops may not).
    pub mime_type: Option<String>,
    /// The location on disk, when the host has one. Never set in the browser.
    pub path: Option<PathBuf>,
    /// The full contents.
    pub bytes: Vec<u8>,
}

/// What to ask the user for.
#[derive(Debug, Clone, Default)]
pub struct FilePickerOptions {
    /// Allow more than one file.
    pub multiple: bool,
    /// Accepted types as file extensions with a leading dot (`".png"`) or MIME
    /// types (`"image/*"`). Empty means any file. Hosts that cannot filter
    /// ignore this.
    pub accept: Vec<SharedString>,
    /// Title or prompt for the dialog, where the host shows one.
    pub prompt: Option<SharedString>,
}

impl FilePickerOptions {
    /// Allow multiple files.
    pub fn multiple(mut self) -> Self {
        self.multiple = true;
        self
    }

    /// Add an accepted extension or MIME type.
    pub fn accept(mut self, kind: impl Into<SharedString>) -> Self {
        self.accept.push(kind.into());
        self
    }

    /// Set the dialog prompt.
    pub fn prompt(mut self, prompt: impl Into<SharedString>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }
}

/// Lets the user choose files and loads their contents.
///
/// An empty result means the user cancelled. Errors are for hosts that cannot
/// pick at all or that failed to read a chosen file.
pub trait FilePicker: 'static {
    /// Show the host's picker and load the chosen files.
    fn pick_files(
        &self,
        options: FilePickerOptions,
    ) -> LocalBoxFuture<'static, Result<Vec<PickedFile>>>;
}

/// The default picker: the platform's native path dialog followed by a read
/// from disk. Works on desktop and iOS. The web backend installs its own
/// picker because the browser has no path dialog.
pub struct PlatformFilePicker {
    platform: Rc<dyn Platform>,
}

impl PlatformFilePicker {
    /// Create a picker over the given platform.
    pub fn new(platform: Rc<dyn Platform>) -> Self {
        Self { platform }
    }
}

impl FilePicker for PlatformFilePicker {
    fn pick_files(
        &self,
        options: FilePickerOptions,
    ) -> LocalBoxFuture<'static, Result<Vec<PickedFile>>> {
        let receiver = self.platform.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: options.multiple,
            prompt: options.prompt,
        });
        Box::pin(async move {
            let paths = receiver
                .await
                .map_err(|_| anyhow::anyhow!("file dialog was dropped"))??
                .unwrap_or_default();
            let mut files = Vec::with_capacity(paths.len());
            for path in paths {
                files.push(read_picked_file(path)?);
            }
            Ok(files)
        })
    }
}

fn read_picked_file(path: PathBuf) -> Result<PickedFile> {
    #[cfg(target_family = "wasm")]
    {
        anyhow::bail!(
            "cannot read {} on the web; install a web file picker",
            path.display()
        )
    }
    #[cfg(not(target_family = "wasm"))]
    {
        let bytes = std::fs::read(&path)
            .map_err(|error| anyhow::anyhow!("reading {}: {error}", path.display()))?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(PickedFile {
            name,
            mime_type: None,
            path: Some(path),
            bytes,
        })
    }
}

/// A picker that returns a fixed list of files. For tests and for hosts that
/// supply files from elsewhere.
pub struct StaticFilePicker {
    files: Vec<PickedFile>,
}

impl StaticFilePicker {
    /// Create a picker that always returns `files`.
    pub fn new(files: Vec<PickedFile>) -> Self {
        Self { files }
    }
}

impl FilePicker for StaticFilePicker {
    fn pick_files(
        &self,
        options: FilePickerOptions,
    ) -> LocalBoxFuture<'static, Result<Vec<PickedFile>>> {
        let mut files = self.files.clone();
        if !options.multiple {
            files.truncate(1);
        }
        Box::pin(async move { Ok(files) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn static_picker_respects_multiple() {
        let file = |name: &str| PickedFile {
            name: name.into(),
            mime_type: None,
            path: None,
            bytes: vec![1],
        };
        let picker = StaticFilePicker::new(vec![file("a"), file("b")]);
        assert_eq!(
            block_on(picker.pick_files(FilePickerOptions::default()))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            block_on(picker.pick_files(FilePickerOptions::default().multiple()))
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn options_builder() {
        let options = FilePickerOptions::default()
            .multiple()
            .accept(".png")
            .accept("image/*")
            .prompt("Choose");
        assert!(options.multiple);
        assert_eq!(options.accept.len(), 2);
        assert_eq!(options.prompt.as_deref(), Some("Choose"));
    }
}
