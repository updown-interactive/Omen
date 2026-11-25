# Omen Studio

A native Omen/GPUI workspace application. Studio stays in the existing Cargo
workspace and depends on the local `omen` facade. No additional UI framework,
WebView, or duplicate project generator is used.

Run from the repository root:

```sh
cargo run -p omen-studio
```

Or use the existing CLI:

```sh
cargo run -p omen-cli -- --project studio run --platform desktop
```

The `native` feature supports the existing desktop packager's invocation.
`Omen.toml` uses the relative SDK path `..`. Studio is currently a desktop tool;
its manifest's mobile/web renderer declarations do not imply Studio support for
those targets.

Studio’s `Omen.toml` points to `studio/assets/logo.png`. The desktop packager
uses it as the app icon automatically. From the Studio project directory, run
`omen generate-icon --platform desktop` to generate the desktop assets separately.

## Startup and onboarding

Studio restores the last open project directly into the editor. It shows only a
neutral loading screen while the background service restores the session; it does
not open onboarding behind the editor. With no saved open project, or when its
folder is unavailable, the primary window shows the panel-free onboarding screen
with create/open actions, recent projects and appearance settings.

**File → Close project** returns to onboarding and persists the closed state, so
the next launch stays on onboarding. Recent projects remain available. Modified
or conflicted documents must be saved/resolved before closing the project. Quitting
Studio with a project still open preserves that project, tabs and layout for the
next launch. Opening a project switches the same primary window to the editor.

## Working with projects

The native title bar contains **File** and **Window** dropdown menus. **File**
provides **New project**, **Open project**, **Save**, and **Save all**. **Window**
provides checked **Sidebar** and **Inspector** visibility toggles. Clicking outside,
pressing Escape, or choosing an action dismisses a dropdown. Only one menu opens
at a time. The old two-row workspace toolbar and path inputs have been removed.

Choose **File → Open project** to select an existing folder with the native picker,
or **File → New project** to create an Omen application. The welcome page also
keeps its create/open shortcuts.
The creation dialog provides bordered **Project name**, **Application ID**, and
**Location** fields. Use **Choose folder** to select a parent directory; the final
project location appears beneath it. Choose Desktop, Android, iOS, and/or Web,
then press the accent-colored **Create project** button. Desktop is selected
initially. Renaming a project updates its default identity and window title while
preserving an application ID you have customized.

The dialog generates `Omen.toml` from the form; it no longer exposes a raw TOML
editor. Invalid names, identifiers, or an empty target list keep the dialog open
without creating files. Generation runs on the workspace worker through the CLI's
shared project generator. Existing directories are never overwritten. Advanced
configuration can be edited in the generated project's `Omen.toml` afterward.
Web-only starters default to the web backend.
`OMEN_SDK` can override the checkout used to generate project dependencies.

The explorer uses left-aligned rows with SVG file-type icons, disclosure chevrons,
16-pixel nesting guides, and full-width selection highlights. Expand directories
and select files to open tabs. Selecting a folder keeps the current editor tab open. The native
plain-text editor supports multiline Unicode input, IME ranges, selection,
clipboard operations, Enter, Tab indentation, and bounded undo/redo history.
Choose **File → Save**, **File → Save all**, or Cmd-S / Ctrl-S. Select a tab to switch documents;
its close button refuses to discard modified text. The tab strip scrolls horizontally
when more tabs are open than fit on screen. Tab identity is keyed by file path.
Right-click tree entries for
an Omen context popover. The **Explorer** project row has icon actions for
**New file**, **New folder**, and **Refresh explorer**. New entries are created
in the project root through a name dialog; files open in an editor tab immediately.
Existing entries are never overwritten. The project chevron collapses or expands
the visible tree, and Refresh explorer rescans expanded folders.

Drag the split handles to resize panels, or focus a handle with Tab and use the
arrow keys. **Window → Sidebar** (Cmd-B / Ctrl-B) and **Window → Inspector** toggle panels. **Settings → Appearance**
switches the inherited Omen theme. Preferred widths are retained while temporary
viewport constraints reserve at least 320 logical pixels for the editor at supported
window sizes. Resizing starts from the currently displayed width, including when
saved preferences were constrained by a narrower window.

