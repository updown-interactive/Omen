use omen_ui::{Action, Observable, Runtime, View, ViewExt, ViewKind, ViewNode, view};
use std::{cell::Cell, rc::Rc};

fn children(node: &ViewNode) -> &[ViewNode] {
    match &node.kind {
        ViewKind::Layout(c) | ViewKind::Group(c) | ViewKind::List(c) | ViewKind::Card(c) => c,
        _ => &[],
    }
}
fn find<'a>(node: &'a ViewNode, label: &str) -> &'a ViewNode {
    if matches!(&node.kind, ViewKind::Button { label: l, .. } | ViewKind::Text(l) | ViewKind::TextField {label: l, ..} | ViewKind::Toggle {label: l, ..} if l == label)
    {
        return node;
    }
    children(node)
        .iter()
        .find_map(|child| search(child, label))
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn search<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
    if matches!(&node.kind, ViewKind::Button { label: l, .. } | ViewKind::Text(l) | ViewKind::TextField {label: l, ..} | ViewKind::Toggle {label: l, ..} if l == label)
    {
        Some(node)
    } else {
        children(node).iter().find_map(|child| search(child, label))
    }
}
fn click(node: &ViewNode, label: &str) {
    let node = find(node, label);
    if let Some(action) = &node.style.on_tap {
        action.invoke();
    } else if let ViewKind::Button { action, .. } = &node.kind {
        action.invoke();
    } else {
        panic!("not actionable")
    }
}

view! {
    Counter(label: String) {
        @State let count = 0;
        Layout {
            Text(format!("{label}:{count}"))
            Button(label.clone()) { count = count + 1; }
        }
    }
}
view! {
    Playground() {
        @State let visible = true;
        @State let rows = vec![1,2,3];
        @State let name = String::from("initial");
        Layout {
            Toggle("visible").checked($visible)
            TextField("name").value($name)
            Text("Name: {name}")
            if visible { Counter(label: "conditional") }
            Button("reverse") { rows.reverse(); }
            Button("remove") { rows.remove(0); }
            Button("insert") { rows.insert(0,3); }
            List(rows, id: |id| *id) { id in Counter(label: format!("row{id}")) }
        }
    }
}
#[test]
fn state_bindings_targeted_updates_and_conditional_identity() {
    let mut runtime = Runtime::new(view! { Playground() }.into_node());
    let root = runtime.render().unwrap();
    click(&root, "conditional");
    let root = runtime.render().unwrap();
    find(&root, "conditional:1");
    assert_eq!(runtime.last_rebuilt().len(), 1);
    assert!(runtime.last_rebuilt()[0].ends_with("Counter"));
    find(&root, "name")
        .style
        .text_value
        .as_ref()
        .unwrap()
        .set("edited".into());
    let root = runtime.render().unwrap();
    find(&root, "Name: edited");
    find(&root, "visible")
        .style
        .checked
        .as_ref()
        .unwrap()
        .set(false);
    let root = runtime.render().unwrap();
    assert!(search(&root, "conditional:1").is_none());
    find(&root, "visible")
        .style
        .checked
        .as_ref()
        .unwrap()
        .set(true);
    let root = runtime.render().unwrap();
    find(&root, "conditional:1");
}
#[test]
fn keyed_rows_retain_state_on_reorder_and_reset_after_removal() {
    let mut runtime = Runtime::new(view! { Playground() }.into_node());
    let root = runtime.render().unwrap();
    click(&root, "row3");
    let root = runtime.render().unwrap();
    click(&root, "reverse");
    let root = runtime.render().unwrap();
    find(&root, "row3:1");
    click(&root, "remove");
    let root = runtime.render().unwrap();
    assert!(search(&root, "row3:1").is_none());
    click(&root, "insert");
    let root = runtime.render().unwrap();
    find(&root, "row3:0");
}
#[derive(Clone)]
struct Model {
    name: String,
}
view! {
    Editor() {
        @Environment let model: Model;
        Layout { TextField("model").value($model.name) Text(model.name) }
    }
}
#[test]
fn shared_environment_projection_and_missing_model_error() {
    let model = Observable::new(Model {
        name: "before".into(),
    });
    let mut runtime = Runtime::new(
        view! { Layout { Editor() Editor() } }
            .environment(model.clone())
            .into_node(),
    );
    let root = runtime.render().unwrap();
    find(&root, "model")
        .style
        .text_value
        .as_ref()
        .unwrap()
        .set("after".into());
    let root = runtime.render().unwrap();
    assert_eq!(model.get().name, "after");
    find(&root, "after");
    assert_eq!(runtime.last_rebuilt().len(), 2);
    assert!(
        Runtime::new(view! { Editor() }.into_node())
            .render()
            .is_err()
    );
}
view! {
    Lifecycle(log: Action) {
        @State let dependency = 0;
        let appear = log.clone();
        let disappear = log.clone();
        Layout { Button("change") { dependency += 1; } }
            .onAppear { appear.invoke(); }
            .onDisappear { disappear.invoke(); }
            .effect(dependency, move || { log.invoke(); log.clone() })
    }
}
#[test]
fn effects_cleanup_on_dependency_change_and_runtime_drop() {
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut runtime = Runtime::new(
        view! { Lifecycle(log: Action::new(move || observed.set(observed.get()+1))) }.into_node(),
    );
    let root = runtime.render().unwrap();
    assert_eq!(calls.get(), 2);
    click(&root, "change");
    runtime.render().unwrap();
    assert_eq!(calls.get(), 4);
    drop(runtime);
    assert_eq!(calls.get(), 6);
}

