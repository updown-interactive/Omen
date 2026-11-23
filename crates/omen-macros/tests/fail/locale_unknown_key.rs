use omen_ui::*;
locale! { Strings { default: "en" en { welcome: "Welcome" } } }
view! { Home { @Localize strings = Strings() Text(strings.missing) } }
fn main() {}
