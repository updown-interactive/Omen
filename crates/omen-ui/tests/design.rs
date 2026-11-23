use omen_ui::{view, *};
use std::time::Duration;
#[test]
fn themes_resolve_semantic_colors_and_custom_token_overrides() {
    let light = Theme::light().resolve(false);
    let dark = Theme::dark().resolve(false);
    assert_ne!(light.color(Color::Primary), dark.color(Color::Primary));
    assert_eq!(Theme::system().resolve(true), dark);
    assert_eq!(Theme::system().resolve(false), light);
    let custom = DesignTokens {
        accent: 0xff00ff,
        spacing: [2., 6., 12., 20.],
        ..light
    };
    assert_eq!(
        Theme::light()
            .with_tokens(custom)
            .resolve(true)
            .color(Color::Accent),
        0xff00ff
    );
    assert_eq!(custom.space(Space::Large), 12.);
}
#[test]
fn responsive_rules_apply_by_width_and_override_token_padding() {
    let node=view!{Layout{Text("Responsive")}.paddingToken(.large)
        .responsive(Breakpoint::Large,ResponsiveStyle{mode:Some(LayoutMode::Horizontal),padding:Some(32.),..Default::default()})
        .responsive(Breakpoint::Medium,ResponsiveStyle{padding:Some(20.),..Default::default()})
        .safeArea(.all).safeAreaInsets(Insets{top:24.,..Default::default()})
    }.into_node();
    assert_eq!(node.style.for_width(599.).padding_token, Some(Space::Large));
    assert_eq!(node.style.for_width(600.).padding, Some(20.));
    let wide = node.style.for_width(1024.);
    assert_eq!(wide.padding, Some(32.));
    assert_eq!(wide.padding_token, None);
    assert_eq!(wide.mode, Some(LayoutMode::Horizontal));
    assert_eq!(
        Viewport {
            width: 1440.,
            ..Default::default()
        }
        .breakpoint(),
        Breakpoint::ExtraLarge
    );
}
view! {ResponsiveView(){@Environment let viewport:Viewport;Text(format!("{:?}",viewport.breakpoint()))}}
fn has_text(node: &ViewNode, text: &str) -> bool {
    match &node.kind {
        ViewKind::Text(value) => value == text,
        ViewKind::Group(c) | ViewKind::Layout(c) => c.iter().any(|node| has_text(node, text)),
        _ => false,
    }
}
#[test]
fn host_viewport_changes_invalidate_named_views() {
    let mut runtime = Runtime::new(view! {ResponsiveView()}.into_node());
    assert!(has_text(&runtime.render().unwrap(), "Medium"));
    runtime.set_viewport(Viewport {
        width: 400.,
        ..Default::default()
    });
    assert!(has_text(&runtime.render().unwrap(), "Compact"));
    assert_eq!(runtime.last_rebuilt().len(), 1);
    runtime.set_viewport(Viewport {
        width: 400.,
        ..Default::default()
    });
    runtime.render().unwrap();
    assert!(runtime.last_rebuilt().is_empty());
}
#[test]
fn localization_falls_back_and_interpolates_values_without_reinterpreting_them() {
    let strings = Localizer::new(Locale::new("ar-EG"), "en")
        .message("ar", "hello", "مرحباً {name}")
        .message("en", "missing", "Fallback");
    assert_eq!(strings.locale().direction, LayoutDirection::RightToLeft);
    assert_eq!(
        strings.format("hello", &[("name", "{literal}")]),
        "مرحباً {literal}"
    );
    assert_eq!(strings.text("missing"), "Fallback");
    assert_eq!(strings.text("unknown"), "unknown");
    strings.set_locale(Locale::new("en-US"));
    assert_eq!(strings.locale().direction, LayoutDirection::LeftToRight);
}
view! {Localized(strings:Localizer){Text(strings.text("hello")).locale(strings.locale())}}
#[test]
fn switching_shared_locale_rebuilds_localized_views() {
    let strings = Localizer::new(Locale::new("en"), "en")
        .message("en", "hello", "Hello")
        .message("ar", "hello", "مرحباً");
    let mut runtime = Runtime::new(view! {Localized(strings:strings.clone())}.into_node());
    assert!(has_text(&runtime.render().unwrap(), "Hello"));
    strings.set_locale(Locale::new("ar"));
    assert!(has_text(&runtime.render().unwrap(), "مرحباً"));
}
#[test]
fn easing_and_springs_have_stable_endpoints_and_finite_samples() {
    for easing in [
        Easing::Linear,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
    ] {
        assert_eq!(easing.sample(0.), 0.);
        assert_eq!(easing.sample(1.), 1.);
        assert!(easing.sample(0.25) <= easing.sample(0.75));
    }
    for damping in [10., 2. * 180_f32.sqrt(), 60.] {
        let spring = Spring {
            stiffness: 180.,
            damping,
        };
        assert!(spring.sample(0.).abs() < 0.0001);
        assert!((spring.sample(10.) - 1.).abs() < 0.001);
        assert!(spring.sample(0.2).is_finite());
    }
    let animation = Animation::spring(Duration::from_secs(1), Spring::default());
    assert_eq!(animation.sample(Duration::from_secs(1)), 1.);
    assert_eq!(
        Animation::new(Duration::ZERO, Easing::Linear).sample(Duration::ZERO),
        1.
    );
    let target = VisualState::default();
    assert_eq!(target.entering(Transition::Fade).opacity, 0.);
    assert_eq!(
        target.entering(Transition::Slide).interpolate(target, 1.),
        target
    );
}

view! {
    LocalResponsive {
        @Environment env
        @Responsive layout
        Layout {
            Text(env.width)
            if layout.compact {
                Text("compact")
            } else {
                Text("wide")
            }
        }
    }
}

#[test]
fn responsive_view_tracks_its_flex_share_independently_of_host_width() {
    let root = view! {
        Layout {
            Layout {}.frame(500, 100)
            LocalResponsive().fill()
        }.mode(.horizontal).spacing(20)
    };
    let mut runtime = Runtime::new(root.into_node());
    let mut viewport = Viewport {
        width: 1100.,
        ..Viewport::default()
    };
    runtime.set_viewport(viewport);
    let root = runtime.render().unwrap();
    assert!(has_text(&root, "1100"));
    assert!(has_text(&root, "compact"));
    viewport.width = 1120.;
    runtime.set_viewport(viewport);
    let root = runtime.render().unwrap();
    assert!(has_text(&root, "1120"));
    assert!(has_text(&root, "wide"));
}
