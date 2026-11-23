// File name: home_view.rs
// App name: website
// Created at: 2026-11-23T02:52:59Z
// Created by: Siva

use omen::prelude::*;

view! {
    @Design
    pub Home {
        Layout {
            Text("Hello, Omen!")
                .font(.largeTitle)
                .weight(.bold)

            Button("Get Started") {
                println!("Welcome to Omen!");
            }
            .padding(16)
            .background(.blue)
            .color(.white)
        }
        .spacing(.lg)
        .padding(.xl)
        .safeArea(.all)
    }
}
