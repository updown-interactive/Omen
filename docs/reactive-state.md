# Reactive state and lifecycle

Omen still has two primary macros: `main!` and `view!`. Named views are retained reactive boundaries. State declarations belong at the start of a named view body, interleaved with local Rust statements.

```rust,ignore
use omen::{main, view};

view! {
    Counter() {
        @State let count: i32 = 0;
        @State let name = String::new();
        Layout {
            Text("Count: {count}")
            TextField("Name").value($name)
            Button("Increment") { count += 1; }
            Text("Reset").onTap { count = 0; }
        }.spacing(12)
    }
}
main! { view! { Counter() } }
```

Reads use ordinary value syntax. Assignments, compound assignments, and common collection mutations (`push`, `pop`, `insert`, `remove`, `clear`, `retain`, `sort`, `sort_by`, `reverse`, `extend`, `truncate`) update the stored value and notify observers. `$name` gives its shared handle; use `$name.update(...)` for other mutations. State reads clone values, so values read in the DSL must implement `Clone`. Custom Rust macros with unusual token grammars should receive explicit `$state.get()` expressions; Omen rewrites expressions in ordinary comma-separated macro arguments and format captures.

`$model.name` creates a projected `Binding<String>`, suitable for `.value(...)`; `$model.enabled` works with `.checked(...)`. Binding writes invalidate observers. Binding closures run on the UI thread.

Named-view properties must implement `Clone + 'static`; property clones are supplied on each rebuild. State handles captured by actions are cloned automatically. Ordinary properties and local Rust captures follow Rust ownership rules: clone a value before moving it into several callbacks.

## Identity and updates

Each named view has independent state slots identified by declaration name and its stable location in the retained tree. Source locations distinguish sibling declarations; `.key(value)` supplies explicit identity. Keys use their string representation and must be unique among siblings.

```rust,ignore
List(tasks, id: |task| task.id) { task in
    TaskRow(title: task.title)
}
```

Keyed rows preserve state and native control identity when reordered. Removing a row disposes its retained state; reinserting that key starts a new instance. Unkeyed repeated rows use their position, so use explicit keys for mutable collections. A view hidden by a conditional retains its state while its owning view remains alive. Its lifecycle resources are released while hidden; remounting runs appearance hooks again. Retained conditional state is released with its owner/runtime.

Observable reads subscribe the named boundary currently being built. Writes mark only dependent boundaries dirty and coalesce backend wakeups. Rebuilding a parent also reevaluates child recipes to deliver updated properties. GPUI refreshes the scene; the targeted unit is a named view, not an individual text node. Dependency subscriptions change with the branches actually evaluated.

## Shared models

An observable model is a typed, single-threaded shared value. It requires no derive macro.

```rust,ignore
#[derive(Clone)]
struct Session { name: String }
let session = omen::Observable::new(Session { name: "Ada".into() });
// Supply with: Home().environment(session.clone()) inside view!.

view! {
    Editor() {
        @Environment let session: Session;
        Layout {
            TextField("Name").value($session.name)
            Text(session.name)
        }
    }
}
```

The closest `.environment(session)` overrides that underlying model type. Missing models produce `RuntimeError::MissingEnvironment`. Changes through an observable notify all reading views, including other runtimes using the same handle. Models and state are main-thread values; do background computation separately and apply its result on the UI executor.

## Lifecycle and effects

`.onAppear { ... }` runs when a node mounts. `.onDisappear { ... }` runs on removal, conditional hiding, or runtime destruction. `.effect(dependency, move || { ...; cleanup_action })` runs setup on mount and when its dependency changes; it invokes the previous cleanup before replacement and on unmount. The dependency must implement `Debug + PartialEq + 'static`. Return an `omen::Action`, using `Action::new(|| {})` when no cleanup is needed. Keep dependency snapshots immutable.

Effects that write observed state are reconciled until settled, with a recoverable error after 32 passes. Avoid unconditional state writes on every appearance/effect dependency change. Avoid relying on ordering between unrelated siblings' cleanup callbacks.

## Async work

