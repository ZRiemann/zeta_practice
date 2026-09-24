mod app;
mod auth;
mod i18n;
#[cfg(feature = "server")]
mod storage;

fn main() {
    #[cfg(feature = "server")]
    if let Err(error) = storage::initialize_from_env() {
        eprintln!("Server initialization failed: {error}");
        std::process::exit(1);
    }
    #[cfg(feature = "server")]
    if let Err(error) = auth::validate_origin_config() {
        eprintln!("Server initialization failed: {error}");
        std::process::exit(1);
    }

    dioxus::launch(app::App);
}
