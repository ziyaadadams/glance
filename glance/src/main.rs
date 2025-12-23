mod app;
mod camera;
mod face;
mod models;
mod storage;
mod window;

use app::GlanceApplication;
use gtk4::prelude::*;
use libadwaita as adw;

fn main() -> gtk4::glib::ExitCode {
    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();
    
    adw::init().expect("Failed to initialize Libadwaita");
    let app = GlanceApplication::new();
    app.run()
}