Recent projects are real stored paths, with up to 12 entries. Missing folders are
removed on restore and during polling. An unavailable restored root returns to
home while retaining document text for recovery. A stale entry clicked before the
next poll is removed and reports how to select another folder.

External file changes are polled on the worker every 750 ms. Clean documents
reload automatically. Modified documents retain edits and show a conflict;
normal saves refuse to silently overwrite changed or deleted files. Editor requests
carry their displayed disk revision and baseline, so a watcher reload racing a queued edit
cannot silently authorize overwriting the external change. Users can choose
**Overwrite disk** to force save the editor buffer over external changes, or
**Reload from disk…** (with confirmation) to discard editor edits. Save
modified documents before switching projects.

## Settings

Choose **Settings** in the native top bar of the welcome or editor window.
Use the sidebar to switch sections.

- **Appearance** controls light or dark mode and the Default, Midnight, or Graphite theme.
- **Editor** controls Auto Save and code typography. Auto Save can stay **Off**,
  save after an adjustable idle delay, save a file when you leave its editor,
  or save all edited files when the Studio window loses focus. Auto Save uses
  the same conflict checks as manual saves.
- **Design** controls the default editor mode for `@Design` files, the initial
  frame profile, and canvas grid visibility. Saved per-file canvas settings
  continue to supply the mode and frames for that file.

- **Account** saves a local name and email in `account.json`. Studio includes
  the name, file name, application name, and UTC creation time in headers for
  newly created project files that support comments. The email stays in the
  account file.

Appearance, Editor, and Design changes apply immediately and save automatically.
Use **Save Account** to save the account profile. **Done**, Escape, or the
backdrop closes the dialog.

Studio creates `settings.json` in its profile directory on first launch:

```json
{
  "appearance": {
    "theme": "xcode",
    "mode": "dark"
  },
  "editor": {
    "font_size": 13,
    "line_height": 20,
    "letter_spacing": 0.5,
    "auto_save": "off",
    "auto_save_delay_ms": 1000
  },
  "design": {
    "initial_mode": "Design",
    "frame_profile": "IPhone",
    "show_grid": true
  }
}
```

The file loads at startup; restart Studio after manually editing it. Missing
properties use defaults. Existing appearance values from `layout.json` migrate
once when no settings file exists. `settings.json` then takes precedence. Invalid
JSON or unsupported values produce a recovery message and use defaults in memory;
the original file is preserved until an explicit settings change replaces it.

The Xcode-inspired treatment uses compact typography, a neutral editor canvas,
layered navigator/inspector and toolbar surfaces, subtle separators, small corner
radii and blue action accents. Midnight adds cool blue surfaces; Graphite uses
neutral and warm gray surfaces. These are custom Omen palettes, not imported Apple
assets or exact copies of Xcode's themes.

Views share color roles through `StudioEnvironment::colors()`:

- `surface`, `on_surface` (`onSurface`)
- `surface_container` (`surfaceContainer`)
- `surface_container_high` (`surfaceContainerHigh`)
- `primary`, `secondary`
- `on_surface_variant`, `outline`, `selection`, `on_selection`

For example, a view can use `.background(colors.surface_container)` and
`.color(colors.on_surface)`. Studio's `primary` and `secondary` roles are accents;
the adapter to Omen's existing design tokens keeps its legacy foreground-text
meaning of `Color::Primary` and `Color::Secondary`. Native text input caret,
selection, focus rings, checkboxes and toggles use the resolved theme accent.

## Persistence

Application settings, layout, and project state use separate JSON files:

- `settings.json`: application appearance, editor, and design preferences.
- `account.json`: local name and email for Studio’s account profile.
- `layout.json`: panel visibility and preferred panel widths.
- `workspace.json`: last project, recent projects, expanded directories, tabs,
  active document, baseline disk contents and unsaved buffers.

They live under `~/Library/Application Support/Omen/Studio` on macOS,
`%LOCALAPPDATA%/Omen/Studio` on Windows, and `$XDG_STATE_HOME/omen-studio` (or
`~/.local/state/omen-studio`) on Linux. Set `OMEN_STUDIO_DATA_DIR` for an isolated
profile. Backups debounce for 250 ms, flush at least every two seconds while
editing, and flush queued work during window teardown. The service drains queued requests
before its final owner is dropped, even if its UI event stream has closed. Disk errors appear in
the status bar. Sudden process termination before a backup completes can lose
the most recent changes; explicit saves write the actual project files.

