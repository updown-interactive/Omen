//! Visual editor commands use the same document and persistent workspace worker.
use super::StudioEnvironment;
use crate::{
    visual::{CanvasSettings, ComponentEdit, EditorMode, PreviewResult, Profile, VisualTree},
    workspace::Command,
};
use omen::{Binding, EditorEvent, EditorState, PreviewGesture, TextSnapshot};
use std::{
    future::Future,
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
struct Revision {
    text: TextSnapshot,
    selection: Range<usize>,
}

impl From<&EditorState> for Revision {
    fn from(state: &EditorState) -> Self {
        Self {
            text: state.text.clone(),
            selection: state.selection.clone(),
        }
    }
}

pub(super) struct PreviewMarker {
    path: PathBuf,
    text: TextSnapshot,
    end: Option<usize>,
}

#[derive(Default)]
pub(super) struct History {
    undo: Vec<Revision>,
    redo: Vec<Revision>,
    current: u64,
}

impl StudioEnvironment {
    pub fn default_frame_profile(&self) -> Profile {
        let preferred = self.settings.read(|settings| settings.design.frame_profile);
        let platforms = self.workspace.read(|workspace| workspace.project_platforms());
        Profile::resolve_default(platforms.as_deref().unwrap_or(&[]), Some(preferred))
    }

    pub fn canvas_settings(&self, path: &Path) -> CanvasSettings {
        if let Some((active, settings)) = self.canvas_motion.get()
            && active == path
        {
            return settings;
        }
        self.workspace
            .read(|workspace| workspace.canvas(path).cloned())
            .unwrap_or_else(|| {
                let profile = self.default_frame_profile();
                self.settings.read(|settings| {
                    CanvasSettings::new(settings.design.initial_mode, profile)
                })
            })
    }

    /// Gesture updates belong to the canvas, so they do not regenerate the
    /// project tree, tabs, language inspector, and status bar for every event.
    pub fn update_canvas_motion(&self, path: &Path, update: impl FnOnce(&mut CanvasSettings)) {
        if !self
            .workspace
            .read(|workspace| workspace.active_file.as_deref() == Some(path))
        {
            return;
        }
        let previous = self.canvas_settings(path);
        let mut settings = previous.clone();
        update(&mut settings);
        settings.sanitize_camera();
        if settings == previous {
            return;
        }
        self.canvas_motion
            .set(Some((path.to_owned(), settings.clone())));
        if let Some(service) = &self.service
            && let Err(message) = service.store_canvas(path.to_owned(), settings)
        {
            self.visual_message.set(message);
        }
    }

    pub fn update_canvas(&self, path: &Path, update: impl FnOnce(&mut CanvasSettings)) {
        let state = self.workspace.get();
        if state.active_file.as_deref() != Some(path) {
            return;
        }
        let mut settings = self.canvas_settings(path);
        update(&mut settings);
        settings.sanitize();
        self.workspace.update(|workspace| {
            if let Some(root) = &workspace.root
                && let Ok(relative) = path.strip_prefix(root)
            {
                workspace
                    .canvases
                    .entry(root.clone())
                    .or_default()
                    .insert(relative.to_owned(), settings.clone());
            }
        });
        self.canvas_motion.set(None);
        self.send(Command::SetCanvas {
            path: path.to_owned(),
            settings,
        });
    }

    pub(crate) fn has_visual_preview(&self, path: &Path, text: &TextSnapshot) -> bool {
        let mut cached = self.preview_marker.borrow_mut();
        let previous = cached.as_ref().filter(|marker| marker.path == path);
        if let Some(marker) = previous.filter(|marker| marker.text.same_snapshot(text)) {
            return marker.end.is_some();
        }
        let source = text.slice(..);
        let end = omen_macro_core::inspector::preview_marker_end(&source).unwrap_or_else(|_| {
            // Unfinished edits below the confirmed marker can keep the current preview.
            let previous = previous?;
            let end = previous.end?;
            let marker_ends = source
                .get(end..)?
                .chars()
                .next()
                .is_none_or(|next| next.is_whitespace() || matches!(next, '/' | '#'));
            (marker_ends && source.get(..end)? == previous.text.slice(..end)).then_some(end)
        });
        *cached = Some(PreviewMarker {
            path: path.to_owned(),
            text: text.clone(),
            end,
        });
        end.is_some()
    }

    pub fn set_visual_mode(&self, mode: EditorMode) {
        if let Some(path) = self.workspace.get().active_file {
            self.set_document_mode(&path, mode);
        }
    }

    pub(crate) fn set_document_mode(&self, path: &Path, mode: EditorMode) {
        if self
            .workspace
            .read(|workspace| workspace.active_file.as_deref() == Some(path))
            && self.has_visual_preview(
                path,
                &self.editor_document(path.to_owned()).state.get().text,
            )
        {
            if mode == EditorMode::Design && self.canvas_settings(path).mode != EditorMode::Design {
                self.editor_blur(path);
            }
            self.update_canvas(path, |canvas| canvas.mode = mode);
        }
    }

    pub fn frame_position(
        &self,
        path: PathBuf,
        id: u64,
        fallback: [i32; 2],
        viewport: [f32; 2],
    ) -> Binding<[i32; 2]> {
        let app = self.clone();
        let write = self.clone();
        let target = path.clone();
        Binding::new(
            move || {
                app.canvas_settings(&path)
                    .frames
                    .into_iter()
                    .find(|frame| frame.id == id)
                    .map_or(fallback, |frame| frame.position)
            },
            move |position| {
                write.update_canvas_motion(&target, |canvas| {
                    canvas.begin_navigation(viewport);
                    if let Some(frame) = canvas.frames.iter_mut().find(|frame| frame.id == id) {
                        frame.position = position;
                        canvas.active_frame = id;
                    }
                })
            },
        )
    }

    pub fn restore_canvas_view(&self, path: &Path, viewport: [f32; 2]) {
        let workspace = self.workspace.get();
        if workspace
            .canvas(path)
            .is_some_and(|canvas| !canvas.frames.is_empty() && !canvas.has_visible_frame(viewport))
        {
            self.update_canvas(path, |canvas| canvas.recover_view(viewport));
        }
    }

    pub fn canvas_navigation(&self, path: PathBuf, viewport: [f32; 2]) -> Binding<PreviewGesture> {
        let app = self.clone();
        Binding::new(
            || PreviewGesture::Pan([0.; 2]),
            move |gesture| {
                app.update_canvas_motion(&path, |canvas| match gesture {
                    PreviewGesture::Pan(delta) => canvas.pan_by(viewport, delta),
                    PreviewGesture::Zoom { factor, anchor } => {
                        let target = canvas.visible_zoom(viewport) * factor;
                        canvas.zoom_at(viewport, target, anchor);
                    }
                })
            },
        )
    }

    pub fn analyze_visual(
        &self,
        path: PathBuf,
        text: TextSnapshot,
    ) -> impl Future<Output = ()> + 'static + use<> {
        let app = self.clone();
        async move {
            let identity = text.identity();
            let result = match &app.service {
                Some(service) => service.analyze_views(text).await,
                None => Err("Preview worker unavailable.".into()),
            };
            if app.current_visual_document(&path, identity).is_some() {
                let previous = app
                    .visual_result
                    .get()
                    .filter(|previous| previous.path == path);
                let (preview_identity, declarations, error) = match result {
                    Ok(mut declarations) => {
                        if let Some(previous) = previous.as_ref() {
                            for declaration in declarations
                                .iter_mut()
                                .filter(|declaration| declaration.tree.is_err())
                            {
                                declaration.fallback = previous
                                    .at(declaration.range.start)
                                    .filter(|old| old.range.start == declaration.range.start)
                                    .and_then(|old| old.preview(previous.preview_identity));
                            }
                        }
                        (identity, declarations.into(), None)
                    }
                    Err(error) => {
                        if let Some(previous) = previous {
                            (
                                previous.preview_identity,
                                previous.declarations,
                                Some(error),
                            )
                        } else {
                            (identity, Default::default(), Some(error))
                        }
                    }
                };
                app.visual_result.set(Some(PreviewResult {
                    path,
                    identity,
                    preview_identity,
                    declarations,
                    error,
                }));
                app.visual_message.set(String::new());
            }
        }
    }

    fn current_visual_document(&self, path: &Path, identity: u64) -> Option<omen::EditorDocument> {
        if self.workspace.get().active_file.as_deref() != Some(path) {
            return None;
        }
        let document = self.editor_document(path.to_owned());
        (document.state.get().text.identity() == identity).then_some(document)
    }

    pub fn visual_selector(
        &self,
        path: PathBuf,
        identity: u64,
        tree: Arc<VisualTree>,
    ) -> Rc<dyn Fn(usize)> {
        let app = self.clone();
        Rc::new(move |index| {
            let Some(document) = app.current_visual_document(&path, identity) else {
                return;
            };
            if let Some(node) = tree.nodes.get(index) {
                let start = node.name_range.start;
                document.state.update(|state| {
                    state.selection = start..start;
                    state.selection_request = Some(start..start);
                    state.secondary_selections = Default::default();
                    state.reveal = None;
                });
                app.visual_message.set(String::new());
            }
        })
    }

    pub fn edit_visual(
        &self,
        path: &Path,
        identity: u64,
        tree: &VisualTree,
        index: usize,
        edit: ComponentEdit,
    ) {
        let result = (|| {
            let document = self
                .current_visual_document(path, identity)
                .ok_or("The source changed. Select the component again.")?;
            let state = document.state.get();
            let (patch, cursor) = tree.edit(&state.text.to_string(), index, edit)?;
            if state.text.slice(patch.range.clone()) != patch.expected {
                return Err("The source range changed.".to_owned());
            }
            let mut text = state.text.clone();
            text.replace(patch.range, &patch.replacement);
            self.checkpoint_visual(path, &state, &text);
            document.state.update(|state| {
                state.text = text;
                state.selection = cursor..cursor;
                state.selection_request = Some(cursor..cursor);
                state.secondary_selections = Default::default();
                state.reveal = None;
                state.completions = Default::default();
            });
            document.emit(EditorEvent::Edited);
            Ok(())
        })();
        self.visual_message.set(result.err().unwrap_or_default());
    }

    pub(super) fn checkpoint_visual(
        &self,
        path: &Path,
        before: &EditorState,
        after: &TextSnapshot,
    ) {
        let mut histories = self.visual_history.borrow_mut();
        let history = histories.entry(path.to_owned()).or_default();
        if history.current != before.text.identity() {
            *history = History::default();
        }
        if history.undo.len() == 64 {
            history.undo.remove(0);
        }
        history.undo.push(before.into());
        history.redo.clear();
        history.current = after.identity();
    }

    pub fn visual_undo(&self, path: &Path, redo: bool) {
        if self.workspace.get().active_file.as_deref() != Some(path) {
            return;
        }
        let document = self.editor_document(path.to_owned());
        let state = document.state.get();
        let revision = {
            let mut histories = self.visual_history.borrow_mut();
            histories.get_mut(path).and_then(|history| {
                if history.current != state.text.identity() {
                    return None;
                }
                let (from, to) = if redo {
                    (&mut history.redo, &mut history.undo)
                } else {
                    (&mut history.undo, &mut history.redo)
                };
                let revision = from.pop()?;
                to.push((&state).into());
                history.current = revision.text.identity();
                Some(revision)
            })
        };
        let Some(revision) = revision else {
            self.visual_message.set(
                if redo {
                    "No Design edit to redo."
                } else {
                    "No Design edit to undo."
                }
                .into(),
            );
            return;
        };
        document.state.update(|state| {
            state.text = revision.text;
            state.selection_request = Some(revision.selection.clone());
            state.selection = revision.selection;
            state.secondary_selections = Default::default();
            state.reveal = None;
            state.completions = Default::default();
        });
        document.emit(EditorEvent::Edited);
    }
}
