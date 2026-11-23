use super::*;
use omen_core::{Animation, Easing, PresentationKind, ViewKind};
use std::time::{Duration, Instant};

const MENU_FADE: Animation = Animation {
    duration: Duration::from_millis(140),
    easing: Easing::EaseOut,
    spring: None,
};

// A menu is mounted at the presentation root, while its trigger can be a
// virtualized row several element-ID scopes below it. Keyed element state is
// local to those scopes, so the captured pointer position must be window-wide.
#[derive(Clone, Copy)]
struct ContextAnchor {
    position: gpui::Point<gpui::Pixels>,
    opened_at: Instant,
}

#[derive(Default)]
struct ContextAnchors(std::collections::HashMap<gpui::WindowId, ContextAnchor>);
impl gpui::Global for ContextAnchors {}
pub(crate) fn context_position_updater(
    window: &mut Window,
    _: &mut App,
) -> impl Fn(gpui::Point<gpui::Pixels>, &mut App) + 'static + use<> {
    let id = window.window_handle().window_id();
    move |position, cx| {
        let windows: std::collections::HashSet<_> = cx
            .windows()
            .iter()
            .map(|window| window.window_id())
            .collect();
        let anchors = cx.default_global::<ContextAnchors>();
        anchors.0.retain(|id, _| windows.contains(id));
        anchors.0.insert(
            id,
            ContextAnchor {
                position,
                opened_at: Instant::now(),
            },
        );
    }
}
fn context_anchor(window: &Window, cx: &mut App) -> Option<ContextAnchor> {
    cx.default_global::<ContextAnchors>()
        .0
        .get(&window.window_handle().window_id())
        .copied()
}

