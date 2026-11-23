use super::{Header, TaskCard};
use omen::view;

view! {
    pub HomeView {
          Layout {
            Header(title: "My Tasks", subtitle: "Stay productive")
            TaskCard(title: "Build Omen", completed: true)
            TaskCard(title: "Implement state", completed: false)
            Button("Get Started") {
                println!("Omen is running!");
            }.padding(16).background(.blue).color(.white).cornerRadius(12)
        }.spacing(16).padding(24).background(.dark)
    }
}
