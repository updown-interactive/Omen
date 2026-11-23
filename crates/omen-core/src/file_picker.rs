//! Native path selection requests, independent of the rendering backend.
use std::{fmt, path::PathBuf, rc::Rc};

/// Result of a native path prompt. `Ok(None)` means the user cancelled.
pub type FilePickerResult = Result<Option<Vec<PathBuf>>, String>;

/// A native file/folder prompt attached to an activatable view.
/// Supported by the GPUI desktop backend. The callback runs on the UI thread.
#[derive(Clone)]
pub struct FilePicker {
    /// Allow selecting regular files.
    pub files: bool,
    /// Allow selecting directories.
    pub directories: bool,
    /// Allow more than one selection.
    pub multiple: bool,
    /// Optional label for the native confirmation button.
    pub prompt: Option<String>,
    callback: Rc<dyn Fn(FilePickerResult)>,
}
impl FilePicker {
    /// Configure a single-folder chooser.
    pub fn folders(callback: impl Fn(FilePickerResult) + 'static) -> Self {
        Self {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
            callback: Rc::new(callback),
        }
    }
    /// Configure a single-file chooser.
    pub fn files(callback: impl Fn(FilePickerResult) + 'static) -> Self {
        Self {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
            callback: Rc::new(callback),
        }
    }
    /// Deliver a selection, cancellation, or error to the application.
    pub fn complete(&self, result: FilePickerResult) {
        (self.callback)(result);
    }
}
impl PartialEq for FilePicker {
    fn eq(&self, other: &Self) -> bool {
        self.files == other.files
            && self.directories == other.directories
            && self.multiple == other.multiple
            && self.prompt == other.prompt
            && Rc::ptr_eq(&self.callback, &other.callback)
    }
}
impl fmt::Debug for FilePicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePicker")
            .field("files", &self.files)
            .field("directories", &self.directories)
            .field("multiple", &self.multiple)
            .field("prompt", &self.prompt)
            .finish_non_exhaustive()
    }
}
