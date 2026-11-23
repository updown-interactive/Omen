//! A native scrollbar for indexed large-document scrolling.
use super::*;
impl TextInput {
    pub(super) fn scroll_to_pointer(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let height = f32::from(bounds.size.height).max(1.);
        let total = f32::from(self.editor_line_height) * self.content.line_count() as f32;
        let thumb = (height * height / total.max(1.)).max(24.).min(height);
        let fraction = ((f32::from(position.y - bounds.top()) - thumb / 2.)
            / (height - thumb).max(1.))
        .clamp(0., 1.);
        self.scroll_y = px(fraction * (total - height).max(0.));
        self.reveal_cursor = false;
        cx.notify();
    }
    pub(super) fn editor_scrollbar(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        self.code_document.as_ref()?;
        let bounds = self.last_bounds?;
        let height = f32::from(bounds.size.height);
        let total = f32::from(self.editor_line_height) * self.content.line_count() as f32;
        if total <= height {
            return None;
        }
        let thumb = (height * height / total).max(24.).min(height);
        let top = (f32::from(self.scroll_y) / (total - height)).clamp(0., 1.) * (height - thumb);
        Some(
            div()
                .id("editor-scrollbar")
                .absolute()
                .right_0()
                .top_0()
                .w(px(12.))
                .h_full()
                .cursor(CursorStyle::Arrow)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|input, event: &MouseDownEvent, _, cx| {
                        input.dismiss_popups();
                        input.is_selecting = false;
                        input.scrollbar_drag = true;
                        input.scroll_to_pointer(event.position, cx);
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_move(cx.listener(|input, event: &MouseMoveEvent, _, cx| {
                    if input.scrollbar_drag && event.dragging() {
                        input.scroll_to_pointer(event.position, cx);
                        cx.stop_propagation();
                    }
                    if !event.dragging() {
                        input.scrollbar_drag = false;
                    }
                }))
                .child(
                    div()
                        .absolute()
                        .top(px(top))
                        .left(px(2.))
                        .w(px(8.))
                        .h(px(thumb))
                        .rounded(px(4.))
                        .bg(self.accent.opacity(0.28)),
                )
                .into_any_element(),
        )
    }
}
