use omen_ui::{state, view, View, BoolExt};
state! {
    #[derive(Clone, Debug)]
    pub CounterState {
        count = 0
        enabled = true
        title = "Hello"
        selected: String = "home"
        opacity: f32 = 1.0
        #[cfg(any())]
        absent: Missing = Missing::new()
        pub fn increment(&mut self) {self.count += 1;}
        pub fn toggle(&mut self) {self.enabled.toggle();}
    }
}
state! { Legacy {let count = 0; @State let label = "legacy";} }
view! { Counter {
    @State state = CounterState()
    Layout {
        Text("Count: {state.count}")
        Button("Increment") {state.increment()}
        Button("Toggle") {state.toggle()}
        Button("Reset") {state.count = 0}
        Toggle("Enabled", $state.enabled)
    }
} }
fn main() {
    let mut state=CounterState::default();state.increment();assert_eq!(state.count,1);
    let _:String=state.title;let _:f32=state.opacity;
    let _=Legacy::new();let _=view!{Counter()}.into_node();
}