pub(crate) struct Layer {
    key: String,
    focus: gpui::FocusHandle,
    previous: Option<gpui::FocusHandle>,
    opened_at: Instant,
}
#[derive(Default)]
pub(crate) struct Layers {
    layers: Vec<Layer>,
}
fn visible(node: &ViewNode, output: &mut Vec<String>) {
    match &node.kind {
        ViewKind::Presentation { children, .. } => {
            if let Some(base) = children.first() {
                visible(base, output);
            }
            if let Some(key) = &node.style.runtime_id {
                output.push(key.clone());
            }
            if let Some(content) = children.get(1) {
                visible(content, output);
            }
        }
        ViewKind::Group(children)
        | ViewKind::Layout(children)
        | ViewKind::List(children)
        | ViewKind::Card(children) => {
            for child in children {
                visible(child, output);
            }
        }
        _ => {}
    }
}
pub(crate) fn reconcile(root: &ViewNode, window: &mut Window, cx: &mut App) {
    let mut keys = Vec::new();
    visible(root, &mut keys);
    let state = window.use_keyed_state("omen-presentations", cx, |_, _| Layers::default());
    state.update(cx, |state, cx| {
        let common = state
            .layers
            .iter()
            .zip(&keys)
            .take_while(|(old, new)| old.key == **new)
            .count();
        while state.layers.len() > common {
            if let Some(layer) = state.layers.pop()
                && let Some(previous) = layer.previous
            {
                previous.focus(window, cx);
            }
        }
        for key in keys.into_iter().skip(common) {
            let previous = window.focused(cx);
            let focus = cx.focus_handle().tab_stop(false);
            focus.focus(window, cx);
            state.layers.push(Layer {
                key,
                focus,
                previous,
                opened_at: Instant::now(),
            });
        }
    });
}
pub(crate) fn dismiss_action(node: &ViewNode) -> Option<omen_core::Action> {
    match &node.kind {
        ViewKind::Presentation {
            children, dismiss, ..
        } => children
            .get(1)
            .and_then(dismiss_action)
            .or_else(|| Some(dismiss.clone())),
        ViewKind::Group(children)
        | ViewKind::Layout(children)
        | ViewKind::List(children)
        | ViewKind::Card(children) => children.iter().rev().find_map(dismiss_action),
        _ => None,
    }
}
pub(crate) fn render(
    node: &ViewNode,
    key: &SharedString,
    next_id: &mut usize,
    settings: RenderSettings,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let ViewKind::Presentation {
        children,
        kind,
        dismiss,
    } = &node.kind
    else {
        return div();
    };
    let kind = *kind;
    let blocking = !matches!(
        kind,
        PresentationKind::Popover | PresentationKind::ContextMenu
    );
    let anchor_bounds = Rc::new(std::cell::Cell::new(None::<gpui::Bounds<gpui::Pixels>>));
    let mut base_settings = settings;
    base_settings.inert |= blocking;
    let base = children
        .first()
        .map(|base| render_node(base, next_id, base_settings, window, cx));
    let content = children.get(1);
    let menu = !blocking
        && content
            .and_then(|content| match &content.kind {
                ViewKind::Group(children) => children.first(),
                _ => Some(content),
            })
            .and_then(|content| content.style.preview.as_deref())
            .is_some_and(|preview| preview.menu_panel);
    let mut content_settings = settings;
    if menu {
        content_settings.scale = 1.;
        content_settings.viewport_width = window.viewport_size().width.into();
        content_settings.focus_scroll = None;
    }
    let content =
        content.map(|content| render_node(content, next_id, content_settings, window, cx));
    let state = window.use_keyed_state("omen-presentations", cx, |_, _| Layers::default());
    let layer = state
        .read(cx)
        .layers
        .iter()
        .position(|layer| layer.key == key.as_ref());
    let focus = layer.map(|index| state.read(cx).layers[index].focus.clone());
    let anchor = if kind == PresentationKind::ContextMenu {
        context_anchor(window, cx)
    } else {
        None
    };
    let opacity = if menu && !settings.reduced_motion {
        layer.map_or(1., |index| {
            let opened_at = state.read(cx).layers[index].opened_at;
            let elapsed = anchor
                .map_or(opened_at, |anchor| opened_at.max(anchor.opened_at))
                .elapsed();
            if elapsed < MENU_FADE.duration {
                window.request_animation_frame();
            }
            MENU_FADE.sample(elapsed)
        })
    } else {
        1.
    };
    let viewport = window.viewport_size();
    let panel = div()
        .id(SharedString::from(format!("{key}-panel")))
        .flex()
        .flex_col()
        .when(kind == PresentationKind::ContextMenu, |panel| {
            panel.w(px(280.))
        })
        .max_w(px((f32::from(viewport.width) - 16.).max(1.)))
        .max_h(px((f32::from(viewport.height) - 16.).max(1.)))
        .when(menu, |panel| {
            panel
                .overflow_hidden()
                .opacity(opacity)
                .debug_selector(|| "studio-menu-panel".into())
        })
        .when(!menu, |panel| {
            panel
                .p(px(if kind == PresentationKind::ContextMenu {
                    4.
                } else {
                    20.
                }))
                .gap(px(12.))
                .max_w(px((f32::from(viewport.width) - 32.).max(1.)))
                .max_h(px((f32::from(viewport.height) - 32.).max(1.)))
                .overflow_y_scroll()
        })
        .rounded(px(settings.tokens.radius))
        .bg(rgb(settings.tokens.surface))
        .text_color(rgb(settings.tokens.primary))
        .border_1()
        .border_color(rgb(settings.tokens.border))
        .when(menu, |panel| panel.occlude())
        .when(!menu, |panel| panel.block_mouse_except_scroll())
        .children(content);
    let mut panel = if let Some(focus) = focus {
        panel.track_focus(&focus)
    } else {
        panel
    };
    panel = panel.role(if kind == PresentationKind::Dialog {
        gpui::Role::Dialog
    } else if kind == PresentationKind::ContextMenu {
        gpui::Role::Menu
    } else {
        gpui::Role::Group
    });
    if kind == PresentationKind::ContextMenu {
        panel =
            panel
                .tab_group()
                .on_key_down(|event, window, cx| match event.keystroke.key.as_str() {
                    "down" => {
                        window.focus_next(cx);
                        cx.stop_propagation();
                    }
                    "up" => {
                        window.focus_prev(cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                });
    }
    if !blocking {
        let anchor = anchor_bounds.clone();
        let outside = dismiss.clone();
        panel = panel.on_mouse_down_out(move |event, window, _| {
            // Let the trigger toggle itself; clicks elsewhere dismiss the popover.
            if kind == PresentationKind::ContextMenu
                || !anchor
                    .get()
                    .is_some_and(|bounds| bounds.contains(&event.position))
            {
                outside.invoke();
                window.refresh();
            }
        });
    }
    let overlay = if blocking {
        let backdrop = dismiss.clone();
        let mut overlay = div()
            .id(SharedString::from(format!("{key}-backdrop")))
            .w(viewport.width)
            .h(viewport.height)
            .flex()
            .items_center()
            .bg(gpui::rgba(0x00000080))
            .block_mouse_except_scroll()
            .on_click(move |_, window, _| {
                backdrop.invoke();
                window.refresh();
            });
        overlay = if kind == PresentationKind::Sheet {
            overlay.justify_end().flex_col()
        } else {
            overlay.justify_center()
        };
        overlay
            .child(panel.on_click(|_, _, cx| cx.stop_propagation()))
            .into_any_element()
    } else {
        panel.into_any_element()
    };
    let mut anchored = gpui::anchored()
        .snap_to_window_with_margin(px(8.))
        .child(overlay);
    if blocking {
        anchored = anchored.position(gpui::point(px(0.), px(0.)));
    } else if kind == PresentationKind::ContextMenu {
        anchored = anchored
            .position(anchor.map_or_else(|| window.mouse_position(), |anchor| anchor.position));
    }
    let deferred = gpui::deferred(anchored).with_priority(layer.unwrap_or_default() + 10);
    div()
        .relative()
        .when(super::fills_height(node), |element| element.h_full())
        .children(base)
        .when(!blocking, |element| {
            element.child(
                gpui::canvas(
                    move |bounds, _, _| anchor_bounds.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
        })
        .child(if menu && kind == PresentationKind::Popover {
            div()
                .absolute()
                .left_0()
                .top(gpui::relative(1.))
                .mt(px(4.))
                .child(deferred)
                .into_any_element()
        } else {
            deferred.into_any_element()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[gpui::test]
    fn studio_menus_anchor_below_the_left_edge_at_window_scale(cx: &mut gpui::TestAppContext) {
        use omen_ui::{AnyView, BoolValue, Button, Layout, View, ViewExt};

        for scale in [0.5, 1.5] {
            let visible = BoolValue::new(false);
            let open = visible.clone();
            let trigger = Button::new("Options", move || open.set(true))
                .frame(100, 24)
                .popover(visible, || {
                    let mut content = Layout::new().frame(180, 100).into_node();
                    content.style.preview_mut().menu_panel = true;
                    AnyView::new(content)
                });
            let mut root = Layout::new().child(trigger).padding(40).into_node();
            root.style.preview_mut().scale = Some(scale);
            let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(AnyView::new(root)));
            vcx.simulate_click(
                gpui::point(px(60. * scale), px(50. * scale)),
                Default::default(),
            );
            vcx.run_until_parked();
            let menu = vcx.debug_bounds("studio-menu-panel").expect("menu panel");
            assert_eq!(
                menu.origin,
                gpui::point(px(40. * scale), px(64. * scale + 4.))
            );
            assert_eq!(menu.size, gpui::size(px(182.), px(102.)));
        }
    }

    #[gpui::test]
    fn context_position_is_shared_between_row_and_root_element_scopes(
        cx: &mut gpui::TestAppContext,
    ) {
        let window = cx.add_empty_window();
        window.update(|window, cx| {
            let update = window.with_id("virtualized-explorer-row", |window| {
                context_position_updater(window, cx)
            });
            update(gpui::point(px(130.), px(240.)), cx);
            let position = window.with_id("studio-presentation-root", |window| {
                context_anchor(window, cx).map(|anchor| anchor.position)
            });
            assert_eq!(position, Some(gpui::point(px(130.), px(240.))));
        });
    }
}
