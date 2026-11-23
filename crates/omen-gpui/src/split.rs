//! Pointer capture for Omen panel resize handles, using GPUI's drag lifecycle.
use gpui::{App, Context, IntoElement, Pixels, Point, Render, Window, div, prelude::*};
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
struct ResizeDrag {
    key: String,
    origin: Rc<Cell<f32>>,
    width: Rc<Cell<f32>>,
}
impl Render for ResizeDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub(crate) fn configure(
    element: gpui::Stateful<gpui::Div>,
    key: &str,
    handle: &omen_core::ResizeHandle,
) -> gpui::Stateful<gpui::Div> {
    let key = key.to_string();
    let drag_key = key.clone();
    let width = handle.width.clone();
    let resize = handle.clone();
    let keyboard = handle.clone();
    element
        .role(gpui::Role::Splitter)
        .aria_label("Resize panel")
        .cursor(gpui::CursorStyle::ResizeLeftRight)
        .on_drag(
            ResizeDrag {
                key: drag_key,
                origin: Rc::new(Cell::new(0.)),
                width: Rc::new(Cell::new(0.)),
            },
            move |drag: &ResizeDrag, _: Point<Pixels>, window, cx| {
                drag.origin.set(window.mouse_position().x.into());
                drag.width.set(width.get());
                cx.new(|_| drag.clone())
            },
        )
        .on_drag_move(
            move |event: &gpui::DragMoveEvent<ResizeDrag>, window: &mut Window, cx: &mut App| {
                let drag = event.drag(cx);
                if drag.key != key {
                    return;
                }
                let x: f32 = event.event.position.x.into();
                let delta = (x - drag.origin.get()) * if resize.reverse { -1. } else { 1. };
                resize
                    .width
                    .set((drag.width.get() + delta).clamp(resize.min, resize.max.max(resize.min)));
                window.refresh();
            },
        )
        .on_key_down(move |event, window, cx| {
            let delta = match event.keystroke.key.as_str() {
                "left" => -10.,
                "right" => 10.,
                _ => return,
            };
            let delta = delta * if keyboard.reverse { -1. } else { 1. };
            keyboard.width.set(
                (keyboard.width.get() + delta).clamp(keyboard.min, keyboard.max.max(keyboard.min)),
            );
            window.refresh();
            cx.stop_propagation();
        })
}
