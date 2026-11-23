//! Reveal keyboard-focused descendants in their nearest native scroll viewport.
use super::*;
use gpui::{Bounds, FocusHandle, Pixels, ScrollHandle, WindowId, canvas};
use std::{cell::Cell, collections::HashMap};
#[derive(Default)]
struct Registry(HashMap<WindowId, HashMap<usize, ScrollHandle>>);
impl gpui::Global for Registry {}
pub(super) fn begin(window: &Window, cx: &mut App) {
    cx.default_global::<Registry>()
        .0
        .remove(&window.window_handle().window_id());
}
pub(super) fn register(id: usize, handle: ScrollHandle, window: &Window, cx: &mut App) {
    cx.default_global::<Registry>()
        .0
        .entry(window.window_handle().window_id())
        .or_default()
        .insert(id, handle);
}
pub(super) fn handle(id: Option<usize>, window: &Window, cx: &mut App) -> Option<ScrollHandle> {
    let id = id?;
    cx.default_global::<Registry>()
        .0
        .get(&window.window_handle().window_id())?
        .get(&id)
        .cloned()
}
pub(super) fn marker(
    focus: FocusHandle,
    scroll: ScrollHandle,
    key: &SharedString,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let was_focused = window
        .use_keyed_state(
            SharedString::from(format!("{key}-focus-reveal")),
            cx,
            |_, _| Rc::new(Cell::new(false)),
        )
        .read(cx)
        .clone();
    canvas(
        move |bounds, window, cx| {
            let focused = focus.is_focused(window);
            if focused && !was_focused.replace(focused) {
                reveal(bounds, &scroll, window, cx);
            } else if !focused {
                was_focused.set(false);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
    .size_full()
    .into_any_element()
}
fn reveal(bounds: Bounds<Pixels>, scroll: &ScrollHandle, window: &mut Window, _: &mut App) {
    let viewport = scroll.bounds();
    if viewport.size.width <= px(0.) || viewport.size.height <= px(0.) {
        return;
    }
    let mut offset = scroll.offset();
    if bounds.top() < viewport.top() {
        offset.y += viewport.top() - bounds.top();
    } else if bounds.bottom() > viewport.bottom() {
        offset.y -= bounds.bottom() - viewport.bottom();
    }
    if bounds.left() < viewport.left() {
        offset.x += viewport.left() - bounds.left();
    } else if bounds.right() > viewport.right() {
        offset.x -= bounds.right() - viewport.right();
    }
    if offset != scroll.offset() {
        scroll.set_offset(offset);
        window.refresh();
    }
}
