//! Stable named-view identity, observable dependencies, environment, and lifecycle.
use crate::{
    Action, DesignTokens, DeviceEnvironment, Insets, ResponsiveLayout, Style, TaskContext,
    TaskHandle, TaskJob, Value, ViewKind, ViewNode,
};
use crate::{
    observation::{self, Subscription},
    tasks::TaskQueue,
};
use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    fmt,
    rc::Rc,
};

/// A recoverable view construction/reconciliation error.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// A tab view received content outside a Tab declaration.
    #[error("TabView children must be Tab declarations")]
    InvalidTabContent,
    /// Required shared environment model is absent.
    #[error("missing environment model `{0}`")]
    MissingEnvironment(&'static str),
    /// A stable state slot was reused with a different Rust type.
    #[error("state slot `{0}` changed type")]
    StateType(&'static str),
    /// Siblings supplied the same explicit key.
    #[error("duplicate sibling key `{0}`")]
    DuplicateKey(String),
    /// State writes/effects failed to settle.
    #[error("view updates did not settle after 32 passes")]
    UnstableRender,
    /// Recursive view composition exceeded the supported depth.
    #[error("view nesting exceeded 256 nodes")]
    ExcessiveDepth,
}
/// A typed shared environment. Cloning is cheap; overrides do not mutate parents.
#[derive(Clone, Default)]
pub struct Environment(Rc<HashMap<TypeId, Rc<dyn Any>>>);
impl Environment {
    /// Add or override an observable model by its underlying Rust type.
    pub fn with<T: 'static>(mut self, value: Value<T>) -> Self {
        Rc::make_mut(&mut self.0).insert(TypeId::of::<T>(), Rc::new(value));
        self
    }
    /// Look up an observable model without reading its current value.
    pub fn get<T: 'static>(&self) -> Result<Value<T>, RuntimeError> {
        self.0
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<Value<T>>())
            .cloned()
            .ok_or(RuntimeError::MissingEnvironment(std::any::type_name::<T>()))
    }
    fn merged(&self, child: &Self) -> Self {
        let mut values = self.0.as_ref().clone();
        values.extend(child.0.iter().map(|(key, value)| (*key, Rc::clone(value))));
        Self(Rc::new(values))
    }
}
impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().all(|(key, value)| {
                other
                    .0
                    .get(key)
                    .is_some_and(|other| Rc::ptr_eq(value, other))
            })
    }
}
impl fmt::Debug for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Environment")
            .field("models", &self.0.len())
            .finish()
    }
}

