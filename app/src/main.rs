mod app;
mod i18n;
#[cfg(feature = "server")]
mod storage;

fn main() {
    #[cfg(feature = "server")]
    if let Err(error) = storage::initialize_from_env() {
        eprintln!("Database initialization failed: {error}");
        std::process::exit(1);
    }

    dioxus::launch(app::App);
}
