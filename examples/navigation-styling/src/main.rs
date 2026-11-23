use omen::prelude::*;

view! {
    Detail(title: String) {
        @State let count = 0
        Layout {
            Text(title.clone()).font(.title)
            Text("Local count: {count}")
            Button("Increment detail") { count += 1; }.padding(.md)
        }.spacing(.lg).padding(.lg).transition(.slide)
        .navigationTitle(title)
    }
}
view! {
    Home {
        @State let navigator = Navigator()
        @State let tab = "overview"
        @State let dark = true
        @State let sheet = false
        @State let dialog = false
        @State let popover = false
        @State let modal = false
        @State let shifted = false
        @Localize strings {
            en { welcome: "Navigation, styling and motion" }
            ar { welcome: "التنقل والتصميم والحركة" }
        }
        @Environment viewport
        NavigationStack($navigator) {
            Layout {
                Text(strings.welcome).font(.largeTitle)
                Text("Viewport: {viewport.breakpoint} · {viewport.width} × {viewport.height}").font(.caption).color(.secondary)
                Layout {
                    Button("Toggle theme") { dark.toggle() }.padding(.md)
                    Button("English") { strings.locale = "en" }.padding(.md)
                    Button("العربية") { strings.locale = "ar" }.padding(.md)
                }.spacing(.md).horizontal(.medium)
                TabView($tab) {
                    Tab("overview", "Overview") {
                        Layout {
                            Button("Open detail") { navigator.push(Detail(title: "Detail")) }.padding(.md).background(.accent).color(.white).cornerRadius(8)
                            Button("Move card") { shifted.toggle() }.padding(.md)
                            Card { Text("Spring motion").font(.headline) Text("Opacity and position animate when state changes") }
                                .offset(x: if shifted { 64. } else { 0. })
                                .opacity(if shifted { 0.6 } else { 1. })
                                .animation(.spring(800.ms))
                        }.spacing(.lg)
                    }
                    Tab("presentations", "Presentations") {
                        Layout {
                            Button("Show sheet") { sheet = true; }.padding(.md)
                            Button("Show dialog") { dialog = true; }.padding(.md)
                            Button("Show popover") { popover = true; }.padding(.md)
                            Button("Show modal") { modal = true; }.padding(.md)
                        }.spacing(.md)
                    }
                }
            }.spacing(.lg).padding(.xl).navigationTitle("Omen Phase 5")
        }
        .sheet($sheet) { Layout { Text("Sheet").font(.title) Text("Press Escape or click the backdrop to dismiss") }.spacing(.lg) }
        .dialog($dialog) { Layout { Text("Dialog").font(.title) Text("Keyboard focus stays inside this presentation") }.spacing(.lg) }
        .popover($popover) { Text("A nonblocking anchored popover").padding(.lg) }
        .modal($modal) { Layout { Text("Modal").font(.title) Text("Press Escape to return focus to the opener") }.spacing(.lg) }
        .theme(if dark { .dark } else { .light })
        .locale(strings.locale)
        .safeArea(.all)
    }
}
main! { Home() }
