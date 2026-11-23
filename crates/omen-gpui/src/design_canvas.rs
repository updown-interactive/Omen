//! Native event adapters for the Studio preview canvas.
use gpui::{
    Context, Div, IntoElement, Render, ScrollHandle, Stateful, Window, div, prelude::*, rgb,
};
use omen_core::{
    Binding, FlowPlacement, LayoutMode, PreviewFrameDrag, PreviewGesture, PreviewPlacement,
};
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
struct ComponentDrag {
    scope: String,
    source: ComponentSource,
    label: String,
}

#[derive(Clone)]
enum ComponentSource {
    Existing(std::ops::Range<usize>),
    Library(Binding<(usize, FlowPlacement)>),
}

pub(crate) fn insert(element: Stateful<Div>, insert: &omen_core::PreviewInsert) -> Stateful<Div> {
    let begin = insert.begin.clone();
    element.on_drag(
        ComponentDrag {
            scope: insert.scope.clone(),
            source: ComponentSource::Library(insert.insert.clone()),
            label: insert.label.clone(),
        },
        move |drag, _, window, cx| {
            let ghost = cx.new(|_| drag.clone());
            window.defer(cx, {
                let begin = begin.clone();
                move |window, _| {
                    begin.invoke();
                    window.refresh();
                }
            });
            ghost
        },
    )
}

impl Render for ComponentDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(0x2563eb))
            .text_color(rgb(0xffffff))
            .child(self.label.clone())
    }
}

#[derive(Clone)]
struct ResizeDrag {
    press: Rc<Cell<gpui::Point<gpui::Pixels>>>,
    size: Binding<[f32; 2]>,
    scale: f32,
}

impl Render for ResizeDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(0x2563eb))
            .text_color(rgb(0xffffff))
            .child("Resize")
    }
}

pub(crate) fn resize_handle(
    key: &gpui::SharedString,
    resize: &omen_core::PreviewResize,
    scale: f32,
    accent: u32,
    window: &mut Window,
    cx: &mut gpui::App,
) -> impl IntoElement {
    let pressed = pressed_pointer(key, window, cx);
    let press = pressed.clone();
    div()
        .id(gpui::SharedString::from(format!("{key}-resize")))
        .debug_selector(|| "studio-resize-handle".into())
        .absolute()
        .bottom_0()
        .right_0()
        .size(gpui::px(10.))
        .rounded_sm()
        .bg(rgb(accent))
        .cursor(gpui::CursorStyle::ResizeUpLeftDownRight)
        .aria_label("Resize fixed component")
        .on_mouse_down(gpui::MouseButton::Left, move |event, _, _| {
            press.set(Some(event.position))
        })
        .on_drag(
            ResizeDrag {
                press: Rc::default(),
                size: resize.size.clone(),
                scale,
            },
            move |drag, _, window, cx| {
                drag.press
                    .set(pressed.take().unwrap_or_else(|| window.mouse_position()));
                cx.new(|_| drag.clone())
            },
        )
}

pub(crate) type Placement<'a> = (&'a PreviewPlacement, DropBounds);

pub(crate) type DropBounds = Rc<Cell<gpui::Bounds<gpui::Pixels>>>;

pub(crate) fn placement_bounds(element: Div, bounds: &DropBounds) -> Div {
    let bounds = bounds.clone();
    element.child(
        gpui::canvas(move |area, _, _| bounds.set(area), |_, (), _, _| {})
            .absolute()
            .inset_0(),
    )
}