```rust,ignore
@State let request: Option<omen::TaskHandle> = None;
@State let message = String::new();
// In the view tree:
Button("Load") async {
    omen::sleep(std::time::Duration::from_secs(2)).await;
    message = "Finished".to_string();
    Ok::<(), String>(())
}.taskHandle($request).onError(|error| { message = error; })
```

`.onTap async { ... }` starts an async action on other views. `.task async { ... }` (also `.onAppear async { ... }`) starts work on mount. Actions return either `()` or `Result<(), E>` where `E: Display`. Errors go to `.onError(...)` and task status. `.taskHandle($request)` exposes activation tasks for explicit cancellation and tracked `status()` reads. Mount tasks are automatically owned and cancelled but do not populate this activation handle.

Starting another task at the same node cancels its previous invocation. Hiding/removing its node, removing a keyed owner, or dropping the runtime cancels it too. Cancellation stops future polling; synchronous work already executed is not rolled back. Await nonblocking work: `omen::sleep` is available with the desktop feature. Blocking I/O or CPU work inside an action will still block the UI thread.

Run the demonstration:

```sh
cargo run -p reactive-state --locked
```

The renderer-independent API is `Runtime::new(view.into_node())`, `render()`, `set_waker(...)`, and `take_tasks()`. Custom backends must drive queued `TaskJob::run()` futures on their local executor. Runtime errors are returned to the host; the desktop adapter reports later failures in the terminal and displays an error view.

## Organizing logic independently of UI

Supporting `state!`, `locale!` and `environment!` declarations can have any valid Rust name. Keep them beside a view or put them in ordinary Rust modules and import them. `main!` and `view!` remain the application and UI entry points.

```rust,ignore
use omen::prelude::*;

state! {
    pub DashboardData {
        tab: String = "overview"
        count = 0

        pub fn increment(&mut self) {
            self.count += 1;
        }
    }
}

locale! {
    pub Messages {
        en { title: "Dashboard" }
        ar { title: "لوحة التحكم" }
    }
}

environment! {
    pub Preferences {
        @Environment let username: String = "Guest";
    }
}

view! {
    Home {
        @State data = DashboardData()
        @Localize strings = Messages::new()
        @Environment preferences: Preferences
        @Environment viewport

        Layout {
            Text(strings.title)
            Text("Hello, {preferences.username}")
            Text("{data.tab}: {data.count}")
            TextField("Tab").value($data.tab)
            Button("Increment") { data.increment() }
            Button("Arabic") { strings.locale = "ar" }
            Text("Viewport: {viewport.breakpoint} · {viewport.width} × {viewport.height}")
                .font(.caption)
                .color(.secondary)
        }.locale(strings.locale)
    }
}

main! { Home().environment(Preferences::shared()) }
```

Inside `state!`, fields use `name[: Type] = initializer`. Both `let` and semicolons are optional; the older `let` and `@State let` spellings remain accepted. Fields remain public, and Rust methods retain their declared visibility. Documentation, `cfg` fields, and ordinary derives such as `Debug` and `Clone` are supported. Constructors named `new` and `shared` are generated and cannot be redeclared.

`state!` and legacy plain-field `environment!` declarations create cloneable data models with `new()`, `Default` and `shared()` constructors. Environments with `@Reactive` fields use the field-level behavior described below. Literal field types can be inferred; give complex initializers an explicit type. Model initializers run once per state owner, and `shared()` returns an observable instance for environment injection. Inside view action blocks, call business methods directly: `data.increment()`. Omen lowers calls rooted in a local `@State` binding to the existing observable update API. `data.update(Model::method)` and `data.update(|model| { ... })` remain available, including for injected environment models. Field assignments and `$data.field` bindings also notify automatically. Updates invalidate at model granularity.

`locale!` creates a reusable catalog factory. Each `Messages::new()` has independent locale selection. `@Localize strings = Messages::new()` retains that localizer and enables `strings.title` and `strings.locale` syntax, just like an inline `@Localize` catalog.

