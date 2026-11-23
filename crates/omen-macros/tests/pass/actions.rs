use omen_ui::{view, View};
use std::{cell::Cell, rc::Rc};
fn main() {
    let count = Rc::new(Cell::new(0));
    let _ = view! { Button("Increment") { count.set(count.get() + 1); } }.into_node();
}