fn proposal(
    target: &PreviewPlacement,
    drag: &ComponentDrag,
    bounds: gpui::Bounds<gpui::Pixels>,
    pointer: gpui::Point<gpui::Pixels>,
    rtl: bool,
) -> Option<FlowPlacement> {
    if drag.scope != target.scope
        || matches!(&drag.source, ComponentSource::Existing(range) if range.contains(&target.source.start))
        || !bounds.contains(&pointer)
    {
        return None;
    }
    let along =
        |offset: gpui::Pixels, size: gpui::Pixels| f32::from(offset) / f32::from(size).max(1.);
    let fraction = if target.axis == Some(LayoutMode::Horizontal) {
        let fraction = along(pointer.x - bounds.origin.x, bounds.size.width);
        if rtl { 1. - fraction } else { fraction }
    } else {
        along(pointer.y - bounds.origin.y, bounds.size.height)
    };
    if target.container && (target.axis.is_none() || (0.28..=0.72).contains(&fraction)) {
        return Some(FlowPlacement::Into);
    }
    match target.axis {
        Some(LayoutMode::Horizontal | LayoutMode::Vertical) if fraction < 0.5 => {
            Some(FlowPlacement::Before)
        }
        Some(LayoutMode::Horizontal | LayoutMode::Vertical) => Some(FlowPlacement::After),
        _ => None,
    }
}

pub(crate) fn placement(
    mut element: Stateful<Div>,
    target: &PreviewPlacement,
    bounds: DropBounds,
    rtl: bool,
    accent: u32,
) -> Stateful<Div> {
    element = element.debug_selector(|| format!("studio-placement-{}", target.source.start));
    if target.draggable {
        element = element.on_drag(
            ComponentDrag {
                scope: target.scope.clone(),
                source: ComponentSource::Existing(target.source.clone()),
                label: target.label.clone(),
            },
            |drag, _, _, cx| cx.new(|_| drag.clone()),
        );
    }
    let hit_target = target.clone();
    let hit_bounds = bounds.clone();
    let hover_target = target.clone();
    let hover_bounds = bounds.clone();
    let drop_target = target.clone();
    let horizontal = target.axis == Some(LayoutMode::Horizontal);
    element
        .can_drop(move |payload, window, _| {
            payload.downcast_ref::<ComponentDrag>().is_some_and(|drag| {
                proposal(
                    &hit_target,
                    drag,
                    hit_bounds.get(),
                    window.mouse_position(),
                    rtl,
                )
                .is_some()
            })
        })
        .drag_over(move |style, drag: &ComponentDrag, window, _| {
            let resolved = proposal(
                &hover_target,
                drag,
                hover_bounds.get(),
                window.mouse_position(),
                rtl,
            );
            let style = match resolved {
                Some(FlowPlacement::Into) => style.border_1(),
                Some(FlowPlacement::Before) if horizontal && !rtl => style.border_l_2(),
                Some(FlowPlacement::After) if horizontal && rtl => style.border_l_2(),
                Some(FlowPlacement::Before | FlowPlacement::After) if horizontal => {
                    style.border_r_2()
                }
                Some(FlowPlacement::Before) => style.border_t_2(),
                Some(FlowPlacement::After) => style.border_b_2(),
                Some(FlowPlacement::Pinned(_)) | None => return style,
            };
            style.border_color(rgb(accent))
        })
        .on_drop(move |drag: &ComponentDrag, window, cx| {
            if let Some(resolved) = proposal(
                &drop_target,
                drag,
                bounds.get(),
                window.mouse_position(),
                rtl,
            ) {
                match &drag.source {
                    ComponentSource::Existing(source) => {
                        drop_target.drop_from.set((source.start, resolved))
                    }
                    ComponentSource::Library(insert) => {
                        insert.set((drop_target.source.start, resolved))
                    }
                }
                window.refresh();
                cx.stop_propagation();
            }
        })
}

#[derive(Clone)]
struct PinDrag {
    label: String,
    parent_bounds: DropBounds,
    source_bounds: DropBounds,
    geometry: Rc<Cell<(gpui::Size<gpui::Pixels>, gpui::Point<gpui::Pixels>)>>,
    source: usize,
    rtl: bool,
    scale: f32,
    drop_from: Binding<(usize, FlowPlacement)>,
}

impl Render for PinDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(0x2563eb))
            .text_color(rgb(0xffffff))
            .child(self.label.clone())
    }
}

