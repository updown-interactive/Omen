//! Retained native windows for declarative desktop scene requests.
use super::*;
use std::collections::HashMap;

struct SceneWindow {
    handle: gpui::WindowHandle<OmenWindow>,
    _closed: gpui::Subscription,
}
#[derive(Default)]
pub(super) struct Windows(HashMap<String, SceneWindow>);
pub(super) fn declarations(root: &ViewNode, output: &mut Vec<omen_core::DesktopScene>) {
    output.extend(root.style.desktop_scenes.clone());
    match &root.kind {
        ViewKind::Layout(children)
        | ViewKind::Group(children)
        | ViewKind::List(children)
        | ViewKind::Card(children)
        | ViewKind::Presentation { children, .. } => {
            for child in children {
                declarations(child, output);
            }
        }
        _ => {}
    }
}
impl Windows {
    pub(super) fn reconcile(&mut self, requested: Vec<omen_core::DesktopScene>, cx: &mut App) {
        self.0.retain(|key, scene| {
            if requested.iter().any(|request| request.key == *key) {
                return true;
            }
            let _ = scene
                .handle
                .update(cx, |_, window, _| window.remove_window());
            false
        });
        for request in requested {
            if self.0.contains_key(&request.key) {
                continue;
            }
            let content = request.content();
            let config = content.style.desktop_window.clone().unwrap_or_default();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(config.width), px(config.height)),
                    cx,
                ))),
                window_min_size: Some(size(
                    px(config.min_width.max(64.)),
                    px(config.min_height.max(64.)),
                )),
                titlebar: Some(titlebar::options(&config, config.title.clone())),
                app_owns_titlebar_drag: config.toolbar.is_some(),
                window_decorations: titlebar::decorations(&config),
                ..Default::default()
            };
            match cx.open_window(options, |_, cx| cx.new(|_| OmenWindow::new(content))) {
                Ok(handle) => {
                    let id = handle.window_id();
                    let visible = request.visible.clone();
                    let closed = cx.on_window_closed(move |_, closed| {
                        if id == closed {
                            visible.set(false);
                        }
                    });
                    self.0.insert(
                        request.key,
                        SceneWindow {
                            handle,
                            _closed: closed,
                        },
                    );
                }
                Err(error) => {
                    eprintln!("Unable to open {}: {error}", request.key);
                    request.visible.set(false);
                }
            }
        }
    }
}
