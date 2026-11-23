# Omen Studio: Phase 1 inspection and milestone

This is the original Phase 1 snapshot. For the implemented Phase 2–4 shell and
workspace capabilities, see [Studio documentation](../studio/README.md).

Studio is a native Omen application in `studio/`, now a workspace member. It uses
`omen::{main, view}`, local `@State` bindings, the existing GPUI renderer, and
semantic themes. No browser host or second UI runtime is involved.

Run from the repository root:

```sh
OMEN_WINDOW_TITLE="Omen Studio" OMEN_WINDOW_WIDTH=1100 OMEN_WINDOW_HEIGHT=720 cargo run -p omen-studio
```

`cargo run` does not read `Omen.toml`; the CLI reads that configuration. The SDK
and Cargo paths in Studio are relative, so the project is portable.

## Repository map and evidence

| Area | Implementation and observed behavior |
| --- | --- |
| Workspace | Root `Cargo.toml`: public `omen` facade, core, UI builders, macros and macro-core compiler, GPUI and web adapters, components, plugin/native bridges, CLI, Android platform, examples, Studio |
| Entry and DSL | `omen-macros/src/lib.rs` exports `main!`, `view!`, `state!`; `omen-macro-core/src/lib.rs` generates entry points; parser, validation, rewrite, codegen and models modules implement DSL and observable models |
| Desktop | `omen-gpui/src/lib.rs`: `run_with_application` opens GPUI `WindowOptions`; `OmenWindow` reconciles an Omen runtime and converts owned nodes into GPUI elements |
| State | `omen-core/src/reactive.rs`, `observation.rs`: mounted scopes retain state, track dependencies, reconcile keyed identities and clean up effects/tasks; GPUI installs a runtime waker that schedules `cx.notify()` |
| Text and keyboard | `omen-gpui/src/input.rs`: retained input entities, UTF-16 IME ranges, grapheme-aware deletion, mouse selection, clipboard actions; root Tab/Shift-Tab traversal and control activation |
| Layout | `Layout.mode`, frame, alignment, responsive styling; GPUI flex/grid translation. Root scrolling existed; this milestone adds independently scrolling regions and remaining-space allocation |
| CLI | `omen-cli/src/main.rs`, `commands`, `project`, `platforms`, `toolchain`: project generation, doctor, devices, build, run and toolchain setup; `docs/cli.md` documents commands |
| Packaging | `scripts/package.py`, `omen.app.json`, platform templates; CLI has its own platform build/run modules |
| Platform pipelines | `.github/workflows/ci.yml` configures macOS/Linux/Windows checks, WASM/browser tests and Android/iOS compilation; `docs/cross-platform.md` records experimental native mobile limitations |
| Tests/examples | Core unit tests, macro compiler tests and compile-fail fixtures, UI reactive/runtime/design/navigation tests, GPUI simulated-window input/layout/presentation tests, browser host and Playwright tests; examples cover controls, reactive state, navigation, platform UI and plugins |

## Capability matrix

“Supported” describes inspected code on the desktop path, not qualification on
all operating systems. Mobile/web support must be assessed separately.

