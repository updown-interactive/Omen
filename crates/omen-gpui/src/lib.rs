//! GPUI desktop rendering adapter.
use gpui::{
    App, Bounds, Context, IntoElement, Render, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};

pub struct OmenWindow;

impl Render for OmenWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(rgb(0x101827))
            .text_color(rgb(0xffffff))
            .child(div().text_3xl().child("Welcome to Omen"))
            .child(div().text_sm().child("A Rust-first UI framework"))
    }
}

/// Open Omen's first desktop window and run the platform event loop.
pub fn run() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(800.), px(520.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Omen".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| OmenWindow),
        )
        .expect("Failed to open Omen window");
        cx.activate(true);
    });
}
