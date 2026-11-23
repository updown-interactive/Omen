use omen_components::{
    EmptyState, FormRow, LoadingCard, PreferenceRow, PrimaryButton, SectionHeader, StatusBadge,
};
use omen_ui::{Action, BoolValue, TextValue, View, ViewKind, view};

#[test]
fn library_views_compose_with_named_properties_without_a_renderer() {
    let root = view! { Layout {
        SectionHeader(title: "Title", subtitle: "Subtitle")
        StatusBadge(label: "Ready")
        PrimaryButton(title: "Save", action: Action::new(|| {}))
        EmptyState(title: "Empty", message: "No items")
        LoadingCard(title: "Loading", fraction: 0.5_f32)
        FormRow(label: "Name", value: TextValue::new(String::new()))
        PreferenceRow(title: "Notify", value: BoolValue::new(true))
    } }
    .into_node();
    assert!(matches!(root.kind, ViewKind::Layout(children) if children.len() == 7));
}
