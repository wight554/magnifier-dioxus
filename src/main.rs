use dioxus::prelude::*;

mod settings;

fn main() {
    dioxus::launch(app);
}

fn app() -> Element {
    rsx! {
        div { id: "root", "magnifier" }
    }
}
