use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box as GtkBox, Button, Label, Orientation, pango};

const APP_ID: &str = "com.asrpro.gtk4";

fn build_ui(app: &Application) -> Result<()> {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("ASR Pro")
        .default_width(480)
        .default_height(320)
        .resizable(false)
        .build();

    let container = GtkBox::new(Orientation::Vertical, 12);
    container.set_margin_top(24);
    container.set_margin_bottom(24);
    container.set_margin_start(24);
    container.set_margin_end(24);

    let heading = Label::new(Some("ASR Pro Linux Frontend"));
    heading.set_margin_bottom(12);
    heading.set_wrap(true);
    heading.set_wrap_mode(pango::WrapMode::WordChar);
    heading.set_xalign(0.0);
    heading.add_css_class("title-2");

    let status = Label::new(Some("Ready."));
    status.set_xalign(0.0);
    status.set_wrap(true);

    let button = Button::with_label("Run Diagnostic");
    let status_clone = status.clone();
    button.connect_clicked(move |_| {
        status_clone.set_text("Backend integration not yet implemented, but the UI is alive.");
    });

    container.append(&heading);
    container.append(&button);
    container.append(&status);

    window.set_child(Some(&container));
    window.present();

    Ok(())
}

fn main() -> Result<()> {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(|application| {
        if let Err(err) = build_ui(application) {
            eprintln!("Failed to build UI: {err:?}");
        }
    });

    app.run();
    Ok(())
}
