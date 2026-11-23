# Studio editor: Zed architecture review and native interaction update

Reviewed 2026-11-16 against Zed revision
`faf8f0b6980205ca2659af174790a212ae3c416a`, the GPUI revision already pinned in
Omen's Cargo workspace. The GitHub repository's current README was also checked.
This is a targeted editor architecture review, not an audit of the entire Zed repository.

## What GPUI supplies, and what Zed adds

GPUI provides native windows, input dispatch, platform text/IME integration,
layout and GPU painting. Zed's editing behavior lives above those primitives:

| Zed source | Responsibility | Omen implementation |
| --- | --- | --- |
| [text](https://github.com/zed-industries/zed/tree/faf8f0b6980205ca2659af174790a212ae3c416a/crates/text) | Text data, snapshots and editing history | `omen_core::TextSnapshot` with persistent Ropey storage; native undo snapshots |
| [display_map.rs](https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/editor/src/display_map.rs) | Transforms buffer positions for inlays, folds, tabs, wrapping and blocks | Indexed logical-line layout today; a future display map is needed for folding/wrapping |
| [selections_collection.rs](https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/editor/src/selections_collection.rs) | Selection management | UTF-8 selection ranges with grapheme-aware movement and native UTF-16 conversion |
| [element.rs](https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/editor/src/element.rs) | Visible editor layout and painting | `omen-gpui/input/multiline.rs`, cached visible shaped lines and gutter painting |
| [mouse_context_menu.rs](https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/editor/src/mouse_context_menu.rs) | Menus associated with pointer/editor positions | `omen-gpui/input/popups.rs`, deferred window-clamped overlays |
| [editor.rs](https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/editor/src/editor.rs) | Commands and integration with project/language services | Native commands module plus `EditorEvent` bridge to Studio's background LSP actor |

The Zed `editor` and `text` crates declare GPL-3.0-or-later. GPUI is available
under Apache-2.0. This change adds Omen implementations of the interaction
patterns; it does not copy Zed's GPL editor implementation or add it as a dependency.
The existing GPUI input-example adaptation retains its Apache attribution.

## User-facing changes

The permanently visible editor action toolbar, completion list, hover block,
code-action list and diagnostic list have been removed from the document layout.
A right-click menu offers definition, rename, quick fixes/refactoring, formatting,
completion, undo/redo, clipboard, selection, find/replace and line comments.
Right-click inside a selection preserves it; clicking elsewhere relocates the
caret. Menus dismiss outside or with Escape and support arrow/Enter navigation.

Completion and quick-fix lists float at the caret, follow keyboard selection and
accept mouse input. Hover information and diagnostics appear after a 450 ms
pointer pause, without changing the caret/selection. Responses are checked against
the hovered document version and offset. Overlays snap inside the native window
and do not reduce the editor viewport.

Double-click selects a Unicode word, triple-click or gutter click selects a line.
Word movement/selection, smart Home, document boundaries and Page Up/Down augment
existing cursor handling. Native commands provide line comment toggling, line
movement, duplication, bracket pairing/wrapping/skipping, paired deletion and
indented Enter between brackets. Copy/cut without a selection operate on the current
line. A native vertical scrollbar supports large-document scrolling; only visible
lines are shaped. The active line has a subtle highlight.

## Framework boundary

Applications still use the declarative Omen `TextEditor.document(EditorDocument)`
API. Reusable native commands, popup rendering, pointer hover timing, selection and
scrolling live in the GPUI adapter. `EditorState.actions` and `hover_offset`, plus
host events for find, rename, quick fixes and pointer hover, keep project/language
service code out of the control. Studio remains an Omen application.

## Validation and remaining scope

GPUI tests exercise real dragging versus hover, context-menu selection preservation,
Escape dismissal, unchanged viewport dimensions, popup rendering, Unicode word
selection, undoable comments/line moves, bracket editing and the existing
100,000-line viewport/IME tests. Workspace tests cover document edits/recovery;
live rust-analyzer tests cover the language-service command round trips.

This is a substantial native interaction update, not complete Zed or VS Code parity.
Multi-cursor editing, folding, soft wrapping, snippets, a full display-map pipeline,
project-wide search, references/implementation navigation and debugging still need
additional milestones. Existing edit-based code actions are supported; server-command
and filesystem-resource code actions are not. Native visual QA and Windows/Linux
runtime qualification remain pending.