Text interpolation works for state, ordinary locals and fields without an application-written `format!`. Use `{{` and `}}` for literal braces. Format specifiers such as `{count:03}` and `{value:?}` remain available. The computed `viewport.breakpoint` shorthand calls the viewport's breakpoint method.

See [`examples/separated-logic`](../examples/separated-logic/src/main.rs) for declarations split across files, plus an environment declared alongside its view.

## Compact state declarations and reactive actions

`@State data = DashboardData()` is constructor shorthand inside `view!`; ordinary Rust code continues to call `DashboardData::new()` or `Default::default()`. The macro generates an ordinary cloneable struct with concrete field types, plus `new()`, `Default`, and `shared()`. It does not allocate another reactive container for each field. The existing runtime owns the observable model and retains it across rebuilds under the same view identity and state slot.

| Inferred initializer | Field type |
| --- | --- |
| `"Hello"` | owned `String` |
| `true`, `false` | `bool` |
| `'x'` | `char` |
| `0`, `-3` | `i32` |
| `0.5`, `-1.5` | `f64` |
| `2_u8`, `1.0_f32` | literal suffix type |

Complex initializers need an annotation, for example `items: Vec<i32> = vec![1, 2]`. Explicitly typed string literals convert to the declared string-compatible type. Rust checks all other initializer and method argument compatibility. There is no runtime type inference or method lookup.

These action mutations share the same observable model as `$data.field`:

```rust,ignore
Button("Change") {
    data.increment();
    data.count += 1;
    data.enabled.toggle();
    data.selected = "details";
}
Toggle("Enabled", $data.enabled)
```

Synchronous action blocks and event closures batch notifications: each modified observable notifies once at the outer action boundary, after mutable borrows have been released. The renderer retains its existing redraw scheduling and targeted invalidation. Updates use the existing model's notification behavior, including its lack of equality-based change detection. Async actions still mutate through the same handles, but batches do not span `await` points.

Direct custom model calls are considered potentially mutating inside actions. Common read methods such as `len`, `is_empty`, `as_ref`, and `clone` retain snapshot-read behavior. Calls on ordinary local variables and shadowed names remain ordinary Rust calls. Outside actions, model reads retain their existing snapshot semantics.

Method arguments and collection indices are evaluated before taking a mutable state borrow, so `data.add(data.count)` and `data.items[data.index] += 1` are safe. Mutable references, borrowed mutation chains such as `data.items.iter_mut()`, and mutation callbacks capturing the same state produce a compile-time diagnostic instead of modifying a temporary snapshot or causing a nested borrow. Use `data.update(|value| { ... })` for such operations, or capture the needed scalar values before invoking a mutation callback. The update closure is ordinary Rust and cannot return references into the temporary mutable borrow.

The reusable state model and action lowering are backend independent. Compilation is checked on desktop, WASM, Android native GPUI, and iOS GPUI. Compilation alone does not verify device rendering, simulator lifecycle, input, or accessibility.

## Reactive application environments

```rust,ignore
use omen::prelude::*;
environment! {
    pub AppEnvironment {
        @Reactive username = "Guest"
        @Reactive dark = false
        navigator: Navigator = Navigator::new()
        pub fn toggle_theme(&self) { self.dark.toggle(); }
    }
}
view! {
    Home {
        @Environment app
        Layout {
            Text(app.username)
            TextField("Username").value($app.username)
            Button("Theme") { app.toggle_theme() }
        }.theme(if app.dark { .dark } else { .light })
    }
}
main! { Home().environment(AppEnvironment::new()) }
```

Only `@Reactive` fields become `Value<T>`; ordinary fields retain their declared Rust types. Services, receivers, locks and navigators are never implicitly wrapped. Literal types can be inferred, while complex initializers need explicit types. Field visibility defaults to `pub` for compatibility with `state!`; explicit `pub(crate)`, `pub(super)` and other restricted visibility is preserved. Put visibility after `@Reactive` when both are present.