All directory reads, document reads/writes, backups, change polling and generation
run on a dedicated worker. Main-thread state uses Omen `Value`/`Binding` and
reactive environment injection. Mounted event subscriptions retain their stream
across background cancellation and resume queued events on foregrounding.

## Workspace service

`workspace::Workspace` is the UI-independent filesystem/document engine;
`workspace::service::WorkspaceService` owns its background thread. UI actions
submit `workspace::Command` values (`OpenProject`, `Create`, `OpenFile`,
`ToggleDirectory`, `Select`, `EditFromSnapshot`, `Save`, `SaveAll`, `Reload`,
`Close`, `NewEntry`, `Refresh`) and consume state snapshots from the service event stream.
The suggested open/create/save/watch APIs are represented by these ordered
commands and automatic polling; no view performs filesystem traversal.
`WorkspaceService::read_file(path).await` also supports bounded async disk reads
without a UI subscription. A project switch commits only after the replacement
root's tree has been read successfully; failed opens retain the current tabs.

## Source layout

- `app.rs`: reactive environment, commands and event delivery.
- `state/`: documents, tree rows, workspace state.
- `theme/`: layout preferences and semantic theme selection.
- `workspace/`: UI-independent filesystem engine and worker service.
- `services/`: platform data paths and atomic persistence.
- `views/`: shell, toolbar, explorer, preview, status and dialogs.
- `editor/`: tabs, welcome view and native text editor composition.
- `tests/`: filesystem, recovery, conflict and declarative shell integration tests.

Framework additions are native multiline `TextEditor`, `.virtualized(row_height)`
for lists, `.resizeHandle(...)`, `.onContextMenu(...)`, `.shortcut(...)`, and
`.desktopWindow(...)`, `.onWindowClose(...)`, `.minWidth(...)` and
`.scrollableHorizontal()`. These extend Omen's existing nodes and GPUI adapter.
Presentation wrappers propagate viewport height without creating a second layout
system. A DSL fix keeps ordinary captured values reusable in repeatable callbacks.

## Current bounds and validation

Files must be UTF-8, contain no NUL bytes, and be at most 16 MiB. Up to 32 documents
and 20,000 expanded tree rows are supported. Directory traversal is lazy by
expansion; symlinks, `target`, `.git`, and `node_modules` are skipped. Tree row
models and Omen recipes are retained, while GPUI creates native elements only for
visible rows. The editor shapes only visible lines and caches their layouts. Soft wrapping,
multi-cursor editing and debugging remain future work.

```sh
cargo test -p omen-studio -p omen-gpui -p omen-ui -p omen-macro-core -p omen-web -p omen-cli --locked
cargo test -p omen-macros --locked
```

Tests cover Unicode edits and clipboard newlines, undo/redo, pointer capture,
keyboard resizing, visible-row virtualization, separate layout/session recovery,
external-change protection, CLI project generation, reactive shell editing/saving,
and mounted event resumption after lifecycle cancellation.

Validation on this macOS host: the targeted framework/Studio/CLI suite and macro
compile-pass/compile-fail fixtures passed; Studio's strict Clippy check with
`--no-deps` and formatting passed. Cargo native builds and the CLI desktop
packager succeeded. The native welcome screen and full-height shell were visually
verified. Desktop automation lost reliable input access, so native editing and
pointer behavior are covered by GPUI tests and the create/open/edit/save/recovery
flow by workspace and declarative-shell integration tests. Windows/Linux runtime
qualification remains pending. The broader strict Clippy check still reports
pre-existing lints in macro code generation and CLI control flow.

## Code editor

Studio uses Omen's native `TextEditor.document(EditorDocument)` control. Persistent
Ropey snapshots back edits and undo history; keystrokes do not clone the whole
document. Selection uses Unicode graphemes and GPUI's native UTF-16/IME interface.
Line numbers, indentation, auto-indent, bracket matching, syntax colors and
diagnostic underlines are rendered with the visible lines. Find/replace scans
run on workers and discard stale results.

