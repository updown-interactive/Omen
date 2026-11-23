# Navigation, styling, and animation

Phase 5 keeps `main!` and `view!` as the two primary macros. These APIs compose with named views, state, bindings, and ordinary Rust modules.

Run the complete example:

```sh
cargo run -p navigation-styling --locked
```

## Navigation

```rust,ignore
use omen::{view, Navigator, Route};

view! {
    Home() {
        @State let navigator = Navigator::new();
        NavigationStack(navigator, |route: Route| view! { Text(route.name) }) {
            Layout {
                Button("Open project") {
                    navigator.push(Route::new("project").parameter("id", 42));
                }
            }.navigationTitle("Projects")
        }
    }
}
```

`Navigator` clones share an observable path. `push` creates a unique route visit; `pop` returns to the preceding screen, `replace` creates a fresh visit in place of the current one, and `reset` returns to the root. `entries()` exposes visit identities and routes. A `Route` has a name and string parameters; routing decisions belong in the destination factory. `push` and `replace` return false only if the visit-ID space is exhausted.

The stack supplies a platform-specific app bar. iPhone and iPad show a chevron
and center the title by default. Android uses a leading arrow and title. macOS
uses a leading chevron and title, and other desktop hosts and browsers use an
arrow. Back is added automatically on pushed screens.

Set per-screen options on the outer view, including inside a named view:

```rust,ignore
Layout {
    Text("Release notes")
}
.navigationTitle("Published")
.showBackButton(false)
.centerTitle(true)
.appBarBackground("#0D0D0F")
.appBarColor("#EAEAED")
```

Use `.appBarVisible(false)` to hide the whole bar. Without color overrides, the
bar inherits the screen’s background and uses readable text and icons.
`NavigationBar("Title") { ... }` builds a standalone bar with action views.

Covered screens retain state while leaving the visible lifecycle: effects clean up and mount tasks cancel. Returning remounts the preceding screen with its state. Popped or replaced visits are disposed. Reopening the same route creates a new instance. Separate stacks have independent state.

## Tabs

```rust,ignore
@State let selected = String::from("home");
// In the tree:
TabView($selected) {
    Tab("home", "Home") { HomeScreen() }
    Tab("settings", "Settings") { SettingsScreen() }
}
```

Tab identifiers must be unique. Selecting a tab updates its binding; unknown selections display the first tab without changing the binding. Inactive tabs retain state and release visible lifecycle resources. Removing a tab disposes its keyed state. Native tab selectors support pointer activation, Tab focus, and Enter/Space activation, and expose tab/list/panel roles and selected state. Arrow-key tab switching is not added automatically.

## Sheets, dialogs, popovers, and modals

```rust,ignore
@State let showing = false;
// In the tree:
Layout {
    Button("Present") { showing = true; }
}.sheet($showing, || view! {
    Layout { Text("Sheet title") Text("Press Escape to dismiss") }
})
```

`.sheet(binding, factory)` aligns a blocking panel to the bottom of the window. `.dialog(...)` and `.modal(...)` center blocking panels. `.popover(...)` anchors a nonblocking panel at the modified view and fits it inside the viewport. These are in-window GPUI surfaces, not separate operating-system windows.

Setting the binding to false dismisses a presentation. Escape dismisses the top presentation. A blocking backdrop dismisses on click; clicks inside the panel do not dismiss it. Focus moves into a presentation and returns to its opener on dismissal. Blocking surfaces exclude background controls from interaction and Tab order; popovers leave background controls available. Content is created only while visible, participates in normal lifecycle, and is disposed on dismissal. The base view retains state. Apply `.theme(...)` after presentation modifiers to theme the whole surface.

Content factories are repeatable `Fn` closures returning any view. State used directly in a callback is cloned by the DSL. Nested `view!` invocations follow Rust macro boundaries: pass cloned ordinary handles into nested factories explicitly if needed. To build a close button, capture a cloned binding and call `binding.set(false)`.

## Themes and tokens

`.theme(.light)`, `.theme(.dark)`, and `.theme(.system)` inherit through a subtree. System mode follows native window appearance. Semantic `.color(.primary)`, `.color(.secondary)`, `.background(.canvas)`, `.background(.surface)`, `.color(.accent)`, and `.color(.border)` resolve against the nearest theme; `.panel` also uses its surface token. Fixed colors such as `.white`, `.black`, and `.blue` stay fixed.

`Theme::light().with_tokens(tokens)` accepts `DesignTokens` with semantic RGB colors, body font size, panel radius, and a four-step spacing scale. `.paddingToken(.small | .medium | .large | .extraLarge)` and `.spacingToken(...)` resolve that scale at render time. Explicit `.padding(...)` / `.spacing(...)` and token modifiers replace each other in source order. Card and presentation defaults use theme tokens; explicit font, corner-radius, and color modifiers remain overrides. The initial default is dark for compatibility; use `.theme(.system)` to opt into system appearance.

## Responsive layout and safe areas

Use adaptive direction to switch arrangement based on the width inside the layout.

```rust,ignore
view! {
    @Design
    Dashboard {
        Layout {
            Text("Sidebar")
            Text("Content")
        }
        .direction(.adaptive(compact: .vertical, expanded: .horizontal))
    }
}
```