pub(crate) struct ScopeInner {
    responsive: RefCell<Option<Value<ResponsiveLayout>>>,
    width: Cell<f32>,
    states: RefCell<HashMap<&'static str, Box<dyn Any>>>,
    environment: RefCell<Environment>,
    pub(crate) tasks: RefCell<HashMap<String, TaskHandle>>,
}
impl ScopeInner {
    fn cancel_tasks(&self) {
        for task in self.tasks.borrow_mut().drain().map(|(_, task)| task) {
            task.cancel();
        }
    }
}
impl Drop for ScopeInner {
    fn drop(&mut self) {
        self.cancel_tasks();
    }
}
/// Persistent state slots belonging to one stable named-view instance.
#[derive(Clone)]
pub struct Scope(Rc<ScopeInner>);
impl Scope {
    fn new(environment: Environment) -> Self {
        Self(Rc::new(ScopeInner {
            responsive: RefCell::default(),
            width: Cell::default(),
            states: RefCell::default(),
            environment: RefCell::new(environment),
            tasks: RefCell::default(),
        }))
    }
    /// Observe the width offered by the current view's container.
    pub fn responsive(&self) -> ResponsiveLayout {
        self.0
            .responsive
            .borrow_mut()
            .get_or_insert_with(|| Value::new(ResponsiveLayout::new(self.0.width.get())))
            .get()
    }
    fn set_width(&self, width: f32) {
        if self.0.width.replace(width) != width
            && let Some(value) = self.0.responsive.borrow().as_ref()
        {
            value.set(ResponsiveLayout::new(width));
        }
    }
    /// Initialize a typed state slot once, or return its preserved value.
    pub fn state<T: 'static>(
        &self,
        name: &'static str,
        initialize: impl FnOnce() -> T,
    ) -> Result<Value<T>, RuntimeError> {
        if let Some(value) = self.0.states.borrow().get(name) {
            return value
                .downcast_ref::<Value<T>>()
                .cloned()
                .ok_or(RuntimeError::StateType(name));
        }
        let value = Value::new(initialize());
        self.0
            .states
            .borrow_mut()
            .insert(name, Box::new(value.clone()));
        Ok(value)
    }
    /// Resolve a shared environment model in this view's ancestry.
    pub fn environment<T: 'static>(&self) -> Result<Value<T>, RuntimeError> {
        self.0.environment.borrow().get()
    }
}
impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Scope(..)")
    }
}
type ViewFactory = dyn Fn(&Scope) -> Result<ViewNode, RuntimeError>;
/// A named-view recipe, evaluated only when its dependencies or properties change.
#[derive(Clone)]
pub struct ReactiveView {
    /// Rust type identity for this boundary.
    pub name: &'static str,
    factory: Rc<ViewFactory>,
}
impl ReactiveView {
    /// Create a repeatable view recipe.
    pub fn new(
        name: &'static str,
        factory: impl Fn(&Scope) -> Result<ViewNode, RuntimeError> + 'static,
    ) -> Self {
        Self {
            name,
            factory: Rc::new(factory),
        }
    }
}
impl fmt::Debug for ReactiveView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ReactiveView").field(&self.name).finish()
    }
}
struct Wake {
    dirty: RefCell<HashSet<String>>,
    scheduled: Cell<bool>,
    alive: Cell<bool>,
    callback: RefCell<Option<Rc<dyn Fn()>>>,
}
impl Wake {
    fn schedule(&self) {
        if self.alive.get() && !self.scheduled.replace(true) {
            let callback = self.callback.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        }
    }
}
struct Boundary {
    recipe: ReactiveView,
    scope: Scope,
    output: ViewNode,
    callback: Rc<dyn Fn()>,
    subscriptions: Vec<Subscription>,
    active: bool,
}
struct Mounted {
    style: Style,
    cleanup: Option<Action>,
    task: TaskContext,
}
#[derive(Clone, Copy)]
struct Constraints {
    width: f32,
    viewport: f32,
    tokens: DesignTokens,
    insets: Insets,
    dark: bool,
}

impl Constraints {
    #[inline(never)]
    fn for_node(mut self, node: &ViewNode) -> (Self, Option<(u16, f32)>, bool, f32) {
        if let Some(width) = node
            .style
            .preview
            .as_deref()
            .and_then(|preview| preview.width)
        {
            self.width = width;
            self.viewport = width;
        }
        if let Some(theme) = node.style.theme {
            self.tokens = theme.resolve(self.dark);
        }
        if let Some(insets) = node.style.safe_area_insets {
            self.insets = insets;
        }
        let style = if node.style.responsive.is_empty() {
            std::borrow::Cow::Borrowed(&node.style)
        } else {
            std::borrow::Cow::Owned(node.style.for_width(self.viewport))
        };
        let content_width = style.content_width(self.width, self.tokens, self.insets);
        let grid = style.grid_metrics(content_width, self.tokens);
        let safe_area = style.safe_area;
        let mode = style
            .adaptive_direction
            .map(|direction| direction.resolve(content_width))
            .or(style.mode);
        let row =
            matches!(node.kind, ViewKind::Layout(_)) && mode == Some(crate::LayoutMode::Horizontal);
        let spacing = style
            .spacing
            .or_else(|| style.spacing_token.map(|token| self.tokens.space(token)))
            .unwrap_or(0.);
        let has_padding = style.padding.is_some() || style.padding_token.is_some();
        self.width = content_width;
        if matches!(node.kind, ViewKind::Card(_)) && !has_padding {
            self.width = (self.width - 2. * self.tokens.space(crate::Space::Large) - 2.).max(0.);
        }
        if matches!(
            safe_area,
            Some(crate::SafeArea::All | crate::SafeArea::Horizontal)
        ) {
            self.insets.left = 0.;
            self.insets.right = 0.;
        }
        if matches!(
            safe_area,
            Some(crate::SafeArea::All | crate::SafeArea::Vertical)
        ) {
            self.insets.top = 0.;
            self.insets.bottom = 0.;
        }
        (self, grid, row, spacing)
    }
}

