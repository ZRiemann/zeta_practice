use dioxus::prelude::*;

use crate::i18n::{Locale, messages};

const APP_CSS: &str = include_str!("../assets/app.css");

#[derive(Clone, Debug, PartialEq, Routable)]
enum Route {
    #[layout(AppShell)]
    #[route("/")]
    Home {},
    #[route("/about")]
    About {},
}

#[allow(non_snake_case)]
pub(crate) fn App() -> Element {
    use_context_provider(|| Signal::new(Locale::ZhCn));
    rsx! { Router::<Route> {} }
}

#[component]
fn AppShell() -> Element {
    let mut locale = use_context::<Signal<Locale>>();
    let selected_locale = *locale.read();
    let text = messages(selected_locale);

    rsx! {
        style { {APP_CSS} }
        div { class: "app-shell", lang: selected_locale.language_tag(),
            header { class: "topbar",
                div { class: "brand",
                    span { class: "brand-mark", "Z" }
                    span { "{text.app_name}" }
                }
                nav { aria_label: "Primary",
                    Link { to: Route::Home {}, "{text.home}" }
                    Link { to: Route::About {}, "{text.about}" }
                }
                div { class: "locale-switcher", aria_label: "{text.language}",
                    button {
                        class: if selected_locale == Locale::ZhCn { "active" } else { "" },
                        onclick: move |_| locale.set(Locale::ZhCn),
                        "中文"
                    }
                    button {
                        class: if selected_locale == Locale::EnUs { "active" } else { "" },
                        onclick: move |_| locale.set(Locale::EnUs),
                        "English"
                    }
                }
            }
            main { Outlet::<Route> {} }
            footer { "{text.stage_label}" }
        }
    }
}

#[component]
fn Home() -> Element {
    let locale = use_context::<Signal<Locale>>();
    let text = messages(*locale.read());

    rsx! {
        section { class: "hero",
            p { class: "eyebrow", "ZetaX · Practice" }
            h1 { "{text.headline}" }
            p { class: "lead", "{text.introduction}" }
            p { class: "status-card", "{text.start_hint}" }
        }
    }
}

#[component]
fn About() -> Element {
    let locale = use_context::<Signal<Locale>>();
    let text = messages(*locale.read());

    rsx! {
        section { class: "content-card",
            h1 { "{text.about_title}" }
            p { "{text.about_body}" }
        }
    }
}
