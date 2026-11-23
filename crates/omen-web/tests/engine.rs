use omen_ui::{view, *};
use omen_web::*;
use std::{cell::Cell, rc::Rc};
fn find<'a>(node: &'a DomNode, text: &str) -> Option<&'a DomNode> {
    if node.text == text
        || node
            .attributes
            .get("aria-label")
            .is_some_and(|label| label == text)
    {
        Some(node)
    } else {
        node.children.iter().find_map(|node| find(node, text))
    }
}
view! {Counter(){@State let count=0;Layout{Text("Count: {count}") Button("Increment"){count+=1;} Text("Gesture").onLongPress{count+=10;}.onSwipe(SwipeDirection::Left,||{count+=100;})}}}
#[test]
fn scene_dispatch_reuses_state_and_rejects_stale_or_wrong_event_types() {
    let mut engine = WebEngine::new(view! {Counter()});
    let scene = engine.render(Viewport::default(), false, false).unwrap();
    let button = find(&scene.root, "Increment").unwrap().id.clone();
    let text = find(&scene.root, "Count: 0").unwrap().id.clone();
    assert!(!engine.dispatch(BrowserEvent::Activate { id: text }));
    assert!(engine.dispatch(BrowserEvent::Activate { id: button }));
    let scene = engine.render(Viewport::default(), false, false).unwrap();
    assert!(find(&scene.root, "Count: 1").is_some());
    let gesture = find(&scene.root, "Gesture").unwrap().id.clone();
    assert!(engine.dispatch(BrowserEvent::Gesture {
        id: gesture,
        gesture: "swipe_left".into()
    }));
    let scene = engine.render(Viewport::default(), false, false).unwrap();
    assert!(find(&scene.root, "Count: 101").is_some());
    assert!(!engine.dispatch(BrowserEvent::Activate {
        id: "unmounted".into()
    }));
}
#[test]
fn host_background_suspends_actions_and_preserves_state_on_resume() {
    let mut engine = WebEngine::new(view! {Counter()});
    let scene = engine.render(Viewport::default(), false, false).unwrap();
    let id = find(&scene.root, "Increment").unwrap().id.clone();
    engine.dispatch(BrowserEvent::Activate { id: id.clone() });
    engine.set_lifecycle(AppLifecycle::Background).unwrap();
    assert!(!engine.dispatch(BrowserEvent::Activate { id }));
    engine.set_lifecycle(AppLifecycle::Foreground).unwrap();
    let scene = engine.render(Viewport::default(), false, false).unwrap();
    assert!(find(&scene.root, "Count: 1").is_some());
}
#[test]
fn semantics_form_properties_and_theme_tokens_survive_the_backend_boundary() {
    let value = Value::new("secret".to_string());
    let checked = Value::new(true);
    let mut engine = WebEngine::new(
        view! {Layout{SecureField("Password").value(value.clone()) Toggle("Enabled").checked(checked.clone()) Text("Heading").accessibilityRole(.heading)}.theme(.light).direction(.rightToLeft)},
    );
    let scene = engine.render(Viewport::default(), true, true).unwrap();
    let field = find(&scene.root, "Password").unwrap();
    assert_eq!(field.tag, "input");
    assert_eq!(field.attributes["type"], "password");
    assert!(!field.attributes.values().any(|value| value == "secret"));
    assert_eq!(field.value.as_deref(), Some("secret"));
    assert_eq!(
        find(&scene.root, "Enabled").unwrap().attributes["role"],
        "switch"
    );
    assert_eq!(
        find(&scene.root, "Heading").unwrap().attributes["role"],
        "heading"
    );
    assert_eq!(scene.root.attributes["dir"], "rtl");
    assert_eq!(scene.root.style["background"], "#f8fafc");
    assert!(engine.dispatch(BrowserEvent::Input {
        id: field.id.clone(),
        value: "edited".into()
    }));
    assert_eq!(value.get(), "edited");
}
#[test]
fn mount_preparation_has_no_appearance_side_effects() {
    let calls = Rc::new(Cell::new(0));
    let log = calls.clone();
    let root = view! {Text("Prepared").onAppear{log.set(log.get()+1);}}.into_node();
    let mut runtime = Runtime::new(root);
    runtime.prepare().unwrap();
    assert_eq!(calls.get(), 0);
    runtime.render().unwrap();
    assert_eq!(calls.get(), 1);
}
#[test]
fn safe_area_edges_are_consumed_once_and_reduced_motion_has_no_animation() {
    let root = view! {Layout{Text("Child").safeArea(.all).transition(.fade)}.safeArea(.all)};
    let mut engine = WebEngine::new(root);
    let scene = engine
        .render(
            Viewport {
                safe_area: Insets {
                    top: 24.,
                    bottom: 16.,
                    ..Default::default()
                },
                ..Default::default()
            },
            false,
            true,
        )
        .unwrap();
    assert_eq!(scene.root.style["padding-top"], "24px");
    let child = find(&scene.root, "Child").unwrap();
    assert_eq!(child.style["padding-top"], "0px");
    assert!(child.motion.is_none());
}

