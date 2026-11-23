use omen::prelude::*;

fn find<'a>(node: &'a ViewNode, text: &str) -> Option<&'a ViewNode> {
    if matches!(&node.kind, ViewKind::Text(label) | ViewKind::Button { label, .. } if label == text) {
        return Some(node);
    }
    match &node.kind {
        ViewKind::Layout(children)
        | ViewKind::Group(children)
        | ViewKind::Card(children)
        | ViewKind::Presentation { children, .. } => {
            children.iter().find_map(|child| find(child, text))
        }
        _ => None,
    }
}

view! {
    ProductCatalog {
        Layout {
            Text("Widget")
            Text(1234.50).format(.currency("USD"))
            Text(0.25).format(.percent())
            Text(0.15).format(.percent)
            Text("2026-11-23").format(.date(.medium))
            Text("2026-11-23T14:30:00").format(.datetime(.long).time_zone("Asia/Kolkata"))
            Text(1250000).format(.number(.compact))
            Text(5000).format(.measurement(.kilometer))
            Text(1048576).format(.bytes(.binary))
            Text(3665).format(.duration(.clock))
            Text(3660).format(.duration(.human))
        }
    }
}

#[test]
fn test_declarative_view_formatting() {
    let mut runtime = Runtime::new(view! { ProductCatalog() }.into_node());
    let root = runtime.render().unwrap();

    assert!(find(&root, "Widget").is_some());
    // Currency USD
    assert!(find(&root, "$1,234.50").is_some());
    // Percent () and bare
    assert!(find(&root, "25%").is_some());
    assert!(find(&root, "15%").is_some());
    // Date medium
    assert!(find(&root, "Nov 23, 2026").is_some());
    // Number compact (HalfEven rounds 1.25 to nearest even -> 1.2M)
    assert!(find(&root, "1.2M").is_some());
    // Duration clock: 3665 sec = 01:01:05
    assert!(find(&root, "01:01:05").is_some());
    // Duration human: 1 hour, 1 minute
    assert!(find(&root, "1 hour, 1 minute").is_some());
}

view! {
    LocalizedPrices {
        @State german = false
        Layout {
            Text(1234.50).format(.currency("EUR"))
            Button("SwitchLocale") { german = true }
        }
        .format_context(if german {
            FormatContext::new(LocaleId::new("de-DE"))
        } else {
            FormatContext::new(LocaleId::new("en-US"))
        })
    }
}

#[test]
fn test_reactive_locale_formatting_environment() {
    let mut runtime = Runtime::new(view! { LocalizedPrices() }.into_node());
    let root = runtime.render().unwrap();

    // In US English: €1,234.50
    assert!(find(&root, "€1,234.50").is_some());

    // Switch to German: 1.234,50\u{00A0}€
    let ViewKind::Button { action, .. } = &find(&root, "SwitchLocale").unwrap().kind else {
        panic!("missing button");
    };
    action.invoke();

    let root_de = runtime.render().unwrap();
    assert!(find(&root_de, "1.234,50\u{00A0}€").is_some());
}
