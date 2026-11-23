pub(crate) fn render(name: &str) -> gpui::Svg {
    let bytes: &[u8] = match name {
        "file-plus" => include_bytes!("../assets/icons/file-plus.svg"),
        "folder-plus" => include_bytes!("../assets/icons/folder-plus.svg"),
        "refresh" => include_bytes!("../assets/icons/refresh.svg"),
        "chevron-down" => include_bytes!("../assets/icons/chevron-down.svg"),
        "chevron-left" => include_bytes!("../assets/icons/chevron-left.svg"),
        "arrow-left" => include_bytes!("../assets/icons/arrow-left.svg"),
        "arrow-right" => include_bytes!("../assets/icons/arrow-right.svg"),
        "chevron-right" => include_bytes!("../assets/icons/chevron-right.svg"),
        "folder" => include_bytes!("../assets/icons/folder.svg"),
        "folder-open" => include_bytes!("../assets/icons/folder-open.svg"),
        "file" => include_bytes!("../assets/icons/file.svg"),
        "files" => include_bytes!("../assets/icons/files.svg"),
        "git-branch" | "source-control" => include_bytes!("../assets/icons/git-branch.svg"),
        "file-code" => include_bytes!("../assets/icons/file-code.svg"),
        "file-config" => include_bytes!("../assets/icons/file-config.svg"),
        "file-image" => include_bytes!("../assets/icons/file-image.svg"),
        "file-omen-view" => include_bytes!("../assets/icons/file-omen-view.svg"),
        "file-omen-state" => include_bytes!("../assets/icons/file-omen-state.svg"),
        "file-omen-env" => include_bytes!("../assets/icons/file-omen-env.svg"),
        "file-omen-locale" => include_bytes!("../assets/icons/file-omen-locale.svg"),
        "file-rust-module" => include_bytes!("../assets/icons/file-rust-module.svg"),
        "file-rust" => include_bytes!("../assets/icons/file-rust.svg"),
        "agent-chat" => include_bytes!("../assets/icons/agent-chat.svg"),
        "check" => include_bytes!("../assets/icons/check.svg"),
        "close" => include_bytes!("../assets/icons/close.svg"),
        "plus" => include_bytes!("../assets/icons/plus.svg"),
        "search" => include_bytes!("../assets/icons/search.svg"),
        "spanner" | "wrench" => include_bytes!("../assets/icons/spanner.svg"),
        "settings" => include_bytes!("../assets/icons/settings.svg"),
        "info" => include_bytes!("../assets/icons/info.svg"),
        path => return gpui::svg().external_path(path.to_string()),
    };
    gpui::svg().data(bytes)
}

#[derive(gpui::IntoElement)]
pub(crate) struct IconElement {
    pub source: String,
}
impl gpui::RenderOnce for IconElement {
    fn render(self, window: &mut gpui::Window, _cx: &mut gpui::App) -> impl gpui::IntoElement {
        use gpui::prelude::*;
        render(&self.source)
            .size_full()
            .text_color(window.text_style().color)
    }
}