Right-click the document or a selection to open editor commands. Completion,
hover and quick-fix lists use floating native popups; no action strip occupies
the document. Keyboard commands include Command/Control-F for find,
Control-Space for completion, F12 for definition, Command/Control-I for hover,
and Alt-Shift-F for formatting. Completion uses Up/Down, Enter or Tab; Escape
dismisses it. Tab/Shift-Tab indent/outdent selections. Standard native clipboard
and undo/redo shortcuts apply.

Install `rustup component add rust-analyzer rust-src` for Rust language support.
Studio finds `RUST_ANALYZER`, `rust-analyzer` on PATH, or `rustup which rust-analyzer`.
A background LSP actor starts for Rust files in Cargo projects, debounces document
updates, cancels superseded queries, and rejects stale responses and edits. It
supplies completion, hover, definitions, diagnostics, semantic highlighting,
formatting, rename and edit-based code actions. Edits remain unsaved for review.
Code actions requiring server commands or file creation/renaming are unsupported.
Build scripts, proc-macro execution, background cargo checks and cache priming
are disabled in this first pass; compiler/build diagnostics and full macro
expansion require later build integration. Omen's compiler vocabulary supplies
component and directive completions inside `view!` bodies.

The native input adaptation retains its Apache-2.0 license. Ropey is MIT-licensed;
Studio does not depend on Zed's GPL editor/rope crates.

Live language-server tests require an installed analyzer and Rust sources:

```sh
cargo test -p omen-studio --test language real_rust_analyzer -- --ignored
```

Automated checks cover a 100,000-line viewport, Unicode/IME composition, persistent
undo/redo, completions, find/replace, stale and project-confined language edits,
and real rust-analyzer diagnostics, completion, hover, definition, formatting,
rename, semantic tokens and action responses. This editor revision has not had
a full native visual or Windows/Linux runtime qualification.

The native editor now supports double-click word selection, triple-click/gutter
line selection, word navigation, Home/End and page movement, bracket pairing and
wrapping, paired deletion, and indented Enter between brackets. Use F2 to rename,
Command/Control-. for quick fixes, Command/Control-/ for line comments, Alt-Up/Down
to move selected lines, and Alt-Shift-Up/Down to duplicate them. Copy/cut with no
selection operates on the current line. Hover pauses for 450 ms before querying
the language server. A vertical scrollbar navigates large files.

See [the Zed architecture review and implementation scope](../docs/studio-editor-architecture.md).

Format Document (right-click → Format Document or Alt-Shift-F) now also formats
Omen `view!` and `main!` modifier chains after Rust formatting. View modifiers
use separate lines; modifiers on childless controls indent four spaces, while
container modifiers align with the closing brace. A trailing `.fill()` stays
compact with the preceding modifier. The compiler-owned formatter preserves
comments, literals, argument expressions and CRLF line endings, and leaves
incomplete DSL bodies unchanged. Omen formatting also works without a Cargo
project or when rust-analyzer formatting is unavailable.

## Explorer context actions