An environment containing reactive fields does not implicitly derive `Clone`, allowing non-cloneable resources. Add `#[derive(Clone)]` if all resources support it and snapshots are useful. Views borrow the injected model and snapshot individual fields; rendering does not clone the whole environment. Existing handwritten structs remain supported through `.environment(Value::new(model))`, and existing `.environment(observable)` injection and nearest-ancestor overrides are unchanged.

`new()` and `shared()` use declared initializers. A typed environment field without an initializer becomes a constructor argument, in declaration order; `Default` is generated only when no arguments are required. External ordinary Rust `impl` blocks work. Within Rust methods, reactive fields use `.get()`, `.set()`, `.update()` and boolean `.toggle()` as usual. Inside views, `app.username` reads the inner value, `$app.username` creates a writable binding, and `app.username = "Ada"` writes through that binding. Explicit field methods such as `app.username.clone()` retain their ordinary Rust meaning (cloning the handle).

`@Environment app` is shorthand for `@Environment app: AppEnvironment`, resolved in the calling module. Other application environment names require explicit types; the existing built-in `viewport` and `lifecycle` shorthands remain supported. Macros do not infer environment types from a distant application entry point. Nested mutable resources and compound assignments should use explicit `.update()` closures. Awaited methods that borrow an environment snapshot require that model to implement `Clone`; Omen owns that snapshot for the duration of the await and never holds an interior borrow across it. For non-cloneable environments, clone a task/service handle first and await its work.

## Typed translation resources

```rust,ignore
locale! {
    pub AppStrings {
        default: "en"
        en {
            welcome: "Welcome"
            greeting: "Hello {name}"
            files: { one: "{count} file", other: "{count} files" }
        }
        ar {
            welcome: "مرحباً"
            greeting: "مرحباً {name}"
            files: { one: "ملف واحد", two: "ملفان", other: "{count} ملفات" }
        }
    }
}
view! {
    Greeting {
        @Localize strings = AppStrings()
        Layout {
            Text(strings.welcome)
            Text(strings.greeting("Ada"))
            Text(strings.files(3))
            Button("Arabic") { strings.locale = "ar" }
        }.locale(strings.locale)
    }
}
```

`AppStrings()` in `@Localize` constructs a typed resource once per retained view owner. Its translation methods validate keys and argument counts through Rust's type checker. Interpolation arguments are `&str`, ordered alphabetically by parameter name. Plural methods take an `i64` count first, then any named interpolation arguments. Messages support simple `{identifier}` substitution and CLDR integer cardinal plural categories (`zero`, `one`, `two`, `few`, `many`, `other`) through `intl_pluralrules`; `other` is required. Decimal counts, ICU/Fluent message syntax, ordinal plurals and locale-specific number/date formatting are not supported in this DSL.

Locale identifiers are checked using `unic-langid`; quote regional identifiers, e.g. `"en-US" { ... }`. Duplicate locales/keys, missing default catalogs, keys absent from the default catalog, malformed placeholders, and translation parameter/plural-shape mismatches produce macro diagnostics. Non-default locales may omit keys; reads fall back from exact locale to language to the default catalog. A missing plural category tries that locale's `other` before falling back, using the fallback catalog's plural rules. `locale().direction` exposes existing RTL metadata, and `.locale(strings.locale)` applies locale metadata to the view subtree.

`try_set_locale(Locale::new("ar"))` returns an error for unsupported languages; `set_locale` and the view assignment shorthand leave the previous locale unchanged for invalid choices. Regional variants of a declared language are accepted. Clones share locale selection, while separate constructors have independent selections. Resource selection uses the existing `Value<Locale>` notifications, not a second reactive runtime.

Compatibility: catalogs without `default:` retain their original `new() -> Localizer` API and first-locale fallback. All catalogs also expose `typed() -> Self`; `@Localize strings = AppStrings()` selects this typed constructor. The older `@Localize strings = Messages::new()` and inline catalog syntax retain dynamic `.text(key)` behavior. For explicit typed ownership use a retained `@State strings = AppStrings::typed()` and call typed methods, or the `@Localize` shorthand above. Typed resources implement `Deref<Target = Localizer>` and convert into `Localizer` with `.into()`.