`AdaptiveLayout {}` and `Layout {}.adaptive()` default to that arrangement. Supply an optional `medium: .vertical` entry when medium and expanded space need different directions. Compact is below 600 logical pixels, medium is 600 to 1023, and expanded starts at 1024. When medium is omitted, adaptive direction uses the expanded direction starting at 600.

```rust,ignore
AdaptiveGrid {
    Text("First")
    Text("Second")
    Text("Third")
}.min_item_width(240).spacing(16)
```

The grid divides its available width among as many columns as fit, with a single column in narrow space. Studio includes both adaptive containers in the component library. The layout Inspector writes direction changes back to the source and each Design frame applies its own dimensions.

Use `@Responsive` when the contents of a view change with available space. A named view receives its parent's offered width, including explicit frames, padding, grid cells, and the space left by fixed-width siblings in a row. Intrinsic sibling sizes use the parent width as a fallback.

```rust,ignore
view! {
    Dashboard {
        @Responsive layout
        @Environment env

        Layout {
            if layout.compact {
                Text("Bottom navigation")
            } else {
                Text("Sidebar")
            }
            Text(env.width)
        }
    }
}
```

`env` exposes `size`, `width`, `height`, `orientation`, `safe_area`, `pixel_ratio`, `platform`, `pointer`, `keyboard`, `text_scale`, and `appearance`. Hosts publish environment changes through the reactive runtime. `@Environment device` is an alias. Hosts without a preferred text-scale setting report 1.0. Run views with structural conditions in the project runtime.

`@Environment let viewport: Viewport;` receives content dimensions and host safe-area insets. Reading it subscribes the named view to window-size changes. `viewport.breakpoint()` returns Compact below 600, Medium from 600, Large from 1024, and ExtraLarge from 1440 logical pixels.

```rust,ignore
Layout { Sidebar() Content() }
    .responsive(Breakpoint::Medium, ResponsiveStyle {
        mode: Some(LayoutMode::Horizontal),
        padding: Some(24.),
        ..Default::default()
    })
```

Rules use minimum widths and apply from narrowest to widest. They can override mode, padding, spacing, frame, and font size. Matching responsive values override the base style. For more substantial changes, branch on the observable viewport in a named view.

`.safeArea(.all | .horizontal | .vertical | .none)` adds the relevant insets to existing padding. Consumed edges are not inset again by descendants. `.safeAreaInsets(Insets { ... })` supplies insets for embedded surfaces. GPUI's desktop content viewport already excludes native window chrome, so its host safe-area insets are zero; no guessed notch or mobile inset is added. Other hosts can supply real insets through `Runtime::set_viewport`.

## Motion

```rust,ignore
Card { Text("Move me") }
    .offset(if expanded { 80. } else { 0. }, 0.)
    .opacity(if expanded { 0.6 } else { 1. })
    .animation(Animation::spring(
        Duration::from_millis(800), Spring::default()
    ))
    .transition(.slide)
```

`.animation(Animation::new(duration, easing))` animates opacity and relative offset. Curves are Linear, EaseIn, EaseOut, and EaseInOut. `Animation::spring(duration, Spring { stiffness, damping })` uses a damped spring response and snaps exactly to its target at the settling deadline. Retargeting starts at the currently displayed position. Translation preserves layout allocation and moves the native hit region with the element.

`.transition(.fade)` fades newly mounted content; `.transition(.slide)` also moves it upward from a 16-pixel offset. Entry transitions default to 220 milliseconds. Removal is immediate: exit transitions, shared-element transitions, and arbitrary layout/color interpolation are not part of this basic animation implementation. `.reducedMotion(true)` disables animation in a subtree; the backend also honors GPUI's reduced-motion preference. No animation frames are requested after settling.

## Localization and RTL

```rust,ignore
let strings = Localizer::new(Locale::new("en"), "en")
    .message("en", "greeting", "Hello, {name}")
    .message("ar", "greeting", "مرحباً، {name}");
strings.format("greeting", &[("name", "Ada")]);
strings.set_locale(Locale::new("ar"));
```

Catalog construction is immutable and cheap to clone. Locale selection is observable and shared by clones, so `strings.text(...)`, `strings.format(...)`, and `strings.locale()` inside named views update automatically. Lookup tries the exact locale, then its language subtag, then the configured fallback locale, then displays the key. Named substitution processes values once and does not reinterpret braces in inserted values. Catalogs are application-supplied strings; plural rules, date/number formatting, and external resource loading are not included.

Apply `.locale(strings.locale())` to inherit locale direction, or `.direction(.rightToLeft)` / `.direction(.leftToRight)` explicitly. Common RTL languages are detected from their locale language subtag; an application can override `Locale.direction`. Logical rows reverse, leading/trailing alignment mirrors, text aligns to the appropriate edge, and native fields align short text consistently with hit testing. Content strings remain in logical Unicode order and use GPUI's native text shaping. Mixed-direction editor navigation and platform-specific bidi behavior remain governed by the underlying text system; this does not replace it with a new bidi editor.
