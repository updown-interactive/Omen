//! Native activity bar and persistent sidebar selection.
use super::{agent::AgentChat, explorer::ProjectExplorer};
use crate::{
    app::StudioEnvironment,
    state::{GitChange, SearchMatch, SidebarPane},
    workspace::Command,
};
use omen::view;

pub(super) const ACTIVITY_BAR_WIDTH: f32 = 44.;

view! {
    pub(super) ActivityBar {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let layout = app.layout;
        Layout {
            for pane in SidebarPane::ALL {
                ActivityButton(pane: pane, selected: layout.sidebar_visible && layout.active_sidebar == pane)
            }
            Layout {}.fill()
        }
        .spacing(5)
        .padding(5)
        .frame(ACTIVITY_BAR_WIDTH, .infinity)
        .background(colors.surface)
    }
}
view! {
    ActivityButton(pane: SidebarPane, selected: bool) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Layout { Icon(pane.icon()).frame(18,18) }
            .frame(34,34)
            .alignment(.center)
            .cornerRadius(7)
            .background(if selected { colors.selection } else { colors.surface })
            .color(if selected { colors.accent_foreground } else { colors.on_surface_variant })
            .hoverBackground(colors.surface_container_high)
            .onTap(move || app.select_sidebar(pane))
            .accessibilityRole(.button)
            .accessibilityLabel(format!("Show {}",pane.title()))
            .accessibilityHint("Select sidebar pane")
    }
}
view! {
    pub(super) Sidebar {
        @Environment app: StudioEnvironment
        let layout = app.layout;
        let root = (app.workspace).root;
        if layout.active_sidebar == SidebarPane::Explorer { ProjectExplorer() }
        else if layout.active_sidebar == SidebarPane::Search { ProjectSearch()
            .key(format!("search:{root:?}")) }
        else if layout.active_sidebar == SidebarPane::Agent { AgentChat() }
        else { SourceControl()
            .key(format!("source-control:{root:?}")) }
    }
}
view! {
    SearchRunner {
        @Environment app: StudioEnvironment
        let runner = app.clone();
        Layout {}.frame(0, 0).task(move || {
            let app = runner.clone();
            async move { app.search_project().await; }
        })
    }

    ProjectSearch {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let root = (app.workspace).root;
        let result = app.search_results;
        let selected_index = app.search_index;
        let current = result.root == root && result.query == app.search_query && result.case_sensitive == app.search_case_sensitive;
        let nonce = app.search_nonce;
        Layout {
            if nonce > 0 {
                SearchRunner().key(format!("search-run:{nonce}"))
            }
            Text("Search")
                .fontSize(13)
                .weight(.semibold)
                .padding(6)
            Layout { TextField("Search in project")
                    .value($app.search_query)
                    .fontSize(12)
                    .frame(.infinity,32)
                    .background(colors.surface)
                    .cornerRadius(5) }
            .padding(1)
            .background(colors.outline)
            .cornerRadius(6)
            .shortcut("enter", move || app.trigger_project_search())
            Checkbox("Match case", $app.search_case_sensitive)
                .fontSize(11)
            Button(if app.search_busy { "Searching…" } else { "Search files" }) async { app.search_project().await; }
            .frame(.infinity,30)
            .background(colors.primary)
            .color(colors.on_primary)
            .cornerRadius(5)
            if root.is_none() { Text("Open a project to search its files.")
                    .font(.caption) }
            else if current {
                if !result.message.is_empty() { Text(result.message)
                            .font(.caption)
                            .color(colors.on_surface_variant) }
                Text(format!("{} matching lines{}", result.matches.len(), if result.truncated { " · limit reached" } else { "" }))
                    .font(.caption)
                    .color(colors.on_surface_variant)
                List(
                    result.matches.iter().cloned().enumerate().collect::<Vec<_>>(),
                    id: |(index, item)| format!("{index}:{}:{}",item.path.display(),item.line)
                ) { (index, item) in SearchResultRow(item: item, selected: index == selected_index) }
                .virtualized(54.)
                .fill()
                .frame(.infinity,.infinity)
            } else { Text("Enter text and press Enter or Search files.")
                        .font(.caption)
                        .color(colors.on_surface_variant) }
        }
        .padding(10)
        .spacing(10)
        .fill()
        .frame(.infinity,.infinity)
        .focusScope(true)
        .background(colors.surface_container)
        .color(colors.on_surface)
        .shortcut("down", move || app.search_move(1))
        .shortcut("up", move || app.search_move(-1))
        .shortcut("enter", move || app.activate_search_result())
    }
}
view! {
    SearchResultRow(item: SearchMatch, selected: bool) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let root = (app.workspace).root;
        let path = root.as_ref().and_then(|root| item.path.strip_prefix(root).ok()).unwrap_or(&item.path).display().to_string();
        let accessible = format!("Open {} line {}",path,item.line+1);
        Layout {
            Text(format!("{}:{}",path,item.line+1))
                .fontSize(11)
                .color(colors.accent_foreground)
                .truncate()
            Text(item.preview.clone())
                .fontSize(11)
                .color(colors.on_surface_variant)
                .truncate()
        }
        .spacing(3)
        .padding(5)
        .frame(.infinity,54)
        .background(if selected { colors.selection } else { colors.surface_container })
        .hoverBackground(colors.selection)
        .onTap(move || app.open_search_result(item.clone()))
        .accessibilityRole(.button)
        .accessibilityLabel(accessible)
    }
}
view! {
    SourceControl {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let root = (app.workspace).root;
        let state = app.source_control;
        let current = state.root == root;
        let staged_changes = state.changes.iter().filter(|change| change.staged()).cloned().collect::<Vec<_>>();
        let unstaged_changes = state.changes.iter().filter(|change| change.unstaged()).cloned().collect::<Vec<_>>();
        Layout {
            Layout {
                Text("Source Control")
                    .fontSize(13)
                    .weight(.semibold).fill()
                Button("Refresh") async { app.refresh_source_control().await; }
                .fontSize(11)
                .color(colors.accent_foreground)
            }
            .mode(.horizontal)
            .spacing(6)
            if app.source_control_busy { Text("Reading Git status…")
                    .font(.caption) }
            if current {
                if !state.message.is_empty() { Text(state.message)
                        .font(.caption)
                        .color(colors.on_surface_variant) }
                if state.repository {
                    Text(format!("Branch: {}",state.branch))
                        .fontSize(12)
                        .color(colors.accent_foreground)
                    Text(format!("Staged changes ({})",staged_changes.len()))
                        .fontSize(11)
                        .weight(.semibold)
                    if !staged_changes.is_empty() {
                        List(
                            staged_changes,
                            id: |change| change.path.display().to_string()
                        ) { change in GitChangeRow(
                                change: change,
                                staged: true
                            ) }
                        .virtualized(36.)
                        .fill()
                        .frame(.infinity,.infinity)
                    }
                    Text(format!("Changes ({})",unstaged_changes.len()))
                        .fontSize(11)
                        .weight(.semibold)
                    if !unstaged_changes.is_empty() {
                        List(
                            unstaged_changes,
                            id: |change| change.path.display().to_string()
                        ) { change in GitChangeRow(
                                change: change,
                                staged: false
                            ) }
                        .virtualized(36.)
                        .fill()
                        .frame(.infinity,.infinity)
                    }
                    if state.changes.is_empty() { Text("Working tree clean")
                            .font(.caption)
                            .color(colors.on_surface_variant) }
                }
            } else { Text("Choose Refresh to read Git status.")
                    .font(.caption)
                    .color(colors.on_surface_variant) }
        }
        .padding(10)
        .spacing(12)
        .fill()
        .frame(.infinity,.infinity)
        .background(colors.surface_container)
        .color(colors.on_surface)
        .task(move ||app.source_control_task())
    }
}
view! {
    GitChangeRow(change: GitChange, staged: bool) {
        @Environment app: StudioEnvironment
        @State change_action = change.clone()
        let colors = app.colors();
        let path = change.path.clone();
        let name = change.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        Layout {
            Button(format!("{} {}",change.status(),name)) { app.send(Command::OpenFile(path.clone())); }
            .fontSize(11)
            .fill()
            .alignment(.leading)
            .accessibilityLabel(format!("Open changed file {}",change.path.display()))
            Button(if staged { "−" } else { "+" }) async { app.stage_change(change_action.clone(),!staged).await; }
            .frame(26,28)
            .fontSize(16)
            .color(colors.accent_foreground)
            .accessibilityLabel(if staged { "Unstage file" } else { "Stage file" })
        }
        .mode(.horizontal)
        .spacing(4)
        .frame(.infinity,36)
        .hoverBackground(colors.selection)
    }
}