view! {
    AsyncView() {
        @State let finished = 0;
        @State let error = String::new();
        Layout {
            Text("Finished: {finished}")
            Text(error)
            Button("async") async { finished += 1; }
            Button("fail") async { Err::<(), _>("failed request") }.onError(|message| { error = message; })
            Button("pending") async { futures::future::pending::<()>().await; finished += 100; }
        }
    }
}
fn spawn(node: &ViewNode, label: &str) -> omen_ui::TaskHandle {
    let node = find(node, label);
    node.style.task_context.as_ref().unwrap().spawn(
        node.style.async_action.as_ref().unwrap(),
        node.style.on_error.clone(),
    )
}
#[test]
fn async_actions_finish_report_errors_and_cancel_previous_invocations() {
    let mut runtime = Runtime::new(view! { AsyncView() }.into_node());
    let root = runtime.render().unwrap();
    let completed = spawn(&root, "async");
    for task in runtime.take_tasks() {
        futures::executor::block_on(task.run());
    }
    assert_eq!(completed.status(), omen_ui::TaskStatus::Succeeded);
    let root = runtime.render().unwrap();
    find(&root, "Finished: 1");
    let failed = spawn(&root, "fail");
    for task in runtime.take_tasks() {
        futures::executor::block_on(task.run());
    }
    assert_eq!(
        failed.status(),
        omen_ui::TaskStatus::Failed("failed request".into())
    );
    let root = runtime.render().unwrap();
    find(&root, "failed request");
    let old = spawn(&root, "pending");
    let current = spawn(&root, "pending");
    assert_eq!(old.status(), omen_ui::TaskStatus::Cancelled);
    current.cancel();
    for task in runtime.take_tasks() {
        futures::executor::block_on(task.run());
    }
    assert_eq!(current.status(), omen_ui::TaskStatus::Cancelled);
    let root = runtime.render().unwrap();
    find(&root, "Finished: 1");
}
#[test]
fn dropping_a_runtime_cancels_already_polled_work() {
    use futures::{FutureExt, task::LocalSpawnExt};
    let mut runtime = Runtime::new(view! { AsyncView() }.into_node());
    let root = runtime.render().unwrap();
    let handle = spawn(&root, "pending");
    let mut executor = futures::executor::LocalPool::new();
    for task in runtime.take_tasks() {
        executor
            .spawner()
            .spawn_local(task.run().boxed_local())
            .unwrap();
    }
    executor.run_until_stalled();
    assert_eq!(handle.status(), omen_ui::TaskStatus::Running);
    drop(runtime);
    executor.run_until_stalled();
    assert_eq!(handle.status(), omen_ui::TaskStatus::Cancelled);
}
#[test]
fn independent_runtime_instances_do_not_share_state() {
    let mut first = Runtime::new(view! { Counter(label:"test") }.into_node());
    let mut second = Runtime::new(view! { Counter(label:"test") }.into_node());
    click(&first.render().unwrap(), "test");
    find(&first.render().unwrap(), "test:1");
    find(&second.render().unwrap(), "test:0");
}

