//! Native title-bar configuration and Omen toolbar hosting.
use super::*;
use omen_core::DesktopWindow;
use std::cell::Cell;

pub(super) fn options(config: &DesktopWindow, title: String) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        appears_transparent: config.toolbar.is_some(),
        traffic_light_position: config
            .toolbar
            .as_ref()
            .map(|toolbar| gpui::point(px(12.), px((toolbar.resolved_height() - 16.) / 2.))),
    }
}
pub(super) fn decorations(config: &DesktopWindow) -> Option<gpui::WindowDecorations> {
    if cfg!(target_os = "linux") && config.toolbar.is_some() {
        Some(gpui::WindowDecorations::Client)
    } else {
        None
    }
}
/// Resolve the toolbar's own surface, including its named-view wrappers.
pub(super) fn background(node: &ViewNode, inherited: omen_core::DesignTokens, dark: bool) -> u32 {
    let tokens = node
        .style
        .theme
        .map_or(inherited, |theme| theme.resolve(dark));
    if let Some(omen_core::Background::Color(color)) = node.style.background {
        return tokens.color(color);
    }
    if let ViewKind::Group(children) = &node.kind
        && let [child] = children.as_slice()
    {
        return background(child, tokens, dark);
    }
    tokens.surface
}

pub(super) fn drag_state(key: &str, window: &mut Window, cx: &mut App) -> Rc<Cell<bool>> {
    window
        .use_keyed_state(
            SharedString::from(format!("{key}-titlebar-drag")),
            cx,
            |_, _| Rc::new(Cell::new(false)),
        )
        .read(cx)
        .clone()
}

pub(super) fn render(
    content: gpui::Stateful<Div>,
    height: f32,
    key: &str,
    background: u32,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<Div> {
    let moving = drag_state(key, window, cx);
    let pressed = moving.clone();
    let released = moving.clone();
    let outside = moving.clone();
    let fullscreen = window.is_fullscreen() || window.is_simple_fullscreen();
    let mut bar = div()
        .id(SharedString::from(format!("{key}-titlebar")))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(height))
        .flex_shrink_0()
        .bg(rgb(background))
        .window_control_area(gpui::WindowControlArea::Drag)
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| pressed.set(true))
        .on_mouse_up(gpui::MouseButton::Left, move |_, _, _| released.set(false))
        .on_mouse_down_out(move |_, _, _| outside.set(false))
        .on_mouse_move(move |event, window, _| {
            if moving.replace(false) && event.pressed_button == Some(gpui::MouseButton::Left) {
                window.start_window_move();
            }
        })
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.titlebar_double_click();
            }
        });
    if cfg!(target_os = "macos") && !fullscreen {
        bar = bar.pl(px(80.));
    }
    bar = bar.child(content.flex_1().min_w_0().h_full());
    #[cfg(not(target_os = "macos"))]
    if !fullscreen {
        for (area, label, symbol, enabled) in [
            (
                gpui::WindowControlArea::Min,
                "Minimize window",
                "−",
                window.window_controls().minimize,
            ),
            (
                gpui::WindowControlArea::Max,
                "Maximize or restore window",
                "□",
                window.window_controls().maximize && window.is_resizable(),
            ),
            (gpui::WindowControlArea::Close, "Close window", "×", true),
        ] {
            if !enabled {
                continue;
            }
            bar = bar.child(
                div()
                    .id(SharedString::from(format!("{key}-{label}")))
                    .role(gpui::Role::Button)
                    .aria_label(label)
                    .window_control_area(area)
                    .w(px(44.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_a11y_action(gpui::AccessibleAction::Click, move |_, window, _| {
                        activate_caption(area, window)
                    })
                    .on_click(move |_, window, cx| {
                        activate_caption(area, window);
                        cx.stop_propagation();
                    })
                    .child(symbol),
            );
        }
    }
    bar
}

#[cfg(not(target_os = "macos"))]
fn activate_caption(area: gpui::WindowControlArea, window: &mut Window) {
    match area {
        gpui::WindowControlArea::Min => window.minimize_window(),
        gpui::WindowControlArea::Max => window.zoom_window(),
        gpui::WindowControlArea::Close => window.remove_window(),
        gpui::WindowControlArea::Drag => {}
    }
}
