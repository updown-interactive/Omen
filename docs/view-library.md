# Omen component reference

Omen applications use `main!` to start the app and `view!` to define and compose views. Built-in controls, named views, and the optional reusable library all use the same view tree, modifiers, reactive state, and identity system. There is no separate `component!` macro or application-written `impl View` requirement.

Import the application API with `use omen::prelude::*;` or import individual macros and types from `omen`. Add `omen-components` separately when using its reusable views.

## Contents

- [Working example](#working-example)
- [Component index](#component-index)
- [Content and actions](#content-and-actions): Text, Button, Image, Icon
- [Form controls and bindings](#form-controls-and-bindings): TextField, SecureField, Checkbox, Toggle
- [Containers and status](#containers-and-status): Layout, List, Card, Progress, Divider
- [Navigation and presentation](#navigation-and-presentation): NavigationStack, NavigationBar, TabView, Tab, overlays
- [Shared modifiers](#shared-modifiers)
- [Reusable library](#reusable-library)
- [Accessibility and platform behavior](#accessibility-and-platform-behavior)
- [Documenting your own views](#documenting-your-own-views)
- [Examples and API documentation](#examples-and-api-documentation)

## Dependencies

`omen new my-app` creates an SDK-backed application; see the [CLI guide](cli.md) for setup and platform selection. Inside an example at `examples/<app>` in this repository, the local dependencies are:

```toml
[dependencies]
omen = { path = "../../crates/omen" }
omen-components = { path = "../../crates/omen-components" } # Optional reusable library
```

Adjust paths for another project location. Omen is currently an SDK workspace; do not assume these unpublished workspace packages are available from a package registry. See the [API build instructions](#examples-and-api-documentation) for backend-independent documentation and tests.

## Working example

The following is a complete native application. `@State` registers a retained model; `$state.field` gives a two-way binding into that same model.

```rust
use omen::prelude::*;

state! {
    FormState {
        name = ""
        accepted = false
        saved = false

        pub fn save(&mut self) {
            self.saved = true;
        }
    }
}

view! {
    Home {
        @State state = FormState()

        Card {
            Text("Create a profile").font(.title).weight(.semibold)
            TextField("Name").value($state.name)
            Checkbox("Accept the terms", $state.accepted)
            Button("Save") { state.save() }
                .padding(.md)
                .background(.accent)
                .color(.white)
                .cornerRadius(8)
            if state.saved { Text("Saved for {state.name}") }
        }
        .padding(.lg)
    }
}

main! { Home() }
```

A Boolean such as `state.accepted` is a value read. `$state.accepted` is a binding that can also write back. Input controls and presentation modifiers need bindings; Omen does not automatically turn a plain field argument into a binding.

For initialization rules, business methods, cancellation, and state lifetimes, see [reactive state and lifecycle](reactive-state.md).

## Component index

These spellings are Omen DSL syntax inside `view!`, rather than Rust function signatures.

| View | Required arguments | Content | Initial behavior |
| --- | --- | --- | --- |
| `Text` | Text value | None | Owned text label |
| `Button` | Label | Rust action block | Content-sized clickable action |
| `Image` | Asset path | None | Local raster image; native default frame is 96 × 96 |
| `Icon` | Built-in name or SVG path | None | Native default size is 24 × 24 |
| `TextField` | Label/placeholder | None | Empty, single-line text input |
| `SecureField` | Label/placeholder | None | Empty, masked single-line input |
| `Checkbox` | Label; optional binding | None | Unchecked unless bound |
| `Toggle` | Label; optional binding | None | Off unless bound |
| `Layout` | None | Child views | Vertical arrangement |
| `List` | Items; optional `id` function | Pattern followed by `in` and row views | Eager vertical rows |
| `Card` | None | Child views | Themed rounded surface |
| `Progress` | Completion fraction | None | Determinate, zero-to-one progress |
| `Divider` | None | None | Horizontal separator |
| `NavigationStack` | Navigator binding | Root views | Root until a route is pushed |
| `NavigationBar` | Title | Optional action views | Horizontal title/action bar |
| `TabView` | Selected-ID binding | `Tab` views | Selected tab, or first tab if the ID is unknown |
| `Tab` | ID, visible title | Child views | Metadata and content for its parent TabView |

The examples below are `view!` body fragments unless they include declarations. State declarations belong before the view tree in a named or stateful anonymous view. Siblings are adjacent; commas between sibling views are unnecessary. Control actions contain ordinary Rust statements.

## Content and actions

### Text

```rust
Text("Hello").font(.title)
Text("Hello, {state.name}").color(.secondary)
Text("Viewport: {viewport.breakpoint} · {viewport.width} × {viewport.height}")
    .font(.caption)
```

`Text` owns its string. Literal interpolation supports local values, state, and fields without an application-written `format!`. Format specifiers work, for example `Text("Count: {state.count:03}")` or `Text("Items: {state.items:?}")`. Use `{{` and `}}` for literal braces. The viewport example requires `@Environment viewport`; `viewport.breakpoint` is shorthand for its computed breakpoint.

Use typography, foreground color, and accessibility modifiers. Text has a text role by default; use `.accessibilityRole(.heading)` for a heading. `Text` does not accept a child block. Interpolation is a DSL feature; the ordinary Rust builder `Text::new(...)` accepts an already constructed string.

### Button

```rust
Button("Save") { state.save() }.padding(.md)
Button("Fixed width") { state.save() }.frame(240, 44)
Button("Full width") { state.save() }.frame(.infinity, 44)
```

A button's default size follows its label and padding. It does not request the full width of a vertical container. Set width and height with `.frame(width, height)`; `.infinity` fills the available parent axis. Rust builders can use `f64::INFINITY`.

Use padding or a frame to give the button a comfortable activation target. The action runs on activation, never while constructing the view. Synchronous actions run on the UI thread: keep them short. Direct state methods and field writes invalidate subscribed views; synchronous action mutations batch their observable notifications. The label supplies the default accessible name. Pointer activation and supported keyboard/accessibility activation use the same action.

Async actions use an explicit async block:

```rust
Button("Load") async {
    state.loading = true;
    omen::sleep(std::time::Duration::from_millis(100)).await;
    state.loading = false;
}
```

Use `.onError(...)` for failed async results and `.taskHandle($state.request)` when cancellation/status is needed. Tasks are tied to mounted view lifetimes. See [async actions](reactive-state.md) for complete error and cancellation examples. No disabled/loading button property is currently part of the built-in API; compose the required state and presentation explicitly.

### Image

```rust
Image("assets/photo.png")
    .frame(160, 100)
    .cornerRadius(12)
    .accessibilityLabel("A mountain reflected in a lake")
```

The native renderer reads a local raster file, uses contain fitting, and supplies an error fallback if the image cannot be loaded. Without an explicit frame its native placeholder size is 96 × 96 logical pixels. Supply a path accessible to the renderer; the macro does not download remote images or resolve application-specific resource identifiers.

The browser renderer uses an image element, so its path is resolved by the browser against the served assets. Package assets for each deployment target and check their final paths; a desktop filesystem path is not a portable web asset URL. See [packaging](cli.md) and [backend status](cross-platform.md).

### Icon

```rust
Icon("check").color(.green).accessibilityLabel("Completed")
Icon("assets/company.svg").accessibilityLabel("Company")
```

Bundled names are `check`, `close`, `plus`, `search`, `settings`, `info`, `file-plus`, `folder-plus`, `refresh`, `chevron-down`, `chevron-right`, `folder`, `folder-open`, `file`, `file-code`, `file-config`, `file-image`, and `file-rust`. The native renderer also supports local SVG files and uses the current foreground color. Its default icon size is 24 × 24 logical pixels. Browser icon output is a semantic text-symbol fallback, not the same native SVG rendering path. Give meaningful icons descriptive labels; do not rely on a glyph alone to explain an action.

## Form controls and bindings

All form controls take their label or placeholder first. `.value(...)` accepts a `Binding<String>` or shared `TextValue`; `.checked(...)` accepts a `Binding<bool>` or shared `BoolValue`. `$state.field` projects the retained state model into the correct binding.

### TextField

```rust
TextField("Full name")
    .value($state.name)
    .accessibilityLabel("Full name")
    .accessibilityHint("Shown on your profile")
    .onChange { state.saved = false; }
```

A single-line editable field, initially empty when no binding is supplied. The constructor text is its placeholder and default accessible name. Native edits update the binding before invoking `.onChange`; a programmatic write does not invoke that user-edit callback. Use an explicit accessible label when a placeholder alone is insufficient.

Desktop editing includes selection, composition/IME integration, clipboard shortcuts and grapheme-aware editing. Mobile input support is platform dependent; see the [platform qualifications](#accessibility-and-platform-behavior). This is not a multiline text editor.

### SecureField

```rust
SecureField("Password")
    .value($state.password)
    .accessibilityLabel("Password")
```

Uses the same string-binding contract as TextField while masking the displayed value. The desktop adapter also masks native text/accessibility queries and blocks copy/cut export; selection replacement and paste remain available. Debug formatting of shared values is redacted.

Masking is a presentation/input behavior, not encrypted credential storage. Keep secrets out of logs and persist credentials through an appropriate platform service. Do not assume unfinished mobile clipboard or accessibility services have desktop behavior.

### Checkbox

```rust
Checkbox("Accept the terms", $state.accepted)
Checkbox("Accept the terms").checked($state.accepted)
```

Both forms bind the same Boolean storage. Without a binding it starts unchecked. The visible label is also the default accessible name; the control exposes checked state. Activation toggles the binding. Use `.onChange { ... }` for work after a user edit.

### Toggle

```rust
Toggle("Notifications", $state.notifications)
Toggle("Notifications").checked($state.notifications)
```

A labeled on/off switch with the same Boolean binding and callback rules as Checkbox. Without a binding it starts off. Use Checkbox for selection/acceptance and Toggle for a setting whose state changes immediately.

### Explicit shared values in ordinary Rust

Shared handles are useful for interoperation or state owned outside a view:

```rust
use omen::prelude::*;

let name = TextValue::new(String::from("Ada"));
let enabled = BoolValue::new(true);
let form = view! {
    Layout {
        TextField("Name").value(name.clone())
        Toggle("Notifications").checked(enabled.clone())
    }
};
```

Retain these handles rather than recreating them on every rebuild. A computed anonymous tree built once contains snapshots of its computed labels; use a retained view/state boundary when text should rebuild from observed reads.

## Containers and status

### Layout

```rust
Layout { Text("Above") Text("Below") }.spacing(.md)
Layout { Icon("check") Text("Done") }.mode(.horizontal).spacing(8)
Layout { Image("assets/photo.png") Text("Caption") }.mode(.stack)
```

Modes are `.vertical` (default), `.horizontal`, and `.stack`. Stack overlays children, so spacing does not separate overlaid children. `.alignment(...)` places content within its container; it does not choose the arrangement mode.

Use `.horizontal(.medium)` to switch from the default layout to horizontal at the medium breakpoint and above. `.vertical(...)` and `.stack(...)` also accept breakpoint symbols. For more detailed overrides use `.responsive(...)`; see [responsive styling](navigation-styling.md).

There are no separate `VStack`, `HStack`, or `ZStack` components.

### List

```rust
@State items: Vec<(u64, String)> = vec![(1, "First".into()), (2, "Second".into())]

List(items, id: |item| item.0) { (id, title) in
    Text("{id}: {title}")
}
.spacing(.md)
```

List eagerly builds its rows and exposes list/list-item semantics. Patterns can destructure each item. Supply stable, unique IDs when rows may be inserted, removed, or reordered: IDs preserve the retained identity of stateful row views. Do not use a changing position as the identity of a movable item.

The simpler `List(items) { item in Text(item) }` is suitable when positional identity is acceptable. Keyed identity is implemented; list virtualization is not. A List does not become a bounded scroll region just because it has many items: use a bounded frame or `.fill()` plus `.scrollable()` where scrolling is required.

### Card

```rust
Card {
    Text("Summary").font(.headline)
    Text("Your changes have been saved.").color(.secondary)
}
.spacing(.md)
.padding(.lg)
```

A vertical container with a themed surface, border, rounded corners, large-token padding, and medium-token child spacing. Override those values with the standard modifiers. A Card does not have a built-in action or heading role; place controls/headings inside it or add an explicit `.onTap` and accessibility semantics when appropriate.

### Progress

```rust
Progress(0.65).accessibilityLabel("Upload progress")
```

The fraction is clamped to zero through one. Non-finite input becomes zero. It exposes a progress role and numeric bounds. Bind a retained view's fraction to observed state to update it. Progress is determinate; indeterminate spinners are not implemented.

### Divider

```rust
Layout {
    Text("Account")
    Divider()
    Text("Preferences")
}.spacing(.md)
```

A horizontal separator without constructor arguments or child content. The native renderer uses a one-pixel themed border-color rule. Divider is not an additional layout container.

## Navigation and presentation

### NavigationStack

```rust
@State navigator = Navigator()

NavigationStack($navigator) {
    Layout {
        Text("Home")
        Button("Open detail") { navigator.push(Detail(title: "Detail")) }
    }.navigationTitle("Home")
}
```

`Detail` must be a named view in scope. The stack owns a persistent root and observes its navigator. Typed named-property destinations register automatically. Each push has a distinct visit ID; returning to an earlier visit retains its state, and popping a visit ends that visit's lifetime. The built-in back control pops the current visit; the root cannot be popped.

A legacy `NavigationStack(navigator, destination_factory)` constructor remains available for string routes. The navigator also provides `pop`, `replace`, `reset`, and `entries`. See [navigation](navigation-styling.md) for the route API and lifecycle details. Core navigation does not imply an OS-native navigation widget or a verified mobile system-back integration.

### NavigationBar

```rust
NavigationBar("Tasks") {
    Button("Add") { state.add_task() }
}
```

A title followed by optional action views in a horizontal themed bar. Use it for a custom standalone bar. NavigationStack already supplies its own title/back bar, so add another bar only when the UI calls for one.

### TabView and Tab

```rust
@State selected = "overview"

TabView($selected) {
    Tab("overview", "Overview") { Text("Overview content") }
    Tab("settings", "Settings") { Text("Settings content") }
}
```

The first Tab argument is a stable ID; the second is the visible title. Selection is a `String` binding. IDs must be unique within the TabView. An unknown selected ID displays the first tab. Children must be Tab declarations, possibly inside transparent groups/conditionals; unrelated child views produce a runtime construction error.

Switching tabs retains state for tabs still present. Removing a tab ends the removed content's lifetime. Put a Layout inside a tab when its contents need arrangement or spacing. TabView is not a bottom-navigation platform widget.

### Sheets, dialogs, popovers, and modals

```rust
@State sheet = false

Layout {
    Button("Show details") { sheet = true }
}
.sheet($sheet) {
    Text("Details").font(.title)
    Button("Close") { sheet = false }
}
```

`.sheet`, `.dialog`, `.popover`, and `.modal` each take a Boolean binding and a content block. Presentations read the binding to decide visibility and write it when dismissed; passing `state.sheet` instead of `$state.sheet` is not equivalent. Presented content shares the normal state/binding system and starts a new lifetime after dismissal and reopening.

Sheets, dialogs and modals are blocking; popovers are nonblocking. Desktop presentation handling includes dismissal and focus containment/restoration for blocking overlays. Placement and native interaction details depend on the backend. These APIs render Omen views rather than requesting OS sheet/dialog widgets. See [presentation behavior](navigation-styling.md#sheets-dialogs-popovers-and-modals).

## Shared modifiers

Modifiers follow a view expression. Later explicit values replace earlier overrides. Symbol spellings below are DSL syntax; Rust builder methods use snake_case and typed enums from `omen`.

| Purpose | DSL examples | Contract |
| --- | --- | --- |
| Padding | `.padding(16)`, `.padding(.md)` | Logical pixels or spacing token |
| Child gaps | `.spacing(12)`, `.spacing(.lg)` | Layout, Card, and List only |
| Frame | `.frame(240, 44)`, `.frame(.infinity, 44)` | Explicit dimensions; the second example fills parent width |
| Flexible sizing | `.fill()` | Consume/shrink along the parent layout axis |
| Scrolling | `.scrollable()` | Vertical scroll region; constrain its size with a frame or bounded fill layout |
| Arrangement | `.mode(.horizontal)` | Layout only: horizontal, vertical, stack |
| Alignment | `.alignment(.center)` | Leading/trailing, top/bottom, center, or corner combinations |
| Responsive layout | `.horizontal(.medium)` | Apply arrangement at a breakpoint and above |
| Surface | `.background(.surface)` | Solid theme color or typed background |
| Gradient | `.gradient(.blue, .purple, 90)` | Two-color linear gradient; zero points up |
| Rounding | `.cornerRadius(12)` | Logical pixels |
| Typography | `.font(.headline).weight(.semibold)` | Semantic font and weight |
| Font metrics | `.fontSize(20).lineHeight(28).fontFamily("Arial")` | Explicit native/system font settings |
| Foreground | `.color(.secondary)` | Semantic color or typed Color |
| Theme | `.theme(.system)` | System, light, or dark theme |
| Localization | `.locale(strings.locale)` | Locale and layout direction from a localizer |
| Safe area | `.safeArea(.all)` | Consume configured safe-area insets |
| Identity | `.key(item.id)` | Stable sibling identity for retained content |
| Tap | `.onTap { state.select() }` | Action callback, not a build-time invocation |
| Appearance | `.onAppear { ... }`, `.onDisappear { ... }` | Mounted view lifecycle |
| Motion | `.offset(x: 24).opacity(0.8).animation(.spring(800.ms))` | Animate offset/opacity; respects reduced-motion configuration |
| Transition | `.transition(.fade)`, `.transition(.slide)` | Entry transition |
| Accessible name | `.accessibilityLabel("Save changes")` | Override the accessible label |
| Accessible help | `.accessibilityHint("Stores your profile")` | Additional descriptive text |
| Semantic role | `.accessibilityRole(.heading)` | Override the semantic role |
| Keyboard focus | `.focusable(true)` | Opt a view into supported focus handling |

Spacing tokens default to `.sm` = 4, `.md` = 8, `.lg` = 16 and `.xl` = 24 logical pixels; custom design tokens can change them. Legacy `.paddingToken` and `.spacingToken` remain accepted. Semantic fonts include `.caption`, `.footnote`, `.body`, `.headline`, `.title2`, `.title`, and `.largeTitle`; an explicit font size overrides the semantic size. Solid and gradient backgrounds replace each other.

For complete theme, responsive, animation, RTL and localization examples see [navigation and styling](navigation-styling.md). For effects, async task handles and cancellation see [state and lifecycle](reactive-state.md). There is currently no general validation-message, disabled, multiline-input, remote-image-loader, or list-virtualization API.

## Reusable library

Import the components you use from `omen_components`, alongside the public `omen` API. Every property listed below is required, and property order does not matter.

| View | Named properties | Default presentation |
| --- | --- | --- |
| `SectionHeader` | `title: String`, `subtitle: String` | Semibold title heading, secondary subtitle, vertical spacing |
| `StatusBadge` | `label: String` | Green semibold caption, panel background, compact rounded padding |
| `PrimaryButton` | `title: String`, `action: Action` | White text, blue/purple gradient, padded content-sized action |
| `EmptyState` | `title: String`, `message: String` | Centered info icon, heading and supporting text in a Card |
| `LoadingCard` | `title: String`, `fraction: f32` | Titled Card with determinate progress |
| `FormRow` | `label: String`, `value: Binding<String>` | Caption above a text input |
| `PreferenceRow` | `title: String`, `value: Binding<bool>` | Labeled toggle |

```rust
use omen::prelude::*;
use omen_components::{FormRow, LoadingCard, PreferenceRow, PrimaryButton, SectionHeader};

let name = TextValue::new(String::from("Ada"));
let notifications = BoolValue::new(true);
let save = Action::new(|| println!("Save"));
let settings = view! {
    Card {
        SectionHeader(title: "Account", subtitle: "Manage your profile")
        FormRow(label: "Name", value: name.clone())
        PreferenceRow(title: "Notifications", value: notifications.clone())
        LoadingCard(title: "Uploading", fraction: 0.5_f32)
        PrimaryButton(title: "Save", action: save)
    }
};
```

Inside a retained view, pass `value: $state.name` or `value: $state.notifications`. String literals convert to owned String properties. Other named properties must satisfy Rust's normal type conversions; use `0.5_f32` for LoadingCard's `f32` property. PrimaryButton's `Action` is synchronous; use the built-in async Button API for asynchronous work.

## Accessibility and platform behavior

Omen attaches semantic roles and values to its controls. Labels, hints and role overrides are part of the shared view model. This describes the framework contract, not equal accessibility service coverage on every platform.

- **Desktop GPUI:** implemented controls include focus rings, source-order Tab/Shift-Tab navigation, button/check/switch activation, text selection/IME integration and secure-field export restrictions. Actual assistive-technology support still depends on the platform integration.
- **Web:** the renderer emits semantic DOM controls and attributes. Browser input, password-field behavior and accessibility are mediated by the browser and host integration. Native SVG rendering and native text-editing internals are not reused.
- **Android native GPUI:** native GPU rendering and basic touch are implemented, but Android IME composition, hardware editing, clipboard and accessibility-tree export remain unfinished in the current backend. Do not promise production form or screen-reader support from a successful build alone.
- **iOS:** compilation and renderer integration do not establish simulator/device input or accessibility coverage. Consult the current backend status before relying on a capability.

See [cross-platform status](cross-platform.md) for validation evidence and remaining work. Omen renders its own controls; GPUI/native rendering does not mean these are UIKit or Android widget wrappers.

## Documenting your own views

Place Rust documentation comments inside `view!`, before the named declaration. Omen carries them onto the generated Rust type, so they appear in IDE help and Rustdoc:

```rust
use omen::view;

view! {
    /// A compact heading for a task group.
    ///
    /// `title` is the visible section name.
    pub TaskHeading(title: String) {
        Text(title).font(.headline).accessibilityRole(.heading)
    }
}
```

Document required properties, default styling, bindings, events, lifetime expectations and platform restrictions. Normal Rust ownership rules apply to local values; clone an owned property before consuming it more than once.

## Examples and API documentation

| Example | What it demonstrates |
| --- | --- |
| [controls-gallery](../examples/controls-gallery/src/main.rs) | Built-in controls and reusable views |
| [declarative-ui](../examples/declarative-ui/src/main.rs) | Named views across normal Rust modules |
| [reactive-state](../examples/reactive-state/src/main.rs) | Bindings, conditional content, lists and async work |
| [separated-logic](../examples/separated-logic/src/main.rs) | State, translations and environment in separate files |
| [navigation-styling](../examples/navigation-styling/src/main.rs) | Navigation, presentations, tabs, themes, motion and localization |
| [native-mobile](../examples/native-mobile/src/main.rs) | Small native GPU view and touch counter |

Run a desktop gallery with `cargo run -p controls-gallery --locked`. For a CLI-created application, run `omen run` from its project directory.

Generate API documentation without compiling a native renderer:

```sh
cargo doc -p omen -p omen-ui -p omen-components --no-default-features --no-deps --locked
```

The facade documentation starts at `target/doc/omen/index.html`; the reusable component catalog is at `target/doc/omen_components/index.html`.

Verify the executable API examples with:

```sh
cargo test --doc -p omen -p omen-ui -p omen-components --no-default-features --locked
```

### Native file and folder selection

The GPUI desktop backend supports `FilePicker` attached to a button:

```rust
let picker = FilePicker::folders(move |result| {
    match result {
        Ok(Some(paths)) => println!("Selected: {:?}", paths),
        Ok(None) => {}, // cancelled
        Err(message) => eprintln!("{message}"),
    }
});
let button = view! { Button("Choose folder…") {}.filePicker(picker) };
```

`FilePicker::files` selects a file. Configure `multiple`, `files`, `directories`,
and `prompt` before attaching it. The native prompt opens on pointer, keyboard,
or accessibility activation and returns asynchronously on the UI thread. Attaching
a picker replaces the control's ordinary click callback. Mobile and web picker
backends are not implemented yet.

### Separate desktop windows

Use `.window(key, visible, factory)` to declare an independent GPUI desktop window:

```rust
let editor_open = BoolValue::new(false);
let editor = editor_open.clone();
let root = view! { Button("Open editor") { editor.set(true); } }
    .window("editor", editor_open, || view! { Text("Editor window") });
```

The stable key retains its native window across redraws. Closing it writes false
to the binding; setting the binding true again creates a new view lifetime. Set
`.desktopWindow(...)` on factory content to configure its size and title. Factory
content must supply any application environments it needs. The desktop host owns
these windows independently, so closing the launcher leaves other windows open.
This API is currently supported by the GPUI desktop backend.

### Custom desktop title-bar toolbar

Set `DesktopWindow.toolbar` to render Omen controls beside the window action
buttons, in the native title-bar area:

```rust
use omen::prelude::*;

view! {
    AppToolbar {
        Layout {
            Text("My application").weight(.semibold)
            Layout {}.fill()
            Button("Save") { println!("Save"); }
        }.mode(.horizontal).spacing(12).padding(8)
            .frame(.infinity, .infinity)
    }
}

main! {
    Home().desktopWindow(DesktopWindow {
        title: "My application".into(),
        toolbar: Some(DesktopToolbar::new(|| AppToolbar {})),
        ..Default::default()
    })
}
```

`None` (the default) preserves the standard title bar. The default toolbar height
is 44 logical pixels; set `DesktopToolbar.height` before opening to customize it
(clamped to 32–96). Toolbar and body share one Omen runtime, environment, focus
order, and reactive state. Empty title-bar space drags the window; double-click
uses platform title-bar behavior. Interactive controls consume clicks normally.

macOS retains its native traffic-light controls with reserved space on the left.
Windows and Linux use caption controls on the right; Linux requests client-side
decorations. Native title-bar integration is desktop-only; mobile and web ignore
this toolbar option. Window configuration applies when the window opens, including
windows declared with `.window(...)`.

Run `cargo run -p controls-gallery` for a working toolbar example.