pub(crate) fn move_pinned(
    element: Stateful<Div>,
    (target, source_bounds): Placement<'_>,
    (parent, parent_bounds): &Placement<'_>,
    rtl: bool,
    scale: f32,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Stateful<Div> {
    let key = gpui::SharedString::from(format!("{}-{}-pin", target.scope, target.source.start));
    let pressed = pressed_pointer(&key, window, cx);
    let press = pressed.clone();
    element
        .cursor(gpui::CursorStyle::OpenHand)
        .capture_any_mouse_down(move |event, _, _| {
            if event.button == gpui::MouseButton::Left {
                press.set(Some(event.position));
            }
        })
        .on_drag(
            PinDrag {
                label: target.label.clone(),
                parent_bounds: parent_bounds.clone(),
                source_bounds,
                geometry: Rc::default(),
                source: target.source.start,
                rtl,
                scale,
                drop_from: parent.drop_from.clone(),
            },
            move |drag, _, window, cx| {
                let bounds = drag.source_bounds.get();
                let pointer = pressed.take().unwrap_or_else(|| window.mouse_position());
                drag.geometry.set((bounds.size, pointer - bounds.origin));
                cx.new(|_| drag.clone())
            },
        )
}

fn drop_pin(drag: &PinDrag, pointer: gpui::Point<gpui::Pixels>) {
    let bounds = drag.parent_bounds.get();
    if !bounds.contains(&pointer) {
        return;
    }
    let (size, grab) = drag.geometry.get();
    let origin = pointer - bounds.origin - grab;
    let available_x = (bounds.size.width - size.width).max(gpui::px(0.));
    let available_y = (bounds.size.height - size.height).max(gpui::px(0.));
    let x = origin.x.clamp(gpui::px(0.), available_x);
    let y = origin.y.clamp(gpui::px(0.), available_y);
    let right = x > available_x / 2.;
    let bottom = y > available_y / 2.;
    let corner = match (right ^ drag.rtl, bottom) {
        (false, false) => omen_core::PinCorner::TopLeading,
        (true, false) => omen_core::PinCorner::TopTrailing,
        (false, true) => omen_core::PinCorner::BottomLeading,
        (true, true) => omen_core::PinCorner::BottomTrailing,
    };
    let inset = |offset| {
        (f32::from(offset) / drag.scale)
            .round()
            .clamp(0., f32::from(u16::MAX)) as u16
    };
    let pin = omen_core::Pin {
        corner,
        x: inset(if right { available_x - x } else { x }),
        y: inset(if bottom { available_y - y } else { y }),
    };
    drag.drop_from
        .set((drag.source, FlowPlacement::Pinned(pin)));
}

#[derive(Clone)]
struct FrameDrag {
    start: Rc<Cell<[f32; 2]>>,
    origin: Rc<Cell<[i32; 2]>>,
    last_position: Rc<Cell<[i32; 2]>>,
    pending: Rc<Cell<Option<gpui::Point<gpui::Pixels>>>>,
    position: Binding<[i32; 2]>,
    scale: f32,
}

impl Render for FrameDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn pressed_pointer(
    key: &gpui::SharedString,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Rc<Cell<Option<gpui::Point<gpui::Pixels>>>> {
    window
        .use_keyed_state(
            gpui::SharedString::from(format!("{key}-preview-press")),
            cx,
            |_, _| Rc::new(Cell::new(None::<gpui::Point<gpui::Pixels>>)),
        )
        .read(cx)
        .clone()
}

pub(crate) fn move_frame(
    element: Stateful<Div>,
    key: &gpui::SharedString,
    frame: &PreviewFrameDrag,
    card: bool,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Stateful<Div> {
    let pressed = pressed_pointer(key, window, cx);
    let press = pressed.clone();
    let select = frame.select.clone();
    element
        .debug_selector(move || {
            if card {
                "studio-frame-card"
            } else {
                "studio-frame-grip"
            }
            .into()
        })
        .cursor(gpui::CursorStyle::OpenHand)
        .on_click(move |_, window, cx| {
            select.invoke();
            window.refresh();
            cx.stop_propagation();
        })
        .on_mouse_down(gpui::MouseButton::Left, move |event, _, _| {
            press.set(Some(event.position))
        })
        .on_drag(
            FrameDrag {
                start: Rc::default(),
                origin: Rc::default(),
                last_position: Rc::default(),
                pending: Rc::default(),
                position: frame.position.clone(),
                scale: frame.scale,
            },
            move |drag, _, window, cx| {
                let start = pressed.take().unwrap_or_else(|| window.mouse_position());
                drag.start.set([start.x.into(), start.y.into()]);
                let origin = drag.position.get();
                drag.origin.set(origin);
                drag.last_position.set(origin);
                follow_frame_drag(drag, window.mouse_position());
                cx.new(|_| drag.clone())
            },
        )
}

fn follow_frame_drag(drag: &FrameDrag, point: gpui::Point<gpui::Pixels>) {
    let [start_x, start_y] = drag.start.get();
    let [x, y] = drag.origin.get();
    let px: f32 = point.x.into();
    let py: f32 = point.y.into();
    let moved = [
        x + ((px - start_x) / drag.scale).round() as i32,
        y + ((py - start_y) / drag.scale).round() as i32,
    ]
    .map(|coordinate| coordinate.clamp(-70_000, 70_000));
    if drag.last_position.replace(moved) != moved {
        drag.position.set(moved);
    }
}

#[derive(Clone)]
struct CanvasDrag {
    key: gpui::SharedString,
    last: Rc<Cell<gpui::Point<gpui::Pixels>>>,
    pending: Rc<Cell<Option<gpui::Point<gpui::Pixels>>>>,
    gesture: Binding<PreviewGesture>,
}

impl CanvasDrag {
    fn pan_to(&self, point: gpui::Point<gpui::Pixels>) {
        let delta = point - self.last.replace(point);
        if delta != gpui::Point::default() {
            self.gesture
                .set(PreviewGesture::Pan([delta.x.into(), delta.y.into()]));
        }
    }
}

impl Render for CanvasDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub(crate) fn navigate(
    element: Stateful<Div>,
    key: &gpui::SharedString,
    bounds: ScrollHandle,
    gesture: Binding<PreviewGesture>,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Stateful<Div> {
    let pressed = pressed_pointer(key, window, cx);
    let press = pressed.clone();
    let key = key.clone();
    let scroll = gesture.clone();
    let wheel_bounds = bounds.clone();
    element
        .debug_selector(|| "studio-canvas".into())
        .overflow_hidden()
        .cursor(gpui::CursorStyle::OpenHand)
        .on_mouse_down(gpui::MouseButton::Left, move |event, _, _| {
            press.set(Some(event.position))
        })
        .on_drag(
            CanvasDrag {
                key: key.clone(),
                last: Rc::default(),
                pending: Rc::default(),
                gesture: gesture.clone(),
            },
            move |drag, _, window, cx| {
                let pointer = window.mouse_position();
                drag.last.set(pressed.take().unwrap_or(pointer));
                drag.pan_to(pointer);
                cx.new(|_| drag.clone())
            },
        )
        .on_drag_move(move |event: &gpui::DragMoveEvent<CanvasDrag>, window, cx| {
            let payload = event.drag(cx);
            if payload.key != key {
                return;
            }
            if payload
                .pending
                .replace(Some(event.event.position))
                .is_none()
            {
                let payload = payload.clone();
                window.on_next_frame(move |_, _| {
                    if let Some(point) = payload.pending.take() {
                        payload.pan_to(point);
                    }
                });
            }
        })
        .on_drop(|drag: &CanvasDrag, window, cx| {
            drag.pending.take();
            drag.pan_to(window.mouse_position());
            cx.stop_propagation();
        })
        .on_drag_move(move |event: &gpui::DragMoveEvent<FrameDrag>, window, cx| {
            let drag = event.drag(cx);
            if drag.pending.replace(Some(event.event.position)).is_none() {
                let drag = drag.clone();
                window.on_next_frame(move |_, _| {
                    if let Some(point) = drag.pending.take() {
                        follow_frame_drag(&drag, point);
                    }
                });
            }
        })
        .on_drop(move |drag: &FrameDrag, window, cx| {
            drag.pending.take();
            follow_frame_drag(drag, window.mouse_position());
            window.refresh();
            cx.stop_propagation();
        })
        .on_drop(move |drag: &PinDrag, window, cx| {
            drop_pin(drag, window.mouse_position());
            window.refresh();
            cx.stop_propagation();
        })
        .on_drop(move |drag: &ResizeDrag, window, cx| {
            let delta = window.mouse_position() - drag.press.get();
            let [width, height] = drag.size.get();
            let dimensions = [
                width + f32::from(delta.x) / drag.scale,
                height + f32::from(delta.y) / drag.scale,
            ]
            .map(|value| value.round().clamp(1., 4096.));
            if dimensions != [width, height] {
                drag.size.set(dimensions);
            }
            window.refresh();
            cx.stop_propagation();
        })
        .on_scroll(move |event, window, cx| {
            let delta = event.delta.pixel_delta(gpui::px(16.));
            let mut x: f32 = delta.x.into();
            let mut y: f32 = delta.y.into();
            if event.modifiers.control || event.modifiers.platform {
                let anchor = event.position - wheel_bounds.bounds().origin;
                scroll.set(PreviewGesture::Zoom {
                    factor: (y / 180.).exp(),
                    anchor: [anchor.x.into(), anchor.y.into()],
                });
            } else {
                if event.modifiers.shift && x == 0. {
                    std::mem::swap(&mut x, &mut y);
                }
                scroll.set(PreviewGesture::Pan([x, y]));
            }
            window.refresh();
            cx.stop_propagation();
        })
        .on_key_down(|event, window, cx| {
            if event.keystroke.key == "escape" && cx.stop_active_drag(window) {
                cx.stop_propagation();
            }
        })
        .on_pinch(move |event, window, cx| {
            let anchor = event.position - bounds.bounds().origin;
            gesture.set(PreviewGesture::Zoom {
                factor: 1. + event.delta,
                anchor: [anchor.x.into(), anchor.y.into()],
            });
            window.refresh();
            cx.stop_propagation();
        })
}

pub(crate) fn layout_outline(mode: LayoutMode, depth: usize, selected: bool) -> impl IntoElement {
    use gpui::{BorderStyle, TextAlign, TextRun, point, px};

    let (label, tint, badge_color) = match mode {
        LayoutMode::Horizontal => ("Row", 0x3b82f6, 0x1e40af),
        LayoutMode::Vertical => ("Column", 0xa855f7, 0x6b21a8),
        LayoutMode::Stack => ("Stack", 0xf59e0b, 0x92400e),
    };
    let label = gpui::SharedString::from(label);
    gpui::canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            let mask = window.content_mask().bounds;
            if !mask.intersects(&bounds)
                || bounds.size.width < px(4.)
                || bounds.size.height < px(4.)
            {
                return;
            }
            let outline = layout_outline_bounds(bounds, depth);
            let color = rgb(tint);
            let (width, stroke, color) = if selected {
                (2., BorderStyle::Solid, color.into())
            } else {
                (1., BorderStyle::Dashed, gpui::rgba((tint << 8) | 0xac))
            };
            window.paint_quad(gpui::quad(
                outline,
                px(0.),
                gpui::transparent_black(),
                px(width),
                color,
                stroke,
            ));

            // Type labels stay legible at canvas zoom and disappear when space is tight.
            if outline.size.width < px(32.) || outline.size.height < px(20.) {
                return;
            }
            let line = window.text_system().shape_line(
                label.clone(),
                px(10.),
                &[TextRun {
                    len: label.len(),
                    font: window.text_style().font(),
                    color: gpui::white(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            let Some(badge) = layout_badge_bounds(bounds, line.width, depth) else {
                return;
            };
            if badge.intersect(&mask) != badge {
                return;
            }
            window.paint_quad(gpui::fill(badge, rgb(badge_color)).corner_radii(px(3.)));
            let _ = line.paint(
                badge.origin + point(px(4.), px(0.)),
                badge.size.height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        },
    )
    .absolute()
    .inset_0()
}

fn layout_outline_bounds(
    bounds: gpui::Bounds<gpui::Pixels>,
    depth: usize,
) -> gpui::Bounds<gpui::Pixels> {
    let max_inset = bounds.size.width.min(bounds.size.height) / 4.;
    bounds.inset(gpui::px((depth.min(8) * 2) as f32).min(max_inset))
}

fn layout_badge_bounds(
    bounds: gpui::Bounds<gpui::Pixels>,
    label_width: gpui::Pixels,
    depth: usize,
) -> Option<gpui::Bounds<gpui::Pixels>> {
    use gpui::{Bounds, point, px, size};
    let margin = px(4.);
    let badge = size(label_width + margin * 2., px(14.));
    if bounds.size.width < badge.width + margin * 2. {
        return None;
    }
    let two_columns = bounds.size.width >= badge.width * 2. + margin * 3.;
    let row = if two_columns { depth / 2 } else { depth };
    let top = bounds.origin.y + margin + (badge.height + px(2.)) * row as f32;
    if top + badge.height + margin > bounds.bottom() {
        return None;
    }

    let left = if two_columns && depth % 2 == 1 {
        bounds.right() - margin - badge.width
    } else {
        bounds.origin.x + margin
    };
    Some(Bounds::new(point(left, top), badge))
}

pub(crate) struct LayoutGuide {
    pub insets: [gpui::Pixels; 4],
    pub alignment: Option<(u8, u8)>,
}

pub(crate) fn insertion_guides(
    layout: Option<LayoutGuide>,
    edge: Option<FlowPlacement>,
    horizontal: bool,
    rtl: bool,
    accent: u32,
) -> impl IntoElement {
    use gpui::{Bounds, Pixels, point, px, size};

    let selector = if layout.is_some() {
        "studio-layout-guide"
    } else {
        "studio-insertion-marker"
    };
    div()
        .absolute()
        .inset_0()
        .debug_selector(move || selector.into())
        .child(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, (), window, _| {
                    let color = rgb(accent);
                    let soft_color = gpui::rgba((accent << 8) | 0xa0);
                    let mut target = bounds;
                    let mut alignment = (u8::from(rtl) * 2, 0);
                    if let Some(layout) = &layout {
                        let [top, right, bottom, left] = layout.insets;
                        let content = Bounds::new(
                            bounds.origin + point(left, top),
                            size(
                                bounds.size.width - left - right,
                                bounds.size.height - top - bottom,
                            ),
                        );
                        if content.size.width >= px(4.) && content.size.height >= px(4.) {
                            target = content;
                        }
                        window.paint_quad(gpui::outline(bounds, color, gpui::BorderStyle::Solid));
                        if target != bounds {
                            window.paint_quad(gpui::outline(
                                target,
                                soft_color,
                                gpui::BorderStyle::Dashed,
                            ));
                        }

                        if let Some((x, y)) = layout.alignment {
                            alignment = (if rtl { 2 - x } else { x }, y);
                        } else if horizontal {
                            alignment.1 = 1;
                        }
                        if horizontal || layout.alignment.is_some() {
                            let y = alignment_coordinate(
                                target.origin.y,
                                target.size.height,
                                alignment.1,
                            );
                            let line = Bounds::new(
                                point(target.origin.x, y),
                                size(target.size.width, px(1.)),
                            );
                            window.paint_quad(gpui::fill(line, soft_color));
                        }
                        if layout.alignment.is_some() {
                            let x = alignment_coordinate(
                                target.origin.x,
                                target.size.width,
                                alignment.0,
                            );
                            let line = Bounds::new(
                                point(x, target.origin.y),
                                size(px(1.), target.size.height),
                            );
                            window.paint_quad(gpui::fill(line, soft_color));
                        }
                    }

                    let stroke = |x, y, w, h| Bounds::new(point(x, y), size(w, h));
                    let paint_marker = |axis: Pixels, window: &mut Window| {
                        let thickness = px(3.);
                        let cap = px(7.);
                        let quads = if horizontal {
                            [
                                stroke(
                                    axis - thickness / 2.,
                                    target.origin.y,
                                    thickness,
                                    target.size.height,
                                ),
                                stroke(axis - cap / 2., target.origin.y, cap, thickness),
                                stroke(
                                    axis - cap / 2.,
                                    target.bottom() - thickness,
                                    cap,
                                    thickness,
                                ),
                            ]
                        } else {
                            [
                                stroke(
                                    target.origin.x,
                                    axis - thickness / 2.,
                                    target.size.width,
                                    thickness,
                                ),
                                stroke(target.origin.x, axis - cap / 2., thickness, cap),
                                stroke(target.right() - thickness, axis - cap / 2., thickness, cap),
                            ]
                        };
                        for bounds in quads {
                            window.paint_quad(gpui::fill(bounds, color));
                        }
                    };
                    let (start, length, align) = if horizontal {
                        (target.origin.x, target.size.width, alignment.0)
                    } else {
                        (target.origin.y, target.size.height, alignment.1)
                    };
                    let axis = match edge {
                        Some(FlowPlacement::Before) if horizontal && rtl => start + length,
                        Some(FlowPlacement::After) if horizontal && rtl => start,
                        Some(FlowPlacement::Before) => start,
                        Some(FlowPlacement::After) => start + length,
                        Some(FlowPlacement::Into) => alignment_coordinate(start, length, align),
                        _ => return,
                    };
                    paint_marker(axis, window);
                },
            )
            .absolute()
            .inset_0(),
        )
}

fn alignment_coordinate(start: gpui::Pixels, length: gpui::Pixels, alignment: u8) -> gpui::Pixels {
    let inset = gpui::px(2.).min(length / 2.);
    start
        + match alignment {
            0 => inset,
            2 => length - inset,
            _ => length / 2.,
        }
}

pub(crate) fn grid(
    color: Option<u32>,
    viewport_size: Option<Binding<[f32; 2]>>,
    transform: Option<[f32; 3]>,
) -> impl IntoElement {
    gpui::canvas(
        move |bounds, _, cx| {
            if let Some(size) = viewport_size {
                let measured = [
                    f32::from(bounds.size.width).floor(),
                    f32::from(bounds.size.height).floor(),
                ];
                if measured != size.get() {
                    cx.defer(move |_| size.set(measured));
                }
            }
        },
        move |bounds, (), window, _| {
            let Some(color) = color else { return };
            let width = f32::from(bounds.size.width).min(4096.);
            let height = f32::from(bounds.size.height).min(4096.);
            let [x, y, scale] = transform.unwrap_or([8., 8., 1.]);
            let step = 16. * scale.max(0.1);
            let step = if step < 16. {
                step * (16. / step).ceil()
            } else {
                step
            };
            let first_x = x.rem_euclid(step);
            let first_y = y.rem_euclid(step);
            for row in 0..((height - first_y).max(0.) / step).ceil() as usize {
                for column in 0..((width - first_x).max(0.) / step).ceil() as usize {
                    let origin = bounds.origin
                        + gpui::point(
                            gpui::px(first_x + column as f32 * step),
                            gpui::px(first_y + row as f32 * step),
                        );
                    window.paint_quad(gpui::fill(
                        gpui::Bounds::new(origin, gpui::size(gpui::px(1.5), gpui::px(1.5))),
                        rgb(color),
                    ));
                }
            }
        },
    )
    .absolute()
    .inset_0()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, point, px, size};

    #[test]
    fn coincident_layout_guides_keep_separate_outlines_and_badges() {
        let bounds = Bounds::new(point(px(24.), px(32.)), size(px(160.), px(60.)));
        let outer_badge = layout_badge_bounds(bounds, px(42.), 0).unwrap();
        let inner_badge = layout_badge_bounds(bounds, px(32.), 1).unwrap();
        let deeper_badge = layout_badge_bounds(bounds, px(42.), 2).unwrap();

        assert_eq!(layout_outline_bounds(bounds, 0), bounds);
        assert_eq!(layout_outline_bounds(bounds, 1), bounds.inset(px(2.)));
        assert!(!outer_badge.intersects(&inner_badge));
        assert!(!outer_badge.intersects(&deeper_badge));
        assert!(!inner_badge.intersects(&deeper_badge));
        assert!(layout_badge_bounds(bounds.inset(px(28.)), px(42.), 0).is_none());
    }
}
