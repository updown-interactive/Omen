//! Inspector edits share the editor's persistent buffer, history and save pipeline.
use super::*;
use crate::state::InspectorState;
use omen::{EditorEvent, TextSnapshot};
use omen_macro_core::inspector;
impl StudioEnvironment {
    pub fn focus_inspector(&self) {
        if self.layout.get().preview_visible {
            self.inspector_focus.set(true);
        }
    }
    pub fn focus_editor(&self) {
        self.inspector_focus.set(false);
        if let Some(path) = self.workspace.get().active_file {
            self.editor_document(path)
                .state
                .update(|state| state.reveal = Some(state.selection.clone()));
        }
    }

    pub fn inspector_task(
        &self,
        path: PathBuf,
        text: TextSnapshot,
        cursor: usize,
    ) -> impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + 'static + use<>
    {
        let app = self.clone();
        move || {
            let app = app.clone();
            let path = path.clone();
            let text = text.clone();
            Box::pin(async move {
                let identity = text.identity();
                let result = app.inspector_service.inspect(text, cursor).await;
                if app.workspace.get().active_file.as_ref() != Some(&path) {
                    return;
                }
                let Some(editor) = app.editor_states.borrow().get(&path).cloned() else {
                    return;
                };
                let state = editor.get();
                if state.text.identity() != identity || state.selection.end != cursor {
                    return;
                }
                let (node, message) = match result {
                    Ok(Some(node)) => (Some(node), String::new()),
                    Ok(None) => (
                        None,
                        "Select a component inside view! to inspect its properties.".into(),
                    ),
                    Err(message) => (None, message),
                };
                let revision = app.inspector.get().revision.wrapping_add(1);
                app.inspector.set(InspectorState {
                    revision,
                    path: Some(path),
                    identity,
                    cursor,
                    node,
                    message,
                });
            })
        }
    }
    pub fn inspector_set(&self, key: &str, value: &str, replace_expression: bool, identity: u64) {
        self.inspector_patch(identity, |source, node| {
            let string = node
                .properties
                .iter()
                .find(|property| property.key == key)
                .is_some_and(|property| property.string);
            let value = if string {
                format!("{value:?}")
            } else {
                value.to_string()
            };
            inspector::set_property(source, node, key, &value, replace_expression)
        });
    }
    pub fn inspector_rename(&self, name: &str, revision: u64) {
        self.inspector_patch(revision, |source, node| {
            inspector::set_name(source, node, name)
        });
    }
    pub fn inspector_remove(&self, key: &str, identity: u64) {
        self.inspector_patch(identity, |source, node| {
            inspector::remove_modifier(source, node, key)
        });
    }
    pub fn inspector_add(&self, name: &str, identity: u64) {
        self.inspector_patch(identity, |source, node| {
            inspector::add_modifier(source, node, name)
        });
    }
    pub fn inspector_layout_direction(&self, value: &str, revision: u64) {
        self.inspector_patch(revision, |source, node| {
            inspector::set_layout_direction(source, node, value)
        });
    }
    fn inspector_patch(
        &self,
        identity: u64,
        plan: impl FnOnce(&str, &inspector::InspectedNode) -> Result<inspector::SourcePatch, String>,
    ) {
        let result = (|| {
            let inspected = self.inspector.get();
            let path = inspected.path.as_ref().ok_or("No component selected")?;
            if self.workspace.get().active_file.as_ref() != Some(path) {
                return Err("The active file changed. Select the component again.".to_string());
            }
            let node = inspected.node.as_ref().ok_or("No component selected")?;
            let document = self.editor_document(path.clone());
            let state = document.state.get();
            if identity != inspected.revision
                || state.text.identity() != inspected.identity
                || state.selection.end != inspected.cursor
            {
                return Err(
                    "The source changed. Select the component again before editing.".to_string(),
                );
            }
            let source = state.text.to_string();
            let patch = plan(&source, node)?;
            if state.text.slice(patch.range.clone()) != patch.expected {
                return Err("The source range changed".to_string());
            }
            let mut text = state.text.clone();
            text.replace(patch.range.clone(), &patch.replacement);
            let offset = node.name_range.start.min(text.len());
            let updated_source = text.to_string();
            let changes_modifier = node.properties.iter().any(|property| {
                property.modifier_range.as_ref().is_some_and(|range| {
                    patch.range.start <= range.start && patch.range.end >= range.end
                })
            });
            let rebased = if patch.range.is_empty() || changes_modifier {
                // Modifier insertion, removal and renaming change the property registry.
                inspector::inspect(&updated_source, offset)?
                    .ok_or("The component could not be inspected.")?
            } else {
                let mut rebased = node.clone();
                inspector::rebase(&mut rebased, &patch, &updated_source);
                rebased
            };
            let updated_identity = text.identity();
            if self.has_visual_preview(path, &state.text) {
                self.checkpoint_visual(path, &state, &text);
            }
            document.state.update(|state| {
                state.text = text;
                state.selection = offset..offset;
                state.reveal = None;
                state.completions = Default::default();
            });
            self.inspector.update(|inspected| {
                inspected.revision = inspected.revision.wrapping_add(1);
                inspected.identity = updated_identity;
                inspected.cursor = offset;
                inspected.node = Some(rebased);
            });
            document.emit(EditorEvent::Edited);
            Ok(())
        })();
        if let Err(message) = result {
            self.inspector
                .update(|inspector| inspector.message = message);
        }
    }
    pub fn inspector_reveal(&self, key: &str) {
        let inspected = self.inspector.get();
        let Some(path) = inspected.path else {
            return;
        };
        let Some(node) = inspected.node else {
            return;
        };
        let Some(property) = node.properties.iter().find(|property| property.key == key) else {
            return;
        };
        if self.workspace.get().active_file.as_ref() != Some(&path) {
            return;
        }
        let document = self.editor_document(path.clone());
        let text = document.state.read(|state| state.text.clone());
        if text.identity() == inspected.identity {
            if self.canvas_settings(&path).mode == crate::visual::EditorMode::Design
                && self.has_visual_preview(&path, &text)
            {
                self.update_canvas(&path, |canvas| {
                    canvas.mode = crate::visual::EditorMode::Code
                });
            }
            document.state.update(|state| {
                state.selection = property.value_range.clone();
                state.reveal = Some(property.value_range.clone());
            });
        }
    }
}
