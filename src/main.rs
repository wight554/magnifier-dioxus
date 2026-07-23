use dioxus::prelude::*;

mod camera;
mod settings;

fn main() {
    #[cfg(not(target_os = "android"))]
    env_logger::init();
    dioxus::launch(app);
}

fn app() -> Element {
    rsx! {
        div { id: "root", "magnifier" }
    }
}