Right-click a file, folder or project header to open a native pointer-anchored
menu. Files can be opened and folders expanded/collapsed. New file/folder actions
use the clicked folder (or a clicked file's parent). Rename supports F2 and Enter
in the form and updates paths for open tabs, unsaved buffers and expanded folders.
Cut/copy/paste supports files and recursive folders within the open project; copies
receive an available “copy” name instead of overwriting an existing item. Copy path
and Copy relative path write to the system clipboard. Reveal opens the system
file manager. Refresh rereads the explorer.

Move to Trash asks for confirmation, uses the platform Trash/Recycle Bin and
closes affected clean tabs only after success. Dirty/conflicted files block moving
and deletion. Filesystem actions run on the workspace actor. The project root,
outside paths, symbolic links and recursive paste-into-self operations are rejected.
Folder copies are limited to 20,000 entries and reject special filesystem entries.
Menus dismiss outside or with Escape and provide hover feedback and keyboard focus.

These controls use the reusable Omen `context_menu`, `desktop_action` (clipboard
and reveal) and `hover_background` APIs. Windows/Linux runtime validation and a
full native visual pass remain pending.

## Omen-aware creation and modules

Explorer context menus and the command palette (File → Command palette, or
Command/Control-Shift-P) offer New Omen View, New Omen State, New Environment,
New Localization, New Rust Module, New Folder and New Rust File. Search the
palette and use arrows/Enter or click a command. Enter a name; the dialog previews
the normalized filename and resolved module directory. `Dashboard` as a View
creates `dashboard_view.rs` with a public `Dashboard` Omen view. Templates live in
`studio/templates/` and compile against this checkout's actual macros.

The centralized `StudioFileKind` registry recognizes exact `_view.rs`, `_state.rs`,
`_env.rs`, `_locale.rs` suffixes and `mod.rs`. Each has a distinct SVG silhouette
and light/dark tint, independent of filename color. Other Rust files retain the
Rust icon; folders retain disclosure/expanded icons.

Registration is offered for Rust source creation and can be disabled for files
that are not in a declared module directory. New Rust Module requires a resolved
parent, creates `name/mod.rs`, registers it, expands the tree and opens the file.
The resolver follows Cargo target paths, crate roots, legacy `foo/mod.rs`, modern
`foo.rs` and inline modules. Multiple possible parents are listed explicitly for
selection. An unregistered directory, invalid source or ambiguous module layout
produces a diagnostic instead of a guessed parent. Conditional/`#[path]` modules
require manual declaration updates; automatic updates are disabled for them.

Module insertion uses the Rust syntax tree, avoids duplicates, follows existing
visibility conventions and preserves source text/comments and line endings.
Parent files must be saved and writable before automatic edits. New files are
created exclusively; failed parent writes roll back the new files/directories.
Destination paths and symbolic links are checked against the open project.

Rename and Trash dialogs inspect affected declarations and offer optional safe
updates. Modern module renames also move their companion file/directory and update
open document paths. Deletion removes only declarations referring into the deleted
item. Parent edits roll back if the filesystem operation fails. Imports and symbol
references are not renamed; use rust-analyzer's Rename Symbol for semantic changes.

Trash discovery tolerates unfinished source in unrelated files and in the file being
deleted. Only parent files actually being edited must parse successfully. If safe
declaration cleanup is unavailable, the dialog hides that option and still allows
moving the item to Trash, with a reminder to update declarations manually. Unsaved
document, project-root and filesystem protections still apply.

Tests cover classification/icons, naming/templates, root ambiguity, nested/inline/
modern/legacy modules, duplicate declarations, read-only/dirty parents, symlink
escapes, failure rollback, palette/dialog integration and `cargo check` of a generated
project containing all starter types. Native visual and Windows/Linux runtime
qualification remain pending.

Explorer creation is grouped under a single **New file…** menu action. It and
the explorer new-file icon open a searchable chooser showing each supported type
with its own icon and description. Pick a type, enter a name in its creation form,
and press Create. The chooser also supports arrows, Enter and Escape.

File creation does not require the project to compile or every source file to parse.
During parent discovery, complete Rust items before an unfinished item can still
provide module declarations. This recovery is bounded and does not extract fake
declarations from strings, comments or incomplete macro bodies. The selected parent
file is validated strictly before inserting a declaration; ordinary file creation
remains available with registration disabled.

## Omen application structure

Studio uses the same Omen APIs as the applications it creates:

- `main!` injects the `environment!`-declared `StudioEnvironment` into the native desktop window and its title bar.
- `@Reactive` fields own shared UI values. Service handles, event receivers, editor caches and cancellation bookkeeping remain ordinary Rust resources.
- `state!` declares workspace/document-list state, explorer drafts, panel preferences and view-owned toolbar state. Persisted state keeps its existing JSON schemas; document buffers and filesystem DTOs remain ordinary Rust types.
- `view!` declares the shell, onboarding, editor, explorer, menus, forms, dialogs and native toolbar. Views use natural field reads and `$app.field` bindings; custom bindings remain where validation or persistence must accompany a write.
- `locale!` declares typed UI copy in `src/strings.rs`. Views retain it with `@Localize`; the current catalog is English.

`src/app.rs` declares environment ownership and bootstrap. Feature methods live under `src/app/`: session lifecycle, project creation, explorer actions, document bindings, preferences and editor/language integration. Filesystem and language-server work stays in background services. `StudioEnvironment::launch()` starts the application; `at(directory)` supplies an isolated data directory for tests.

`StudioLauncher` owns the event task while routing between onboarding and `StudioShell`. A standalone `Studio` owns that lifecycle itself. The shell does not start a second receiver when a project opens. Local toolbar and settings state stays scoped to its view; layout, application settings and project sessions remain separate persistence concerns.

## Activity bar, project search and source control

The left activity bar selects **Explorer**, **Search**, or **Source Control** while
keeping the code editor open in the center. Explorer is the default. The selected
pane is stored in layout preferences, and choosing an activity reopens a hidden
sidebar. Window → Sidebar and the existing sidebar shortcut control its visibility.
Use Command/Ctrl+Shift+E, +Shift+F, or +Shift+G to select a pane.

Search provides literal text search with an optional **Match case** setting. Choose
**Search files** to search the open project, including unsaved buffers for files on
disk. Results are virtualized and open at the matching line and column. Search skips
symlinks, binary files, files over 2 MiB, and build/VCS directories. It reports when
its 1,000 matching-line or traversal limits are reached; it does not interpret regex
patterns or `.gitignore` rules.

Source Control reads the enclosing Git repository, restricted to the opened project,
and displays the current branch, staged changes and working-tree changes. Select a
file to open it, use **+** to stage or **−** to unstage it, and choose **Refresh** to
read external Git changes. Staging and unstaging support spaces in filenames and
renames, including repositories without a first commit. A project outside Git shows
an explanatory empty state. Git must be available on PATH. Commit, diff, branch
management and remote operations are not part of this initial pane.

Search and Git commands run on background threads. Results from an old project are
ignored; canceling a pane task clears its busy state. Pane selection, project state
and unsaved editor buffers have independent ownership.

## Omen component snippets

Omen suggestions insert complete DSL snippets rather than bare element names:
`Text("Text")`, `Button("Button") { ... }`, form controls with `.value($text)` or
`.checked($enabled)`, list rows, and navigation/container children. All compiler
built-ins are covered, including `TextEditor` and argument-free `Divider()`.

Accept with Enter, Tab, or a suggestion click. The first default is selected;
Tab advances, Shift-Tab goes back, and the last Tab moves to the final caret.
Escape exits snippet navigation. Edits inside placeholders update subsequent
positions, including Unicode and IME edits. Undo/redo or an external reload ends
navigation. Nested and mirrored TextMate/LSP snippets are not implemented.

Studio also discovers `view!` signatures in project Rust files and declared local path dependencies and inserts their
named properties with defaults appropriate to the declared type. Unknown property
types use an editable `todo!()` expression; imports and state declarations still
need to exist in the calling module. The project catalog refreshes at most every
two seconds and skips generated/VCS directories, symlinks and large source files.
`@State`, `@Environment`, and `@Localize` have declaration snippets as well.
Existing argument/child blocks are preserved when completing their element name.


## Code Inspector

Select a component in a Design frame or place the caret inside `view!` in Code
to see its breadcrumb, constructor
properties, and existing modifier values. Literal text/numbers and compiler tokens
can be edited; changes patch only the selected source range and use the normal
unsaved-document, undo, save and external-change pipeline.

The **+ Modifier** search lists real Omen APIs supported by the selected component.
Add or remove layout, typography, appearance, sizing and scrolling modifiers. Layout
orientation maps to `.mode(...)`; available sizing maps to `.frame(...)` and `.fill()`.
Token choices come from the compiler's registry. Existing Rust expressions and
bindings are labeled **Bound**: use **Go to source** or check **Replace binding with literal** before replacing it. The inspector never evaluates Rust or silently hardcodes a binding.

Analysis runs on a latest-selection worker. Every write checks the active document,
text snapshot and selection revision; stale controls cannot patch another version.
The reusable source model and patch API live in `omen_macro_core::inspector`, sharing
the existing compiler AST. Invalid/incomplete macro syntax temporarily disables
inspection instead of guessing ranges. Legacy `preview_visible` / `preview_width`
layout settings are retained for compatibility and now control the inspector.

### Visual editor

Any open source file with `@Design` on a named `view!` declaration supports
**Design** and **Split**. **Code** uses the existing Rust editor, and **Split**
shows the canvas beside it. Cmd/Ctrl+Alt+1, +2, and +3 switch modes.
All modes share the same unsaved text, save pipeline, and external-change checks.
Put `@Design` before a named view to make it the default when its file opens.
The frame shows `@Design` and the view name. A `view!` block can contain
multiple declarations, and Studio can render static, parameterless helpers
from the same file. In Split, moving the Code cursor into another declaration
switches the frames to that view. Hand edits update the preview in the background as soon as the
declaration is valid. While an edit is incomplete, Studio shows a diagnostic and
keeps the previous preview where available.

Use **+ Frame** for phone, tablet, desktop, or browser viewport profiles.
Drag the empty canvas or scroll to pan in any direction. Drag a frame by its
header to reposition it. Pinch or use Cmd/Ctrl+scroll to zoom around the pointer.
Surface zoom scales the grid and each entire frame together. Each simulator
retains its logical viewport dimensions for layout.
The toolbar zoom controls zoom around the viewport center. **Fit** brings all
frames back into view, and **Center** focuses the selected frame. Fit follows the
canvas size until you pan, zoom, or drag a frame. The canvas clips offscreen
artwork while you navigate.
The frame menu provides profile changes, dimensions, name, rotation, appearance,
locale, safe-area guides, duplication, and removal. These settings live in
`workspace.json` under the project and relative view path. Closing a tab releases
its mounted preview. Reopening the same file restores its settings, and moves
within Studio carry the settings to the destination.

Open a simulator's **Frame settings** from its header. The menu and profile
picker use fixed window-scale controls, so their size stays consistent across
frame profiles and canvas zoom levels. All frame actions are visible without
scrolling.

The floating **View tree** follows the active Design frame. Select a row or
click a component on the canvas to update both the preview selection and
Inspector. Use **Name** in the Inspector and click **Set name** to write a
`.name("…")` modifier. Names are saved with the view source.

Click a preview component to reveal its selected bounds and properties in the
existing Inspector. Use **+ Component** to insert a Layout, Text, Button, Image,
TextField, Gap, or scrollable Layout. A Gap adds width in a row and height in a
column. `Gap()` uses eight logical pixels, and `Gap(16)` sets a custom size.
While the chooser is open, the active frame outlines the destination Layout
and its padded content area. The alignment guide shows how
children line up, and a solid marker previews the **Before**, **After**, or
**Append** slot. The chooser also supports dragging into
the active preview frame. Drag an existing child to a target edge to place it
before or after that target, or drop inside a compatible Layout or Card to append
it. A drag stays in its originating frame and commits one source edit on release.
Use **Parent**, **First child**, and the move buttons for keyboard-accessible
arrangement. With the Design canvas focused, Alt+Up/Down selects the previous or
next component, Alt+Left/Right selects its parent or first child, and
Alt+Shift+Up/Down moves it among siblings. **Wrap in Layout** encloses the
selected child in a new flow layout.

Select a child of a Layout and choose **Pin for free placement** to drag it
anywhere inside that Layout. Studio saves the nearest corner and two insets in
a `.pin()` modifier. Pinning a child of an unsized root Layout makes that Layout
fill its preview. For a nested Layout, set its size or Fill first. Choose
**Return to flow** to remove the pin.

Select a component with a literal finite `.frame(width, height)` to resize it
using its corner handle. Dragging commits one update through the same source
property engine as the Inspector. Use the Inspector to add or remove fixed size,
Fill, alignment, and supported minimum width. Layouts also expose **Horizontal at
breakpoint** and **Vertical at breakpoint**, which write the corresponding Omen
modifiers. Flow changes apply across the view’s previews.

Design undo and redo cover Inspector and structural edits while the buffer
remains on that history. Code retains its native editing and undo behavior.
Explicit grid cells require additional framework layout support.


The frame renderer uses actual Omen/GPUI builders for supported static components
and modifiers. Each frame evaluates responsive layout at its own logical
viewport width. **Interact** allows native field input, scrolling, and a separate
navigation stack in each frame. A button with a direct
`navigator.push_view(view! { Detail(title: "Release") })` action can open a
same-file view with literal string arguments. Use **Back** to return to the
previous screen. Computed arguments and general Rust actions require running the
app. Browser and device profiles use GPUI. Unsupported properties and invalid
or multiple top-level views show diagnostics. Code stays available for editing
those files. The versioned `VisualTree`, `ComponentEdit`, frame settings, and
existing Inspector operations are reusable Rust APIs for future agent tools.

## Declaration shortcuts

At a Rust module item position, type `view`, `state`, `environment`, `locale`, or
`main` and accept the macro suggestion. It inserts a declaration with editable name
and body defaults. A missing `use omen::prelude::*;` is inserted in the same undo
step; existing direct/grouped prelude imports and `use omen::*;` are recognized.
Imports are placed after crate headers/documentation and in the correct inline
module scope. Names inside strings, comments, function bodies and existing macro
calls do not trigger declaration expansion. Tab/Shift-Tab navigate the placeholders.

Inspector properties update the source immediately. Enum and theme-token values use
anchored dropdowns, booleans use checkboxes, and numeric/text values use direct input.
Spacing and padding also offer token presets. Incomplete or invalid numeric drafts stay
in the field without modifying source. There is no Edit or Apply step.

## View component library

With the code editor focused, press **Cmd+L** on macOS (**Ctrl+L** elsewhere).
The library displays Omen's 18 built-in views with category, overview and the actual
insertion example. Type to search by name, category or purpose. Up/Down choose a
view, Home/End jump to the first/last result, Enter inserts, and Escape dismisses.
Tab/Shift-Tab move keyboard focus through popup controls. Clicking a result shows
its documentation; **Insert View** inserts it at the original cursor.

Insertion preserves surrounding source and indentation, uses the normal editor
undo/save pipeline, and selects the first snippet placeholder. Tab/Shift-Tab then
move through the inserted arguments and body. Insertion is rejected if the active
file or its snapshot changes while browsing. The library currently covers built-in
views; declaration categories such as `state!` and workspace component discovery
can be added later using the same item model. Existing completion suggestions
continue to include project components.

Editable native controls also support `.autofocus(true)` to focus once when they
first become active; the component library uses this for its search field.

### Inspector keyboard navigation

**Cmd+I** (**Ctrl+I** on Windows/Linux) toggles focus between the editor and the
Inspector while the Inspector is visible. The Window menu also offers Focus
Inspector and Focus Editor. **Cmd+1 / Ctrl+1** returns directly to the editor.
The shortcut does not open a hidden Inspector.

Up/Down and Tab/Shift-Tab move through Inspector controls and wrap within the pane. Enter or
Space activates buttons, opens enum dropdowns, and toggles checkboxes. Dropdowns
focus a filter field immediately; type to filter, use Up/Down and Home/End to
choose a value, Enter to select, and Escape to dismiss. Escape from the Inspector
returns to the editor. Values still update source immediately, retaining Inspector
focus. Keyboard hover information is now **Cmd+Shift+I / Ctrl+Shift+I**.

Omen exposes native `.focusRequest($requested)` and `.focusScope(true)` for this
behavior. Focus requests are consumed only when a target is active; a focus scope
wraps sequential navigation. `.focusArrows(true)` additionally enables unmodified
Up/Down traversal; nested dropdowns keep their own arrow-key selection.

### Multiple cursors and deletion shortcuts

The code editor supports the following VS Code-style commands. On Windows/Linux,
use Ctrl in place of Cmd and Ctrl in place of Option for word movement/deletion.

| macOS shortcut | Action |
| --- | --- |
| Cmd+D | Select the current word; repeat to add the next matching occurrence |
| Cmd+Shift+L / Ctrl+F2 | Select all matching occurrences |
| Cmd+K, Cmd+D | Skip the current occurrence and select the next |
| Cmd+U | Remove the last added occurrence selection |
| Cmd+Option+Up/Down | Add a cursor on the adjacent line (Ctrl+Alt+Up/Down elsewhere) |
| Option+click | Add a cursor at an arbitrary position |
| Shift+Option+I | Add cursors at the ends of the selected lines |
| Option+Backspace / Option+Delete | Delete to the previous/next word boundary |
| Cmd+Backspace / Cmd+Delete | Delete to the beginning/end of the line |
| Cmd+Shift+K | Delete the current line or selected lines |
| Esc | Return to one cursor |

Typing, selection movement, Backspace/Delete, indentation, clipboard operations,
and undo/redo work across the active cursors. A multi-cursor text edit is one undo
step. Pasting a matching number of clipboard lines distributes them in document
order; other text is inserted at every cursor. The status bar reports the cursor
count. IME composition consolidates to the primary cursor before starting marked
text so composition is never duplicated or committed prematurely.

Inspector arrow/Tab navigation automatically scrolls the focused property into
view while the fixed Inspector heading stays in place. Manual scrolling is not
continually reset while a control retains focus.
