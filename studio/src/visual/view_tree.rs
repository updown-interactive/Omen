//! A window-scale hierarchy for the active Design frame.
use super::{VisualFrame, VisualTree, render};
use crate::app::StudioEnvironment;
use omen::{AnyView, List, View, ViewExt, ViewNode, view};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Clone, Default)]
pub(super) struct ViewTreeCache(Rc<RefCell<Option<CachedViewTree>>>);

struct CachedViewTree {
    tree: Arc<VisualTree>,
    frame_name: String,
    frame_width: u16,
    selected: Option<usize>,
    viewport: [f32; 2],
    view: AnyView,
}

impl ViewTreeCache {
    pub fn view(
        &self,
        tree: &Arc<VisualTree>,
        frame: &VisualFrame,
        selected: Option<usize>,
        select: &render::Select,
        viewport: [f32; 2],
    ) -> AnyView {
        let mut cache = self.0.borrow_mut();
        if let Some(cached) = cache.as_ref()
            && Arc::ptr_eq(&cached.tree, tree)
            && cached.frame_width == frame.width
            && cached.frame_name == frame.name
            && cached.selected == selected
            && cached.viewport == viewport
        {
            return cached.view.clone();
        }

        let margin = 10.;
        let width = (viewport[0] * 0.34)
            .clamp(148., 208.)
            .min(viewport[0] - margin * 2.);
        let max_height = (viewport[1] - margin * 2.).clamp(34., 342.);
        let mut node = ViewTree {
            tree: tree.clone(),
            frame_name: frame.name.clone(),
            frame_width: f32::from(frame.width),
            selected,
            select: select.clone(),
            width,
            max_height,
        }
        .key("view-tree")
        .into_node();
        let preview = node.style.preview_mut();
        preview.position = Some([viewport[0] - margin - width, margin]);
        preview.floating_panel = true;
        let view = AnyView::new(node);
        *cache = Some(CachedViewTree {
            tree: tree.clone(),
            frame_name: frame.name.clone(),
            frame_width: frame.width,
            selected,
            viewport,
            view: view.clone(),
        });
        view
    }
}

view! {
    ViewTree(tree: Arc<VisualTree>, frame_name: String, frame_width: f32, selected: Option<usize>, select: render::Select, width: f32, max_height: f32) {
        @Environment app: StudioEnvironment
        @State expanded = true
        let colors = app.colors();
        let rows = if expanded { tree_rows(&tree, frame_width) } else { Vec::new() };
        let body_height = (rows.len() as f32 * 27. + 8.).min(max_height - 34.);
        let show_rows = expanded && body_height >= 27.;
        Layout {
            Layout {
                Text("View tree").fontSize(12).weight(.semibold)
                Text(frame_name).fontSize(10).color(colors.on_surface_variant).truncate().fill()
                Button(if expanded { "−" } else { "+" }) { expanded = !expanded; }
                    .frame(22, 22).cornerRadius(4).hoverBackground(colors.surface)
                    .accessibilityLabel(if expanded { "Collapse view tree" } else { "Expand view tree" })
            }
            .mode(.horizontal).spacing(7).padding(6).frame(.infinity, 32)
            if show_rows {
                Divider()
                ViewTreeRows(rows: rows, selected: selected, select: select.clone())
                    .padding(4).frame(.infinity, body_height)
            }
        }
        .frame(width, if show_rows { body_height + 34. } else { 34. })
        .cornerRadius(8).background(colors.surface_container_high).color(colors.on_surface)
    }
}

struct ViewTreeRows {
    rows: Vec<(usize, String)>,
    selected: Option<usize>,
    select: render::Select,
}

impl View for ViewTreeRows {
    fn into_node(self) -> ViewNode {
        List::new()
            .children(
                self.rows
                    .into_iter()
                    .enumerate()
                    .map(|(index, (depth, label))| {
                        ViewTreeRow {
                            index,
                            depth,
                            label,
                            selected: self.selected == Some(index),
                            select: self.select.clone(),
                        }
                        .key(index)
                        .into_node()
                    }),
            )
            .virtualized(27.)
            .into_node()
    }
}

view! {
    ViewTreeRow(index: usize, depth: usize, label: String, selected: bool, select: render::Select) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let row_label = label.clone();
        Layout {
            Layout {}.frame(depth.min(8) as f32 * 9., 1)
            Icon("file-omen-view").frame(12,12)
                .color(if selected { colors.on_selection } else { colors.accent_foreground })
            Text(label.clone()).fontSize(11).truncate().fill()
        }
        .mode(.horizontal).spacing(5).padding(5).frame(.infinity, 27)
        .cornerRadius(4)
        .background(if selected { colors.selection } else { colors.surface_container_high })
        .hoverBackground(if selected { colors.selection } else { colors.surface })
        .color(if selected { colors.on_selection } else { colors.on_surface })
        .onTap(move || select(index))
        .accessibilityRole(.button).accessibilityLabel(row_label)
    }
}

fn tree_rows(tree: &VisualTree, width: f32) -> Vec<(usize, String)> {
    let mut rows: Vec<(usize, String)> = Vec::with_capacity(tree.nodes.len());
    for (index, node) in tree.nodes.iter().enumerate() {
        let depth = tree.parent(index).map_or(0, |parent| rows[parent].0 + 1);
        let label = node
            .design_name()
            .unwrap_or_else(|| render::component_label(node, width));
        rows.push((depth, label));
    }
    rows
}