#[test]
fn buttons_fit_content_and_explicit_frames_override_the_default() {
    let root = view! {Layout {
        Button("Fit") {}.padding(12)
        Button("Fixed") {}.frame(240,44)
        Button("Full") {}.frame(.infinity,44)
    }};
    let mut engine = WebEngine::new(root);
    let scene = engine.render(Viewport::default(), false, false).unwrap();
    assert_eq!(
        find(&scene.root, "Fit").unwrap().style["width"],
        "fit-content"
    );
    assert_eq!(find(&scene.root, "Fixed").unwrap().style["width"], "240px");
    assert_eq!(find(&scene.root, "Full").unwrap().style["width"], "100%");
    assert_eq!(find(&scene.root, "Full").unwrap().style["height"], "44px");
}

#[test]
fn gap_uses_the_responsive_parent_axis() {
    let mut engine = WebEngine::new(view! {
        Layout { Gap(18).accessibilityLabel("Spacing") }.horizontal(.medium)
    });
    let phone = engine
        .render(
            Viewport {
                width: 390.,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
    let gap = find(&phone.root, "Spacing").unwrap();
    assert_eq!(gap.style.get("height").map(String::as_str), Some("18px"));
    assert!(!gap.style.contains_key("width"));

    let desktop = engine
        .render(
            Viewport {
                width: 1200.,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
    let gap = find(&desktop.root, "Spacing").unwrap();
    assert_eq!(gap.style.get("width").map(String::as_str), Some("18px"));
    assert!(!gap.style.contains_key("height"));
}

#[test]
fn hex_colors_render_with_alpha_and_keep_theme_tokens() {
    let mut engine = WebEngine::new(view! {
        Layout {
            Text("Swatch")
                .textColor("#123")
                .background("#fa08")
                .hoverBackground("aabbcc")
            Text("Gradient")
                .textcolor("#123456")
                .gradient(.red, "#11223380", 45)
        }
    });
    let scene = engine.render(Viewport::default(), false, false).unwrap();
    let swatch = find(&scene.root, "Swatch").unwrap();
    assert_eq!(swatch.style["color"], "#112233");
    assert_eq!(swatch.style["background"], "#ffaa0088");
    assert_eq!(swatch.style["--omen-hover-background"], "#aabbcc");
    assert!(swatch.attributes.contains_key("data-omen-hover-background"));

    let gradient = find(&scene.root, "Gradient").unwrap();
    assert_eq!(gradient.style["color"], "#123456");
    assert_eq!(
        gradient.style["background"],
        "linear-gradient(45deg, #ef4444, #11223380)"
    );
}

#[test]
fn adaptive_direction_and_grid_use_nested_content_width() {
    let root = view! {
        Layout {
            Layout { Text("Direction") }
                .direction(.adaptive(compact: .vertical, expanded: .horizontal))
            AdaptiveGrid { Text("A") Text("B") Text("C") }
                .min_item_width(200).spacing(12)
        }.frame(460, 300).padding(20)
    };
    let mut engine = WebEngine::new(root);
    let viewport = Viewport {
        width: 1400.,
        ..Viewport::default()
    };
    let scene = engine.render(viewport, false, false).unwrap();
    assert_eq!(scene.root.children[0].style["flex-direction"], "column");
    assert_eq!(
        scene.root.children[1].style["grid-template-columns"],
        "repeat(2, minmax(0, 1fr))"
    );
}