| Studio requirement | Status | Evidence / next work |
| --- | --- | --- |
| Native GPU desktop shell | Supported | Existing Omen → GPUI path; Studio uses it |
| Resizable three-panel window | Supported | Horizontal Layout, fixed side widths, flexible center; OS window resizing. Very narrow windows can compress side panels |
| SplitView / draggable dividers | Missing | Window resizing works; independent pointer-resized splits are deferred |
| ScrollView equivalent | Supported for this milestone | `.scrollable()` on bounded/flexible Layout; stable native element identity retains scrolling; no scrollbar widget added |
| TreeView | Missing | Explorer is a filtered flat sample list, no filesystem traversal |
| ListView equivalent | Incomplete | Existing List is eager; keyed identity works, virtualization missing |
| TextField | Supported | Existing binding, Unicode, selection, IME callbacks, copy/cut/paste and focus APIs |
| TextEditor | Missing | Single-line input is not a multiline code editor; source panel is read-only |
| Toolbar | Supported by composition | Horizontal Layout with labeled native controls |
| Menu / ContextMenu | Missing | No Studio command registry or native application/context menus |
| Popover / Dialog | Supported | Existing presentation modifiers and GPUI presentation host; modal focus and dismissal tests |
| ProgressView equivalent | Supported | Existing Progress control |
| Multiple windows | Incomplete | OmenWindow instances can be hosted with GPUI; public main/run path opens one window, no declarative window commands |
| Focus / keyboard shortcuts | Incomplete | Tab traversal and standard editing shortcuts exist; IDE-wide shortcut registration and focus commands needed |
| Mouse selection / dragging / capture | Incomplete | Input selection and gestures exist; no reusable pointer capture or draggable panel API |
| Clipboard / file drop | Incomplete | Text clipboard exists; project file drag-and-drop missing |
| Unicode / IME | Incomplete qualification | Desktop grapheme deletion and UTF-16 composition implemented; native OS IME candidate placement requires interactive qualification; Android IME unfinished |
| Virtualized scrolling | Missing | Required before large filesystem trees or large-file editors |
| Async tasks / cancellation | Supported | AsyncAction, TaskHandle, Abortable jobs and mounted lifecycle ownership |
| Accessible controls | Incomplete qualification | Semantic roles, labels, input actions and modal focus exist; OS assistive-technology audit pending |
| Theme tokens / dark mode | Supported | Core design tokens, inherited themes, Studio light/dark toggle |
| Rust language intelligence | Missing | No LSP client, diagnostics or completion engine |
| Native live preview | Incomplete | In-process Omen text preview responds to binding changes; arbitrary project preview host absent |
| State-preserving hot reload | Missing | Normal reactive rerenders preserve mounted state; recompiling Rust is not hot reload |
| Creation / installation / builds | Incomplete integration | CLI capabilities exist; Studio has no project wizard, installer or build orchestration yet |
| Terminal / debugger | Missing | No Studio terminal emulator, PTY or DAP client |
| Mobile and web IDE tools | Missing | Framework pipelines exist; Studio integration deferred |

## Milestone behavior

The explorer filters bundled sample filenames and changes the displayed sample
source. Editing “Preview text” changes a native Omen Text view and Unicode
character count. Theme changes and file selection retain input state. These are
in-memory samples, not filesystem editing, compilation or arbitrary source preview.
The framework additions are `.fill()` and `.scrollable()`; both are implemented
in GPUI and the existing web renderer without introducing another view tree type.
The native root also unwraps transparent single-child groups so viewport-sized
named views receive a bounded height.

## Acceptance checks

Automated: `cargo test -p omen-studio -p omen-ui -p omen-macro-core -p omen-gpui -p omen-web`.
Studio tests exercise binding changes across filtering, file selection and theme
changes. Existing GPUI tests cover native Unicode input, clipboard and Tab traversal.

Interactive desktop procedure:

1. Run the command above. Resize the native window; all three panels should remain
   present and the center should consume remaining space.
2. Tab and Shift-Tab through the toggle, filter, file buttons and preview field;
   type into the focused field and use standard selection/clipboard shortcuts.
3. Enter Unicode text into the preview field; the preview and count should update.
4. Filter/select files and switch themes; the preview text should remain intact.
5. At a short window height, scroll the source panel without moving the toolbar.

Later milestones should first add pointer-resized splits and a virtualized,
multiline editor, then filesystem documents and language-server support. Public
window commands, terminal/debugging and isolated compiled previews can follow.

## Validation in this change

- The targeted Studio, UI, macro-core, GPUI and web test suite passed offline.
- After the final root sizing change, all 13 GPUI tests and the Studio state test
  passed again. The new GPUI test resizes a grouped root to 900 and 700 logical
  pixels, hits the right panel near the bottom edge, and edits Unicode text in
  the center panel at each size.
- Formatting checks passed for all changed Rust packages.
- The native macOS window was launched outside the sandbox and visually observed
  with three panels. Desktop automation could capture its screenshot but rejected
  click/keyboard actions with `noWindowsAvailable`; native input verification
  therefore relies on the simulated GPUI tests. The final sizing adjustment was
  tested in GPUI, but was not visually recaptured.
- Platform support beyond this macOS host was inspected, not executed.
- Strict targeted Clippy was attempted but stopped at the existing
  `clippy::nonminimal_bool` expression in `omen-macro-core/src/codegen.rs:58`.
  That unrelated expression was left unchanged.
