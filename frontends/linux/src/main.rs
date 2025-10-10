mod ui;

use gtk4::prelude::*;
use gtk4::Application;

const APP_ID: &str = "com.asrpro.gtk4";

fn main() {
    let application = Application::builder().application_id(APP_ID).build();

    application.connect_activate(|app| {
        ui::build_ui(app);
    });

    application.run();
}
