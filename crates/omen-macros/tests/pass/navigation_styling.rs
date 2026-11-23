use omen_ui::{view, *};
use std::time::Duration;
view! {
    pub Screen() {
        @State let nav=Navigator::new();
        @State let selected=String::from("home");
        @State let presented=false;
        @Environment let viewport:Viewport;
        NavigationStack(nav, |route:Route| view!{Text(route.name)}) {
            TabView($selected) {
                Tab("home","Home") {Text(format!("{}",viewport.width)).navigationTitle("Home")}
                Tab("settings","Settings") {NavigationBar("Settings"){Button("Show"){presented=true;}}}
            }
        }.theme(.light).paddingToken(.large).spacingToken(.medium)
            .direction(.rightToLeft).safeArea(.all)
            .responsive(Breakpoint::Large,ResponsiveStyle{padding:Some(32.),..Default::default()})
            .transition(.fade).offset(10.,0.).opacity(0.8)
            .animation(Animation::new(Duration::from_millis(100),Easing::EaseInOut))
            .reducedMotion(false)
            .sheet($presented,||view!{Text("Sheet")})
    }
}
fn main(){let _=view!{Screen()}.into_node();}
