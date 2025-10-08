use dioxus::prelude::*;
use ui::MigrationService;

const FAVICON: Asset = asset!("/assets/favicon.png");
const MAIN_CSS: Asset = asset!("/assets/main.css");
// Banner image is referenced in index.html meta tags and bundled automatically
const _BANNER: Asset = asset!("/assets/banner-1456-1000.png");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        // Global app resources
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        // Note: Title and meta tags are set in web/index.html for SEO/crawler support

        Router::<Route> {}
    }
}

#[derive(Clone, Routable, Debug, PartialEq)]
enum Route {
    #[route("/")]
    Home {},
}

#[component]
fn Home() -> Element {
    rsx! {
        div {
            MigrationService {}
        }
    }
}
