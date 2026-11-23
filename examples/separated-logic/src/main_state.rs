use omen::prelude::*;

state! {
    pub TaskState {
        tab: String = "overview"
        name: String = "Omen"
        count = 0

        pub fn increment(&mut self) {
            self.count += 1;
        }
    }
}
