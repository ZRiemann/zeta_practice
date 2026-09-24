use dioxus::prelude::*;

use crate::auth::{self, PublicUser};
use crate::i18n::{Locale, messages};

const APP_CSS: &str = include_str!("../assets/app.css");

#[derive(Clone, Debug, PartialEq, Routable)]
enum Route {
    #[layout(AppShell)]
    #[route("/")]
    Home {},
    #[route("/about")]
    About {},
    #[route("/login")]
    Login {},
}

#[allow(non_snake_case)]
pub(crate) fn App() -> Element {
    let _locale = use_context_provider(|| Signal::new(Locale::ZhCn));
    let _account = use_context_provider(|| Signal::new(None::<PublicUser>));
    let _auth_changed = use_context_provider(|| Signal::new(false));
    #[cfg(feature = "web")]
    use_effect(move || {
        let mut locale = _locale;
        let mut account = _account;
        let auth_changed = _auth_changed;
        spawn(async move {
            if let Ok(user) = auth::current_user().await {
                if !*auth_changed.read() {
                    if let Some(ref signed_in) = user {
                        locale.set(Locale::from_language_tag(&signed_in.locale));
                    }
                    account.set(user);
                }
            }
        });
    });
    rsx! { Router::<Route> {} }
}

#[component]
fn AppShell() -> Element {
    let mut locale = use_context::<Signal<Locale>>();
    let selected_locale = *locale.read();
    let text = messages(selected_locale);
    let mut account = use_context::<Signal<Option<PublicUser>>>();
    let mut auth_changed = use_context::<Signal<bool>>();
    let mut logout_error = use_signal(|| false);
    let navigator = use_navigator();

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
                    if account.read().is_some() {
                        button {
                            class: "nav-button",
                            onclick: move |_| {
                                spawn(async move {
                                    match auth::logout().await {
                                        Ok(_) => {
                                            auth_changed.set(true);
                                            account.set(None);
                                            logout_error.set(false);
                                            navigator.push(Route::Home {});
                                        }
                                        Err(_) => logout_error.set(true),
                                    }
                                });
                            },
                            "{text.logout}"
                        }
                    } else {
                        Link { to: Route::Login {}, "{text.login}" }
                    }
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
            main {
                if logout_error() { p { class: "form-error", "{text.request_failed}" } }
                Outlet::<Route> {}
            }
            footer { "{text.stage_label}" }
        }
    }
}

#[component]
fn Home() -> Element {
    let locale = use_context::<Signal<Locale>>();
    let text = messages(*locale.read());
    let account = use_context::<Signal<Option<PublicUser>>>();

    rsx! {
        section { class: "hero",
            p { class: "eyebrow", "ZetaX · Practice" }
            h1 { "{text.headline}" }
            p { class: "lead", "{text.introduction}" }
            if let Some(user) = account.read().as_ref() {
                p { class: "status-card", "{text.welcome}{user.username}" }
            } else {
                div { class: "status-card",
                    p { "{text.start_hint}" }
                    Link { to: Route::Login {}, "{text.login}" }
                }
            }
        }
    }
}

#[component]
fn Login() -> Element {
    let mut locale = use_context::<Signal<Locale>>();
    let text = messages(*locale.read());
    let mut account = use_context::<Signal<Option<PublicUser>>>();
    let mut auth_changed = use_context::<Signal<bool>>();
    let navigator = use_navigator();
    let mut is_registering = use_signal(|| false);
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut pending = use_signal(|| false);
    let heading = if is_registering() {
        text.register
    } else {
        text.login
    };
    let hint = if is_registering() {
        text.register_hint
    } else {
        text.login_hint
    };

    rsx! {
        section { class: "content-card auth-card",
            h1 { "{heading}" }
            p { class: "lead", "{hint}" }
            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    if pending() { return; }
                    pending.set(true);
                    error.set(None);
                    let name = username();
                    let secret = password();
                    let language = locale.read().language_tag().to_string();
                    let registering = is_registering();
                    spawn(async move {
                        let result = if registering {
                            auth::register(name, secret, language).await
                        } else {
                            auth::login(name, secret).await
                        };
                        match result {
                            Ok((_, user)) => {
                                auth_changed.set(true);
                                locale.set(Locale::from_language_tag(&user.0.locale));
                                account.set(Some(user.0));
                                password.set(String::new());
                                navigator.push(Route::Home {});
                            }
                            Err(failure) => {
                                let message = match failure {
                                    ServerFnError::ServerError { message, .. } => message,
                                    _ => "request_failed".to_string(),
                                };
                                error.set(Some(message));
                            }
                        }
                        pending.set(false);
                    });
                },
                label { "{text.username}"
                    input {
                        name: "username", autocomplete: "username", required: true,
                        minlength: "3", maxlength: "32", value: "{username}",
                        oninput: move |event| username.set(event.value()),
                    }
                }
                label { "{text.password}"
                    input {
                        r#type: "password", name: "password", required: true,
                        autocomplete: if is_registering() { "new-password" } else { "current-password" },
                        value: "{password}",
                        oninput: move |event| password.set(event.value()),
                    }
                }
                if let Some(code) = error.read().as_ref() {
                    p { class: "form-error", role: "alert", "{auth_message(text, code)}" }
                }
                button { r#type: "submit", disabled: pending(), "{heading}" }
            }
            button {
                class: "mode-toggle",
                onclick: move |_| {
                    is_registering.set(!is_registering());
                    error.set(None);
                    password.set(String::new());
                },
                if is_registering() { "{text.login}" } else { "{text.register}" }
            }
        }
    }
}

fn auth_message<'a>(text: &'a crate::i18n::Messages, code: &str) -> &'a str {
    match code {
        "invalid_input" => text.invalid_input,
        "username_taken" => text.username_taken,
        "invalid_credentials" => text.invalid_credentials,
        "login_required" => text.login_required,
        _ => text.request_failed,
    }
}

#[cfg(test)]
mod tests {
    use super::auth_message;
    use crate::i18n::{Locale, messages};

    #[test]
    fn authentication_feedback_is_localized() {
        assert_eq!(
            auth_message(messages(Locale::ZhCn), "username_taken"),
            "用户名已被使用。"
        );
        assert_eq!(
            auth_message(messages(Locale::EnUs), "invalid_credentials"),
            "Incorrect username or password."
        );
        assert_eq!(
            auth_message(messages(Locale::ZhCn), "unknown"),
            messages(Locale::ZhCn).request_failed
        );
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
