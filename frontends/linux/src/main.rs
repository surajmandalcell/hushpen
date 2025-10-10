mod backend;
mod state;
mod ui;

use anyhow::Result;
use gtk4::prelude::*;
use gtk4::Application;
use std::sync::mpsc;

use backend::BackendClient;
use ui::{AppUi, UiEvent};

const APP_ID: &str = "com.asrpro.gtk4";

fn main() -> Result<()> {
    let application = Application::builder().application_id(APP_ID).build();

    application.connect_activate(|app| {
        if let Err(err) = launch_ui(app) {
            eprintln!("Failed to start UI: {err:?}");
        }
    });

    application.run();
    Ok(())
}

fn launch_ui(app: &Application) -> Result<()> {
    let backend = BackendClient::from_env()?;
    let (sender, receiver) = mpsc::channel::<UiEvent>();
    let ui = AppUi::new(app, backend, sender)?;
    ui.init(receiver);
    Ok(())
}