/// A retained reactive view tree. Each runtime owns its own identity/state graph.
pub struct Runtime {
    root: ViewNode,
    viewport: Value<crate::Viewport>,
    device: Value<DeviceEnvironment>,
    lifecycle: Value<crate::AppLifecycle>,
    base_environment: Environment,
    wake: Rc<Wake>,
    boundaries: HashMap<String, Boundary>,
    root_scope: Scope,
    mounted: HashMap<String, Mounted>,
    lists: HashMap<String, HashSet<String>>,
    queue: Rc<TaskQueue>,
    rebuilt: Vec<String>,
}
impl Runtime {
    /// Create an isolated runtime. Mounting occurs on the first render.
    pub fn new(root: ViewNode) -> Self {
        let viewport = Value::new(crate::Viewport::default());
        let device = Value::new(DeviceEnvironment::default());
        let lifecycle = Value::new(crate::AppLifecycle::Foreground);
        let base_environment = Environment::default()
            .with(viewport.clone())
            .with(device.clone())
            .with(lifecycle.clone());
        Self {
            root,
            viewport,
            device,
            lifecycle,
            base_environment,
            wake: Rc::new(Wake {
                dirty: RefCell::default(),
                scheduled: Cell::new(false),
                alive: Cell::new(true),
                callback: RefCell::default(),
            }),
            boundaries: HashMap::new(),
            root_scope: Scope::new(Environment::default()),
            mounted: HashMap::new(),
            lists: HashMap::new(),
            queue: Rc::default(),
            rebuilt: Vec::new(),
        }
    }
    /// Suspend visible resources while retaining state, or resume after becoming visible.
    pub fn set_lifecycle(&mut self, lifecycle: crate::AppLifecycle) {
        if self.lifecycle.get() != lifecycle {
            self.lifecycle.set(lifecycle);
        }
    }
    /// Update host viewport data. Equal values do not invalidate readers.
    pub fn set_viewport(&mut self, viewport: crate::Viewport) {
        if self.viewport.get() != viewport {
            self.viewport.set(viewport);
            self.device.set(self.device.get().with_viewport(viewport));
        }
    }
    /// Update device capabilities and legacy viewport readers together.
    pub fn set_device_environment(&mut self, device: DeviceEnvironment) {
        let device = device.with_viewport(device.size);
        if self.viewport.get() != device.size {
            self.viewport.set(device.size);
        }
        if self.device.get() != device {
            self.device.set(device);
        }
    }
    fn constraints(&self) -> Constraints {
        let device = self.device.get();
        let dark = device.appearance == crate::ThemeMode::Dark;
        Constraints {
            width: device.width,
            viewport: device.width,
            tokens: crate::Theme::default().resolve(dark),
            insets: device.safe_area,
            dark,
        }
    }
    /// Install a backend wake callback. Multiple writes before a render coalesce.
    pub fn set_waker(&mut self, callback: impl Fn() + 'static) {
        *self.wake.callback.borrow_mut() = Some(Rc::new(callback));
    }
    /// Named view types rebuilt by the last render call.
    pub fn last_rebuilt(&self) -> &[String] {
        &self.rebuilt
    }
    /// Drain async work for the host's local executor.
    pub fn take_tasks(&mut self) -> Vec<TaskJob> {
        self.queue.0.borrow_mut().drain(..).collect()
    }
    /// Build initial recipes without mounting hooks/tasks, for host startup validation.
    pub fn prepare(&mut self) -> Result<(), RuntimeError> {
        let mut seen = HashSet::new();
        let mut mounts = Vec::new();
        self.resolve(
            &mut self.root.clone(),
            "root".into(),
            &self.base_environment.clone(),
            self.root_scope.clone(),
            self.constraints(),
            &mut seen,
            &mut mounts,
            0,
        )?;
        Ok(())
    }
    /// Render dirty boundaries, reconcile identities, and run lifecycle transitions.
    pub fn render(&mut self) -> Result<ViewNode, RuntimeError> {
        self.rebuilt.clear();
        self.wake.scheduled.set(false);
        if self.lifecycle.get() == crate::AppLifecycle::Background {
            self.reconcile_mounts(Vec::new());
            for boundary in self.boundaries.values_mut() {
                boundary.active = false;
                boundary.subscriptions.clear();
                boundary.scope.0.cancel_tasks();
            }
            self.root_scope.0.cancel_tasks();
            return Ok(ViewNode::new(ViewKind::Group(Vec::new())));
        }
        for _ in 0..32 {
            let mut seen = HashSet::new();
            let mut mounts = Vec::new();
            let mut root = self.root.clone();
            self.resolve(
                &mut root,
                "root".into(),
                &self.base_environment.clone(),
                self.root_scope.clone(),
                self.constraints(),
                &mut seen,
                &mut mounts,
                0,
            )?;
            for (id, boundary) in &mut self.boundaries {
                if boundary.active && !seen.contains(id) {
                    boundary.active = false;
                    boundary.subscriptions.clear();
                    boundary.scope.0.cancel_tasks();
                }
            }
            self.reconcile_mounts(mounts);
            let active_dirty = self.wake.dirty.borrow().iter().any(|id| {
                self.boundaries
                    .get(id)
                    .is_some_and(|boundary| boundary.active)
            });
            if !active_dirty {
                return Ok(root);
            }
        }
        Err(RuntimeError::UnstableRender)
    }
    fn context(&self, key: String, scope: &Scope) -> TaskContext {
        let wake = Rc::downgrade(&self.wake);
        TaskContext {
            queue: Rc::clone(&self.queue),
            owner: Rc::downgrade(&scope.0),
            key,
            wake: Rc::new(move || {
                if let Some(wake) = wake.upgrade() {
                    wake.schedule();
                }
            }),
        }
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "explicit reconciliation context avoids hidden mutable runtime state"
    )]
    fn resolve(
        &mut self,
        node: &mut ViewNode,
        id: String,
        environment: &Environment,
        owner: Scope,
        constraints: Constraints,
        seen: &mut HashSet<String>,
        mounts: &mut Vec<(String, Style, TaskContext)>,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > 256 {
            return Err(RuntimeError::ExcessiveDepth);
        }
        let environment = node
            .style
            .environment
            .as_ref()
            .map_or_else(|| environment.clone(), |child| environment.merged(child));
        let (next_constraints, grid, row, spacing) = constraints.for_node(node);
        let constraints = next_constraints;
        node.style.runtime_id = Some(id.clone());
        if let ViewKind::Reactive(recipe) = &node.kind {
            let boundary_id = segment(&id, recipe.name);
            let boundary =
                self.prepare_boundary(&boundary_id, recipe, &environment, constraints.width)?;
            seen.insert(boundary_id.clone());
            let mut child = boundary.output.clone();
            let resolve = self.resolve(
                &mut child,
                segment(&boundary_id, "body"),
                &environment,
                boundary.scope.clone(),
                constraints,
                seen,
                mounts,
                depth + 1,
            );
            let scope = boundary.scope.clone();
            self.boundaries.insert(boundary_id, boundary);
            resolve?;
            node.kind = ViewKind::Group(vec![child]);
            let context = self.context(id.clone(), &scope);
            node.style.task_context = Some(context.clone());
            mounts.push((id, node.style.clone(), context));
            return Ok(());
        }
        let context = self.context(id.clone(), &owner);
        node.style.task_context = Some(context.clone());
        mounts.push((id.clone(), node.style.clone(), context));
        let list = matches!(node.kind, ViewKind::List(_)) || node.style.keyed_children;
        let navigation_chrome = node.style.navigation_chrome.clone();
        match &mut node.kind {
            ViewKind::Layout(children)
            | ViewKind::Card(children)
            | ViewKind::Group(children)
            | ViewKind::List(children)
            | ViewKind::Presentation { children, .. } => {
                let mut keys = HashSet::new();
                let mut occurrences = HashMap::<&'static str, usize>::new();
                let mut row_ids = HashSet::new();
                let child_width = grid.map(|(_, width)| width).or_else(|| {
                    row.then(|| crate::remaining_row_width(children, constraints.width, spacing))
                        .flatten()
                });
                for (position, child) in children.iter_mut().enumerate() {
                    let part = if let Some(key) = &child.style.key {
                        if !keys.insert(key.clone()) {
                            return Err(RuntimeError::DuplicateKey(key.clone()));
                        }
                        format!("key:{key}")
                    } else if let Some(site) = child.style.source_site {
                        let occurrence = occurrences.entry(site).or_default();
                        let part = format!("site:{site}:{}", *occurrence);
                        *occurrence += 1;
                        part
                    } else {
                        format!("position:{position}")
                    };
                    let child_id = segment(&id, &part);
                    row_ids.insert(child_id.clone());
                    self.resolve(
                        child,
                        child_id,
                        &environment,
                        owner.clone(),
                        Constraints {
                            width: child_width
                                .filter(|_| {
                                    child.style.pin.is_none()
                                        && (grid.is_some() || child.style.fill)
                                })
                                .unwrap_or(constraints.width),
                            ..constraints
                        },
                        seen,
                        mounts,
                        depth + 1,
                    )?;
                }
                if let Some(chrome) = navigation_chrome
                    && let Some(pages) = children.first()
                    && let Some(mut bar) = chrome.build(pages)
                {
                    self.resolve(
                        &mut bar,
                        segment(&id, "navigation-bar"),
                        &environment,
                        owner,
                        constraints,
                        seen,
                        mounts,
                        depth + 1,
                    )?;
                    children.insert(0, bar);
                }
                if list && let Some(previous) = self.lists.insert(id, row_ids.clone()) {
                    for removed in previous.difference(&row_ids) {
                        self.purge(removed);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    // Keep reactive construction off the recursive traversal’s stack frames.
    #[inline(never)]
    fn prepare_boundary(
        &mut self,
        boundary_id: &str,
        recipe: &ReactiveView,
        environment: &Environment,
        width: f32,
    ) -> Result<Boundary, RuntimeError> {
        let previous = self.boundaries.remove(boundary_id);
        if let Some(previous) = &previous {
            previous.scope.set_width(width);
        }
        let needs_build = previous.as_ref().is_none_or(|previous| {
            !previous.active
                || !Rc::ptr_eq(&previous.recipe.factory, &recipe.factory)
                || *previous.scope.0.environment.borrow() != *environment
        }) || self.wake.dirty.borrow_mut().remove(boundary_id);
        let mut boundary = if let Some(previous) = previous {
            previous
        } else {
            let wake = Rc::downgrade(&self.wake);
            let target = boundary_id.to_owned();
            let callback: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(wake) = wake.upgrade() {
                    wake.dirty.borrow_mut().insert(target.clone());
                    wake.schedule();
                }
            });
            Boundary {
                recipe: recipe.clone(),
                scope: Scope::new(environment.clone()),
                output: ViewNode::new(ViewKind::Group(Vec::new())),
                callback,
                subscriptions: Vec::new(),
                active: true,
            }
        };
        *boundary.scope.0.environment.borrow_mut() = environment.clone();
        boundary.scope.set_width(width);
        if needs_build {
            // Remove only the old invalidation; writes during this build remain dirty.
            self.wake.dirty.borrow_mut().remove(boundary_id);
            let (output, signals) = observation::track(|| (recipe.factory)(&boundary.scope));
            match output {
                Ok(output) => boundary.output = output,
                Err(error) => {
                    self.wake.dirty.borrow_mut().insert(boundary_id.to_owned());
                    self.boundaries.insert(boundary_id.to_owned(), boundary);
                    return Err(error);
                }
            }
            boundary.subscriptions = signals
                .iter()
                .map(|signal| signal.subscribe(&boundary.callback))
                .collect();
            boundary.recipe = recipe.clone();
            self.rebuilt.push(recipe.name.to_string());
        }
        boundary.active = true;
        Ok(boundary)
    }
    fn purge(&mut self, prefix: &str) {
        let prefix = format!("{prefix}/");
        self.boundaries.retain(|id, _| !id.starts_with(&prefix));
        self.lists.retain(|id, _| !id.starts_with(&prefix));
    }
    fn reconcile_mounts(&mut self, mounts: Vec<(String, Style, TaskContext)>) {
        let seen = mounts
            .iter()
            .map(|(id, _, _)| id.clone())
            .collect::<HashSet<_>>();
        let mut removed = self
            .mounted
            .keys()
            .filter(|id| !seen.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        removed.sort_by_key(|id| std::cmp::Reverse(id.len()));
        for id in removed {
            if let Some(old) = self.mounted.remove(&id) {
                old.task.cancel();
                if let Some(cleanup) = old.cleanup {
                    cleanup.invoke();
                }
                if let Some(disappear) = old.style.on_disappear {
                    disappear.invoke();
                }
            }
        }
        for (id, style, task) in mounts {
            let mut old = self.mounted.remove(&id);
            let first = old.is_none();
            let old_key = old
                .as_ref()
                .and_then(|old| old.style.effect.as_ref().map(|effect| &effect.key));
            let new_key = style.effect.as_ref().map(|effect| &effect.key);
            let same_effect = match (old_key, new_key) {
                (Some(old), Some(new)) => old.same(new.as_ref()),
                (None, None) => true,
                _ => false,
            };
            let cleanup = if !same_effect {
                if let Some(cleanup) = old.as_mut().and_then(|old| old.cleanup.take()) {
                    cleanup.invoke();
                }
                style.effect.as_ref().map(|effect| (effect.setup)())
            } else {
                old.as_mut().and_then(|old| old.cleanup.take())
            };
            self.mounted.insert(
                id,
                Mounted {
                    style: style.clone(),
                    cleanup,
                    task: task.clone(),
                },
            );
            if first {
                if let Some(appear) = style.on_appear {
                    appear.invoke();
                }
                if let Some(action) = &style.mount_task {
                    task.spawn(action, style.on_error);
                }
            }
        }
    }
}
fn segment(parent: &str, part: &str) -> String {
    format!("{parent}/{}:{part}", part.len())
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.wake.alive.set(false);
        let mut mounts = self.mounted.drain().collect::<Vec<_>>();
        mounts.sort_by_key(|(id, _)| std::cmp::Reverse(id.len()));
        for (_, mounted) in mounts {
            mounted.task.cancel();
            if let Some(cleanup) = mounted.cleanup {
                cleanup.invoke();
            }
            if let Some(disappear) = mounted.style.on_disappear {
                disappear.invoke();
            }
        }
        self.root_scope.0.cancel_tasks();
        for boundary in self.boundaries.values() {
            boundary.scope.0.cancel_tasks();
        }
    }
}