#[test]
fn dependencies_are_replaced_and_wakeups_are_coalesced() {
    let choose = Observable::new(true);
    let left = Observable::new(1);
    let right = Observable::new(10);
    let (c, l, r) = (choose.clone(), left.clone(), right.clone());
    let root = ViewNode::new(ViewKind::Reactive(omen_ui::ReactiveView::new(
        "Dynamic",
        move |_| {
            Ok(omen_ui::Text::new(if c.get() {
                l.get().to_string()
            } else {
                r.get().to_string()
            })
            .into_node())
        },
    )));
    let mut runtime = Runtime::new(root);
    let wakes = Rc::new(Cell::new(0));
    let observed = wakes.clone();
    runtime.set_waker(move || observed.set(observed.get() + 1));
    runtime.render().unwrap();
    left.set(2);
    left.set(3);
    assert_eq!(wakes.get(), 1);
    find(&runtime.render().unwrap(), "3");
    choose.set(false);
    find(&runtime.render().unwrap(), "10");
    let previous = wakes.get();
    left.set(4);
    assert_eq!(wakes.get(), previous);
    right.set(11);
    find(&runtime.render().unwrap(), "11");
}
#[test]
fn duplicate_list_keys_return_an_error() {
    let root =
        view! { List([1,1], id: |id| *id) { id in Counter(label:format!("{id}")) } }.into_node();
    assert!(matches!(
        Runtime::new(root).render(),
        Err(omen_ui::RuntimeError::DuplicateKey(_))
    ));
}

view! {
    MountedTask() {
        Text("mounted").task async { futures::future::pending::<()>().await; }
    }
}
view! {
    TaskOwner() {
        @State let visible = true;
        Layout {
            Toggle("visible").checked($visible)
            if visible { MountedTask() }
        }
    }
}
#[test]
fn mount_tasks_start_once_and_cancel_when_their_node_is_hidden() {
    let mut runtime = Runtime::new(view! { TaskOwner() }.into_node());
    let root = runtime.render().unwrap();
    let jobs = runtime.take_tasks();
    assert_eq!(jobs.len(), 1);
    runtime.render().unwrap();
    assert!(runtime.take_tasks().is_empty());
    let mut executor = futures::executor::LocalPool::new();
    use futures::task::LocalSpawnExt;
    let completed = Rc::new(Cell::new(false));
    let observed = completed.clone();
    let task = jobs.into_iter().next().unwrap();
    executor
        .spawner()
        .spawn_local(async move {
            task.run().await;
            observed.set(true);
        })
        .unwrap();
    executor.run_until_stalled();
    assert!(!completed.get());
    find(&root, "visible")
        .style
        .checked
        .as_ref()
        .unwrap()
        .set(false);
    let root = runtime.render().unwrap();
    executor.run_until_stalled();
    assert!(completed.get());
    find(&root, "visible")
        .style
        .checked
        .as_ref()
        .unwrap()
        .set(true);
    runtime.render().unwrap();
    assert_eq!(runtime.take_tasks().len(), 1);
}
view! {
    CollectionMutations() {
        @State let values = vec![1, 2];
        @State let count = 2;
        Layout {
            Text("Values: {values:?}")
            Text("Padded: {count:03}")
            Button("append") { values.push(values.len() as i32); count += count; }
        }
    }
}
#[test]
fn same_state_rhs_collection_reads_and_debug_captures_are_safe() {
    let mut runtime = Runtime::new(view! { CollectionMutations() }.into_node());
    let root = runtime.render().unwrap();
    find(&root, "Values: [1, 2]");
    find(&root, "Padded: 002");
    click(&root, "append");
    let root = runtime.render().unwrap();
    find(&root, "Values: [1, 2, 2]");
    find(&root, "Padded: 004");
}

view! {
    BoundOnly() {
        @Environment let model: Model;
        TextField("bound").value($model.name)
    }
}
#[test]
fn binding_only_views_observe_programmatic_model_writes() {
    let model = Observable::new(Model {
        name: "before".into(),
    });
    let mut runtime = Runtime::new(view! { BoundOnly() }.environment(model.clone()).into_node());
    let wakes = Rc::new(Cell::new(0));
    let observed = wakes.clone();
    runtime.set_waker(move || observed.set(observed.get() + 1));
    runtime.render().unwrap();
    model.update(|value| value.name = "after".into());
    assert_eq!(wakes.get(), 1);
    let root = runtime.render().unwrap();
    assert_eq!(
        find(&root, "bound")
            .style
            .text_value
            .as_ref()
            .unwrap()
            .get(),
        "after"
    );
    assert_eq!(runtime.last_rebuilt().len(), 1);
}

view! {
    ShadowedState() {
        @State let count = 1;
        Layout {
            Text("Count: {count}")
            Button("shadow") {
                for count in [2,3] { assert!(count > 0); }
                if let Some(count) = Some(5) { assert_eq!(count,5); }
                match Some(7) { Some(count) => assert_eq!(count,7), None => panic!("expected a value") }
                let increment = |count| count + 1;
                count += increment(1);
            }
        }
    }
}
#[test]
fn action_patterns_can_shadow_state_names() {
    let mut runtime = Runtime::new(view! { ShadowedState() }.into_node());
    click(&runtime.render().unwrap(), "shadow");
    find(&runtime.render().unwrap(), "Count: 3");
}

#[test]
fn desktop_toolbar_shares_environment_and_reacts_without_replacing_body_state() {
    use omen_ui::{DesktopToolbar, DesktopWindow, ViewExt};
    #[derive(Clone)]
    struct ToolbarModel {
        count: omen_ui::Value<i32>,
    }
    view! {
        ToolbarContent {
            @Environment model: ToolbarModel
            Button(format!("Toolbar {}", model.count.get())) { model.count.update(|count| *count += 1); }
        }
    }
    view! {
        ToolbarBody {
            @Environment model: ToolbarModel
            Text(format!("Body {}", model.count.get()))
        }
    }
    fn find<'a>(node: &'a omen_ui::ViewNode, label: &str) -> Option<&'a omen_ui::ViewNode> {
        match &node.kind {
            omen_ui::ViewKind::Button { label: text, .. } | omen_ui::ViewKind::Text(text)
                if text == label =>
            {
                Some(node)
            }
            omen_ui::ViewKind::Layout(children) | omen_ui::ViewKind::Group(children) => {
                children.iter().find_map(|child| find(child, label))
            }
            _ => None,
        }
    }
    let count = omen_ui::Value::new(0);
    let root = ToolbarBody {}
        .environment(omen_ui::Value::new(ToolbarModel {
            count: count.clone(),
        }))
        .desktop_window(DesktopWindow {
            toolbar: Some(DesktopToolbar::new(|| ToolbarContent {})),
            ..Default::default()
        });
    let mut runtime = omen_ui::Runtime::new(root.into_node());
    let first = runtime.render().unwrap();
    let omen_ui::ViewKind::Button { action, .. } = &find(&first, "Toolbar 0").unwrap().kind else {
        panic!("Expected toolbar action")
    };
    action.invoke();
    let next = runtime.render().unwrap();
    assert!(find(&next, "Toolbar 1").is_some());
    assert!(find(&next, "Body 1").is_some());
    assert_eq!(count.get(), 1);
}
