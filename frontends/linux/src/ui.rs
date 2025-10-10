use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use glib::{self, ControlFlow};
use gtk4::pango;
use gtk4::prelude::*;
use gtk4::Settings;
use gtk4::{
    gdk, Align, Application, ApplicationWindow, Box as GtkBox, Button, ComboBoxText, CssProvider,
    FileDialog, Image, Label, ListBox, ListBoxRow, MessageDialog, Orientation, ProgressBar,
    ResponseType, ScrolledWindow, SelectionMode, Stack, StackTransitionType, TextBuffer, TextView,
    STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use uuid::Uuid;

use crate::backend::{
    BackendClient, HealthResponse, ModelInfo, StartTranscriptionResponse,
    TranscriptionResultResponse, TranscriptionStatusResponse,
};
use crate::state::{FileRecord, FileStatus};

const GENERAL_PAGE_ID: &str = "general";
const MODELS_PAGE_ID: &str = "models";
const TRANSCRIBE_PAGE_ID: &str = "transcribe";
const HISTORY_PAGE_ID: &str = "history";
const KEYBOARD_PAGE_ID: &str = "keyboard";
const ABOUT_PAGE_ID: &str = "about";

struct NavItem {
    id: &'static str,
    label: &'static str,
    icon: Option<&'static str>,
}

const NAV_ITEMS: &[NavItem] = &[
    NavItem {
        id: GENERAL_PAGE_ID,
        label: "General Settings",
        icon: Some("preferences-system-symbolic"),
    },
    NavItem {
        id: MODELS_PAGE_ID,
        label: "Dictation Models",
        icon: Some("system-search-symbolic"),
    },
    NavItem {
        id: TRANSCRIBE_PAGE_ID,
        label: "Transcribe File",
        icon: Some("media-record-symbolic"),
    },
    NavItem {
        id: HISTORY_PAGE_ID,
        label: "History",
        icon: Some("document-open-recent-symbolic"),
    },
    NavItem {
        id: KEYBOARD_PAGE_ID,
        label: "Keyboard Controls",
        icon: Some("input-keyboard-symbolic"),
    },
    NavItem {
        id: ABOUT_PAGE_ID,
        label: "About",
        icon: Some("dialog-information-symbolic"),
    },
];

const APP_CSS: &str = r#"
window.app-window {
    background: #101014;
    color: #f5f7fa;
}

.root {
    background: transparent;
}

.sidebar {
    background: linear-gradient(180deg, rgba(34,34,40,0.95), rgba(20,20,24,0.95));
    padding: 32px 24px;
    border-right: 1px solid rgba(255,255,255,0.06);
}

.sidebar-title {
    font-size: 20px;
    font-weight: 600;
    color: #f3f4f7;
}

.sidebar-subtitle {
    font-size: 12px;
    color: rgba(245,247,250,0.6);
}

.sidebar-nav {
    background: transparent;
}

.sidebar-nav row {
    border-radius: 12px;
    margin-bottom: 8px;
    transition: background 120ms ease;
}

.sidebar-nav row box {
    padding: 4px 12px;
    border-radius: 12px;
    background: transparent;
    color: rgba(245,247,250,0.72);
}

.sidebar-nav row:selected,
.sidebar-nav row:focus {
    background: rgba(80,147,255,0.18);
}

.sidebar-nav row:selected box,
.sidebar-nav row:focus box {
    background: rgba(80,147,255,0.18);
}

.sidebar-nav row:selected label,
.sidebar-nav row:focus label {
    color: #5ea2ff;
}

.sidebar-nav-icon {
    margin-right: 12px;
    opacity: 0.75;
}

.sidebar-nav row:selected .sidebar-nav-icon,
.sidebar-nav row:focus .sidebar-nav-icon {
    opacity: 1;
}

.sidebar-nav-label {
    font-weight: 500;
}

.sidebar-status {
    font-size: 12px;
    color: rgba(245,247,250,0.6);
    margin-top: 16px;
}

.sidebar-footer {
    padding-top: 18px;
}

.sidebar-version {
    font-size: 11px;
    color: rgba(245,247,250,0.3);
}

.content {
    padding: 36px 40px;
    background: radial-gradient(circle at top left, rgba(94,162,255,0.08), transparent 60%),
                radial-gradient(circle at bottom right, rgba(255,135,105,0.06), transparent 55%);
}

.page-title {
    font-size: 28px;
    font-weight: 600;
}

.page-subtitle {
    font-size: 14px;
    color: rgba(245,247,250,0.65);
}

.card {
    background: rgba(34,36,44,0.85);
    border-radius: 22px;
    padding: 24px;
    border: 1px solid rgba(255,255,255,0.04);
}

.card-title {
    font-size: 16px;
    font-weight: 600;
}

.card-subtitle {
    font-size: 12px;
    color: rgba(245,247,250,0.5);
}

.primary-button {
    background: linear-gradient(90deg, #5ea2ff, #4577ff);
    color: #fff;
    border-radius: 14px;
    padding: 12px 18px;
    font-weight: 600;
}

.primary-button:hover {
    background: linear-gradient(90deg, #6ab2ff, #4f82ff);
}

.secondary-button {
    background: rgba(94,162,255,0.12);
    color: #5ea2ff;
    border-radius: 12px;
    padding: 10px 16px;
    font-weight: 500;
}

.secondary-button:hover {
    background: rgba(94,162,255,0.18);
}

.file-list {
    background: transparent;
}

.file-row {
    border-radius: 16px;
    margin-bottom: 8px;
}

.file-row-content {
    background: rgba(18,19,25,0.55);
    border-radius: 16px;
    padding: 12px;
    border: 1px solid rgba(255,255,255,0.04);
}

.heading {
    font-weight: 600;
}

.subtitle {
    font-size: 12px;
    color: rgba(245,247,250,0.55);
}

.transcript-view {
    background: rgba(12,13,18,0.85);
    border-radius: 16px;
    padding: 18px;
    border: 1px solid rgba(255,255,255,0.05);
}

.status-label {
    font-size: 13px;
    color: rgba(245,247,250,0.72);
}

.placeholder-page {
    padding: 48px 60px;
}

.placeholder-title {
    font-size: 26px;
    font-weight: 600;
    color: #f5f7fa;
}

.placeholder-subtitle {
    font-size: 14px;
    color: rgba(245,247,250,0.62);
    max-width: 420px;
}
"#;

pub enum UiEvent {
    ModelsLoaded(Vec<ModelInfo>),
    BackendHealth(HealthResponse),
    FileStatus {
        file_id: Uuid,
        status: FileStatus,
        progress: Option<f64>,
    },
    FileCompleted {
        file_id: Uuid,
        text: String,
    },
    FileFailed {
        file_id: Uuid,
        message: String,
    },
    ShowMessage(String),
}

pub struct AppUi {
    window: ApplicationWindow,
    nav_list: ListBox,
    content_stack: Stack,
    file_list: ListBox,
    model_combo: ComboBoxText,
    add_button: Button,
    transcribe_button: Button,
    status_label: Label,
    sidebar_status_label: Label,
    transcript_buffer: TextBuffer,
    file_rows: Rc<RefCell<HashMap<Uuid, FileRowWidgets>>>,
    files: Rc<RefCell<HashMap<Uuid, FileRecord>>>,
    selected_file: Rc<RefCell<Option<Uuid>>>,
    backend: BackendClient,
    sender: mpsc::Sender<UiEvent>,
}

struct FileRowWidgets {
    row: ListBoxRow,
    status_label: Label,
    progress: ProgressBar,
}

struct TranscribePageWidgets {
    container: GtkBox,
    model_combo: ComboBoxText,
    add_button: Button,
    file_list: ListBox,
    transcribe_button: Button,
    transcript_buffer: TextBuffer,
    status_label: Label,
}

enum PollOutcome {
    Continue,
    Completed,
    Failed,
}

impl AppUi {
    pub fn new(
        app: &Application,
        backend: BackendClient,
        sender: mpsc::Sender<UiEvent>,
    ) -> Result<Rc<Self>> {
        Self::apply_theme()?;

        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(1220)
            .default_height(780)
            .title("ASR Pro")
            .build();
        window.add_css_class("app-window");

        let root = GtkBox::new(Orientation::Horizontal, 0);
        root.add_css_class("root");
        root.set_hexpand(true);
        root.set_vexpand(true);
        window.set_child(Some(&root));

        let sidebar = GtkBox::new(Orientation::Vertical, 24);
        sidebar.add_css_class("sidebar");
        sidebar.set_vexpand(true);
        sidebar.set_width_request(260);

        let brand_box = GtkBox::new(Orientation::Vertical, 6);
        let brand_label = Label::new(Some("ASRPro"));
        brand_label.add_css_class("sidebar-title");
        brand_label.set_halign(Align::Start);
        let subtitle_label = Label::new(Some("Speech Studio"));
        subtitle_label.add_css_class("sidebar-subtitle");
        subtitle_label.set_halign(Align::Start);
        brand_box.append(&brand_label);
        brand_box.append(&subtitle_label);

        let nav_list = ListBox::new();
        nav_list.set_selection_mode(SelectionMode::Single);
        nav_list.set_activate_on_single_click(true);
        nav_list.set_hexpand(true);
        nav_list.set_vexpand(true);
        nav_list.add_css_class("sidebar-nav");

        for item in NAV_ITEMS {
            let row = Self::create_nav_row(item.id, item.label, item.icon);
            nav_list.append(&row);
        }
        if let Some(index) = NAV_ITEMS
            .iter()
            .position(|item| item.id == TRANSCRIBE_PAGE_ID)
        {
            if let Some(row) = nav_list.row_at_index(index as i32) {
                nav_list.select_row(Some(&row));
            }
        }

        let sidebar_status_label = Label::new(Some("Backend: checking…"));
        sidebar_status_label.add_css_class("sidebar-status");
        sidebar_status_label.set_halign(Align::Start);

        let sidebar_footer = GtkBox::new(Orientation::Vertical, 4);
        sidebar_footer.set_valign(Align::End);
        sidebar_footer.set_halign(Align::Start);
        sidebar_footer.add_css_class("sidebar-footer");
        let version_label = Label::new(Some(&format!("v{} (dev)", env!("CARGO_PKG_VERSION"))));
        version_label.add_css_class("sidebar-version");
        version_label.set_halign(Align::Start);
        sidebar_footer.append(&sidebar_status_label);
        sidebar_footer.append(&version_label);

        sidebar.append(&brand_box);
        sidebar.append(&nav_list);
        sidebar.append(&sidebar_footer);

        let content_stack = Stack::new();
        content_stack.add_css_class("content");
        content_stack.set_transition_type(StackTransitionType::Crossfade);
        content_stack.set_hexpand(true);
        content_stack.set_vexpand(true);

        let transcribe_page = Self::build_transcribe_page();
        content_stack.add_named(&transcribe_page.container, Some(TRANSCRIBE_PAGE_ID));
        content_stack.set_visible_child_name(TRANSCRIBE_PAGE_ID);

        let general_page = Self::build_placeholder_page(
            "General Settings",
            "Adjust application-wide preferences, theme choices, and connectivity defaults.",
        );
        content_stack.add_named(&general_page, Some(GENERAL_PAGE_ID));

        let models_page = Self::build_placeholder_page(
            "Dictation Models",
            "Browse cloud and on-device models, compare accuracy, and set your preferred engine.",
        );
        content_stack.add_named(&models_page, Some(MODELS_PAGE_ID));

        let history_page = Self::build_placeholder_page(
            "History",
            "Review previous transcription runs, reopen transcripts, and manage exports.",
        );
        content_stack.add_named(&history_page, Some(HISTORY_PAGE_ID));

        let keyboard_page = Self::build_placeholder_page(
            "Keyboard Controls",
            "Discover shortcuts for managing recordings, launching transcriptions, and navigating.",
        );
        content_stack.add_named(&keyboard_page, Some(KEYBOARD_PAGE_ID));

        let about_page = Self::build_placeholder_page(
            "About ASRPro",
            "ASRPro provides professional-grade speech-to-text with seamless cloud integration.",
        );
        content_stack.add_named(&about_page, Some(ABOUT_PAGE_ID));

        root.append(&sidebar);
        root.append(&content_stack);

        let ui = Rc::new(Self {
            window,
            nav_list,
            content_stack,
            file_list: transcribe_page.file_list,
            model_combo: transcribe_page.model_combo,
            add_button: transcribe_page.add_button,
            transcribe_button: transcribe_page.transcribe_button,
            status_label: transcribe_page.status_label,
            sidebar_status_label,
            transcript_buffer: transcribe_page.transcript_buffer,
            file_rows: Rc::new(RefCell::new(HashMap::new())),
            files: Rc::new(RefCell::new(HashMap::new())),
            selected_file: Rc::new(RefCell::new(None)),
            backend,
            sender,
        });

        ui.setup_handlers();
        ui.window.present();

        Ok(ui)
    }

    fn build_transcribe_page() -> TranscribePageWidgets {
        let container = GtkBox::new(Orientation::Vertical, 24);
        container.add_css_class("page");
        container.set_margin_top(12);
        container.set_margin_bottom(12);
        container.set_margin_start(12);
        container.set_margin_end(12);

        let header_box = GtkBox::new(Orientation::Vertical, 6);
        header_box.set_halign(Align::Start);
        let title = Label::new(Some("Transcribe Audio"));
        title.add_css_class("page-title");
        title.set_halign(Align::Start);
        let subtitle = Label::new(Some(
            "Upload audio files, pick a model, and stream transcripts from the ASR Pro engine.",
        ));
        subtitle.add_css_class("page-subtitle");
        subtitle.set_halign(Align::Start);
        header_box.append(&title);
        header_box.append(&subtitle);

        let body = GtkBox::new(Orientation::Horizontal, 24);
        body.set_hexpand(true);
        body.set_vexpand(true);

        let queue_card = GtkBox::new(Orientation::Vertical, 18);
        queue_card.add_css_class("card");
        queue_card.set_hexpand(true);
        queue_card.set_vexpand(true);

        let queue_header = GtkBox::new(Orientation::Vertical, 4);
        queue_header.set_halign(Align::Start);
        let queue_title = Label::new(Some("Transcription Queue"));
        queue_title.add_css_class("card-title");
        queue_title.set_halign(Align::Start);
        let queue_subtitle = Label::new(Some(
            "Stack up audio files, then launch transcription when you are ready.",
        ));
        queue_subtitle.add_css_class("card-subtitle");
        queue_subtitle.set_halign(Align::Start);
        queue_header.append(&queue_title);
        queue_header.append(&queue_subtitle);

        let model_row = GtkBox::new(Orientation::Horizontal, 10);
        let model_label = Label::new(Some("Model"));
        model_label.set_halign(Align::Start);
        model_label.add_css_class("subtitle");
        let model_combo = ComboBoxText::new();
        model_combo.set_hexpand(true);
        model_row.append(&model_label);
        model_row.append(&model_combo);

        let add_button = Button::with_label("Add Audio File");
        add_button.add_css_class("secondary-button");
        add_button.set_icon_name("list-add-symbolic");
        add_button.set_halign(Align::Start);

        let file_list = ListBox::new();
        file_list.set_selection_mode(SelectionMode::Single);
        file_list.add_css_class("file-list");
        file_list.set_vexpand(true);
        file_list.set_hexpand(true);

        let file_scroller = ScrolledWindow::builder()
            .child(&file_list)
            .vexpand(true)
            .hexpand(true)
            .min_content_height(260)
            .build();

        let transcribe_button = Button::with_label("Transcribe Selected");
        transcribe_button.add_css_class("primary-button");
        transcribe_button.set_sensitive(false);
        transcribe_button.set_icon_name("media-playback-start-symbolic");
        transcribe_button.set_halign(Align::End);

        queue_card.append(&queue_header);
        queue_card.append(&model_row);
        queue_card.append(&add_button);
        queue_card.append(&file_scroller);
        queue_card.append(&transcribe_button);

        let transcript_card = GtkBox::new(Orientation::Vertical, 16);
        transcript_card.add_css_class("card");
        transcript_card.set_hexpand(true);
        transcript_card.set_vexpand(true);

        let transcript_title = Label::new(Some("Live Transcript"));
        transcript_title.add_css_class("card-title");
        transcript_title.set_halign(Align::Start);
        let transcript_hint = Label::new(Some(
            "Results appear here as soon as the backend finishes processing.",
        ));
        transcript_hint.add_css_class("card-subtitle");
        transcript_hint.set_halign(Align::Start);

        let transcript_buffer = TextBuffer::new(None);
        let transcript_view = TextView::builder()
            .buffer(&transcript_buffer)
            .editable(false)
            .monospace(true)
            .wrap_mode(gtk4::WrapMode::WordChar)
            .vexpand(true)
            .hexpand(true)
            .build();
        transcript_view.add_css_class("transcript-view");

        let transcript_scroller = ScrolledWindow::builder()
            .child(&transcript_view)
            .vexpand(true)
            .hexpand(true)
            .build();

        transcript_card.append(&transcript_title);
        transcript_card.append(&transcript_hint);
        transcript_card.append(&transcript_scroller);

        body.append(&queue_card);
        body.append(&transcript_card);

        let status_label = Label::new(Some("Ready."));
        status_label.add_css_class("status-label");
        status_label.set_halign(Align::Start);

        container.append(&header_box);
        container.append(&body);
        container.append(&status_label);

        TranscribePageWidgets {
            container,
            model_combo,
            add_button,
            file_list,
            transcribe_button,
            transcript_buffer,
            status_label,
        }
    }

    fn build_placeholder_page(title: &str, description: &str) -> GtkBox {
        let container = GtkBox::new(Orientation::Vertical, 18);
        container.add_css_class("placeholder-page");
        container.set_valign(Align::Start);
        container.set_halign(Align::Start);

        let title_label = Label::new(Some(title));
        title_label.add_css_class("placeholder-title");
        title_label.set_halign(Align::Start);

        let description_label = Label::new(Some(description));
        description_label.add_css_class("placeholder-subtitle");
        description_label.set_wrap(true);
        description_label.set_wrap_mode(pango::WrapMode::WordChar);
        description_label.set_halign(Align::Start);

        container.append(&title_label);
        container.append(&description_label);
        container
    }

    fn apply_theme() -> Result<()> {
        if let Some(settings) = Settings::default() {
            settings.set_gtk_application_prefer_dark_theme(true);
        }

        let provider = CssProvider::new();
        provider.load_from_data(APP_CSS);

        if let Some(display) = gdk::Display::default() {
            gtk4::StyleContext::add_provider_for_display(
                &display,
                &provider,
                STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        Ok(())
    }

    fn create_nav_row(id: &str, label: &str, icon_name: Option<&str>) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_widget_name(id);

        let container = GtkBox::new(Orientation::Horizontal, 12);
        container.set_margin_start(18);
        container.set_margin_end(18);
        container.set_margin_top(12);
        container.set_margin_bottom(12);
        container.set_valign(Align::Center);

        if let Some(icon_name) = icon_name {
            let icon = Image::from_icon_name(icon_name);
            icon.add_css_class("sidebar-nav-icon");
            icon.set_pixel_size(18);
            container.append(&icon);
        }

        let label_widget = Label::new(Some(label));
        label_widget.add_css_class("sidebar-nav-label");
        label_widget.set_halign(Align::Start);
        container.append(&label_widget);

        row.set_child(Some(&container));
        row
    }

    fn on_nav_selected(&self, row: Option<&ListBoxRow>) {
        let Some(row) = row else {
            if let Some(first) = self.nav_list.row_at_index(0) {
                self.nav_list.select_row(Some(&first));
            }
            return;
        };

        let page_id = row.widget_name();
        match page_id.as_str() {
            GENERAL_PAGE_ID => {
                self.content_stack.set_visible_child_name(GENERAL_PAGE_ID);
                self.sidebar_status_label
                    .set_text("General settings coming soon.");
            }
            MODELS_PAGE_ID => {
                self.content_stack.set_visible_child_name(MODELS_PAGE_ID);
                self.sidebar_status_label
                    .set_text("Browse dictation models (coming soon).");
            }
            TRANSCRIBE_PAGE_ID => {
                self.content_stack
                    .set_visible_child_name(TRANSCRIBE_PAGE_ID);
            }
            HISTORY_PAGE_ID => {
                self.content_stack.set_visible_child_name(HISTORY_PAGE_ID);
                self.sidebar_status_label
                    .set_text("View transcripts history (coming soon).");
            }
            KEYBOARD_PAGE_ID => {
                self.content_stack.set_visible_child_name(KEYBOARD_PAGE_ID);
                self.sidebar_status_label
                    .set_text("Keyboard shortcuts (coming soon).");
            }
            ABOUT_PAGE_ID => {
                self.content_stack.set_visible_child_name(ABOUT_PAGE_ID);
                self.sidebar_status_label.set_text("About ASRPro.");
            }
            _ => {
                self.content_stack
                    .set_visible_child_name(TRANSCRIBE_PAGE_ID);
            }
        }
    }

    pub fn init(self: Rc<Self>, receiver: mpsc::Receiver<UiEvent>) {
        let ui = Rc::clone(&self);
        let receiver = Rc::new(RefCell::new(receiver));

        glib::idle_add_local(move || {
            loop {
                let event = {
                    let rx = &mut *receiver.borrow_mut();
                    match rx.try_recv() {
                        Ok(event) => event,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return ControlFlow::Break,
                    }
                };
                ui.handle_event(event);
            }
            ControlFlow::Continue
        });

        self.load_initial_data();
    }

    fn setup_handlers(self: &Rc<Self>) {
        let ui = Rc::clone(self);
        self.nav_list.connect_row_selected(move |_, row| {
            ui.on_nav_selected(row);
        });

        let ui = Rc::clone(self);
        self.nav_list.connect_row_activated(move |_, row| {
            ui.on_nav_selected(Some(row));
        });

        let ui = Rc::clone(self);
        self.add_button.connect_clicked(move |_| {
            ui.open_file_dialog();
        });

        let ui = Rc::clone(self);
        self.file_list.connect_row_selected(move |_, row| {
            ui.on_row_selected(row);
        });

        let ui = Rc::clone(self);
        self.transcribe_button.connect_clicked(move |_| {
            if let Err(err) = ui.start_transcription_for_selected() {
                ui.show_error(&format!("{err:?}"));
            }
        });
    }

    fn load_initial_data(&self) {
        self.status_label
            .set_text("Loading model catalog and backend status…");

        let backend = self.backend.clone();
        let sender = self.sender.clone();
        thread::spawn(move || match backend.list_models() {
            Ok(models) => {
                let _ = sender.send(UiEvent::ModelsLoaded(models));
            }
            Err(err) => {
                let _ = sender.send(UiEvent::ShowMessage(format!("Model list failed: {err}")));
            }
        });

        let backend = self.backend.clone();
        let sender = self.sender.clone();
        thread::spawn(move || match backend.health() {
            Ok(health) => {
                let _ = sender.send(UiEvent::BackendHealth(health));
            }
            Err(err) => {
                let _ = sender.send(UiEvent::ShowMessage(format!("Health check failed: {err}")));
            }
        });
    }

    fn open_file_dialog(self: &Rc<Self>) {
        let dialog = FileDialog::builder()
            .title("Select Audio File")
            .accept_label("Add")
            .modal(true)
            .build();
        let window = self.window.clone();
        let ui = Rc::clone(self);

        glib::MainContext::default().spawn_local(async move {
            match dialog.open_future(Some(&window)).await {
                Ok(file) => {
                    if let Some(path) = file.path() {
                        ui.add_file(path);
                    } else {
                        let uri = file.uri();
                        ui.show_error(&format!("Unsupported URI: {uri}"));
                    }
                }
                Err(err) => {
                    if !err.matches::<gtk4::gio::IOErrorEnum>(gtk4::gio::IOErrorEnum::Cancelled) {
                        ui.show_error(&format!("File selection failed: {err}"));
                    }
                }
            }
        });
    }

    fn add_file(&self, path: PathBuf) {
        let record = FileRecord::new(path);
        let file_id = record.id;

        let row = ListBoxRow::new();
        row.set_activatable(true);
        row.add_css_class("file-row");

        let container = GtkBox::new(Orientation::Vertical, 6);
        container.add_css_class("file-row-content");

        let name_label = Label::new(Some(&record.display_name));
        name_label.set_halign(Align::Start);
        name_label.add_css_class("heading");

        let status_label = Label::new(Some("Pending"));
        status_label.set_halign(Align::Start);
        status_label.add_css_class("subtitle");

        let progress_bar = ProgressBar::new();
        progress_bar.set_visible(false);
        progress_bar.set_hexpand(true);
        progress_bar.set_margin_top(6);

        container.append(&name_label);
        container.append(&status_label);
        container.append(&progress_bar);

        row.set_child(Some(&container));
        self.file_list.append(&row);
        self.file_list.select_row(Some(&row));

        self.file_rows.borrow_mut().insert(
            file_id,
            FileRowWidgets {
                row,
                status_label,
                progress: progress_bar,
            },
        );

        self.files.borrow_mut().insert(file_id, record);
        self.status_label
            .set_text("File added. Select it and choose a model to transcribe.");
    }

    fn on_row_selected(&self, row: Option<&ListBoxRow>) {
        if let Some(row) = row {
            let file_id = self.file_rows.borrow().iter().find_map(|(id, widgets)| {
                if widgets.row == *row {
                    Some(*id)
                } else {
                    None
                }
            });

            if let Some(id) = file_id {
                *self.selected_file.borrow_mut() = Some(id);
                let model_selected = self.model_combo.active_id().is_some();
                self.transcribe_button.set_sensitive(model_selected);

                if let Some(record) = self.files.borrow().get(&id) {
                    if let Some(text) = &record.result_text {
                        self.transcript_buffer.set_text(text);
                    } else {
                        self.transcript_buffer.set_text("");
                    }
                }
            }
        } else {
            *self.selected_file.borrow_mut() = None;
            self.transcribe_button.set_sensitive(false);
        }
    }

    fn start_transcription_for_selected(self: &Rc<Self>) -> Result<()> {
        let file_id = self
            .selected_file
            .borrow()
            .clone()
            .ok_or_else(|| anyhow!("Select a file to transcribe"))?;

        let model = self
            .model_combo
            .active_id()
            .map(|id| id.to_string())
            .ok_or_else(|| anyhow!("Select a model before transcribing"))?;

        let path = self
            .files
            .borrow()
            .get(&file_id)
            .map(|record| record.path.clone())
            .ok_or_else(|| anyhow!("File not found"))?;

        self.update_file_status(file_id, FileStatus::Uploading, Some(0.0));
        self.status_label.set_text("Uploading audio file...");
        self.transcribe_button.set_sensitive(false);

        let backend = self.backend.clone();
        let sender = self.sender.clone();

        thread::spawn(move || {
            if let Err(err) =
                AppUi::run_transcription(backend, sender.clone(), file_id, path, model)
            {
                let _ = sender.send(UiEvent::FileFailed {
                    file_id,
                    message: err.to_string(),
                });
            }
        });

        Ok(())
    }

    fn run_transcription(
        backend: BackendClient,
        sender: mpsc::Sender<UiEvent>,
        file_id: Uuid,
        path: PathBuf,
        model: String,
    ) -> Result<()> {
        let start: StartTranscriptionResponse = backend.start_transcription(&path, &model)?;

        sender.send(UiEvent::FileStatus {
            file_id,
            status: FileStatus::Processing,
            progress: Some(0.05),
        })?;

        if start.status.eq_ignore_ascii_case("failed") {
            sender.send(UiEvent::FileFailed {
                file_id,
                message: "Transcription request rejected by backend".into(),
            })?;
            return Ok(());
        }

        let task_id = start.task_id;
        let outcome = loop {
            thread::sleep(Duration::from_secs(2));
            match backend.transcription_status(task_id) {
                Ok(status) => match AppUi::handle_status(&sender, file_id, &status) {
                    PollOutcome::Continue => {}
                    finished => break finished,
                },
                Err(err) => {
                    sender.send(UiEvent::FileFailed {
                        file_id,
                        message: format!("Status polling failed: {err}"),
                    })?;
                    return Ok(());
                }
            }
        };

        if matches!(outcome, PollOutcome::Completed) {
            match backend.transcription_result(task_id) {
                Ok(result) => {
                    if let Some(text) = AppUi::extract_text(&result) {
                        sender.send(UiEvent::FileCompleted { file_id, text })?;
                    } else {
                        sender.send(UiEvent::FileFailed {
                            file_id,
                            message: "Result did not include transcription text.".into(),
                        })?;
                    }
                }
                Err(err) => {
                    sender.send(UiEvent::FileFailed {
                        file_id,
                        message: format!("Failed to fetch result: {err}"),
                    })?;
                }
            }
        }

        Ok(())
    }

    fn handle_status(
        sender: &mpsc::Sender<UiEvent>,
        file_id: Uuid,
        status: &TranscriptionStatusResponse,
    ) -> PollOutcome {
        let progress = status.progress.map(|p| p.clamp(0.0, 1.0));

        match status.status.to_lowercase().as_str() {
            "completed" => {
                sender
                    .send(UiEvent::FileStatus {
                        file_id,
                        status: FileStatus::Completed,
                        progress: Some(1.0),
                    })
                    .ok();
                PollOutcome::Completed
            }
            "failed" => {
                let message = status
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "Transcription failed".into());
                sender.send(UiEvent::FileFailed { file_id, message }).ok();
                PollOutcome::Failed
            }
            _ => {
                sender
                    .send(UiEvent::FileStatus {
                        file_id,
                        status: FileStatus::Processing,
                        progress,
                    })
                    .ok();
                PollOutcome::Continue
            }
        }
    }

    fn extract_text(result: &TranscriptionResultResponse) -> Option<String> {
        result
            .result
            .as_ref()
            .and_then(|payload| payload.text.clone())
    }

    fn update_file_status(&self, file_id: Uuid, status: FileStatus, progress: Option<f64>) {
        if let Some(record) = self.files.borrow_mut().get_mut(&file_id) {
            record.status = status.clone();
            if status.is_terminal() {
                record.progress = if matches!(status, FileStatus::Failed(_)) {
                    0.0
                } else {
                    progress.unwrap_or(1.0)
                };
            } else if let Some(p) = progress {
                record.progress = p;
            }
        }

        if let Some(widgets) = self.file_rows.borrow().get(&file_id) {
            widgets.status_label.set_text(&status.label());
            if let Some(progress) = progress {
                widgets.progress.set_visible(true);
                widgets.progress.set_fraction(progress);
            } else if status.is_terminal() {
                widgets.progress.set_visible(false);
            }
        }
    }

    fn handle_event(&self, event: UiEvent) {
        match event {
            UiEvent::ModelsLoaded(models) => self.populate_models(models),
            UiEvent::BackendHealth(health) => {
                let message = format!(
                    "Backend: {}{}",
                    health.status,
                    health
                        .version
                        .map(|v| format!(" (v{v})"))
                        .unwrap_or_default()
                );
                self.sidebar_status_label.set_text(&message);
                self.status_label.set_text(&message);
            }
            UiEvent::FileStatus {
                file_id,
                status,
                progress,
            } => {
                self.update_file_status(file_id, status, progress);
            }
            UiEvent::FileCompleted { file_id, text } => {
                if let Some(record) = self.files.borrow_mut().get_mut(&file_id) {
                    record.result_text = Some(text.clone());
                }

                if Some(file_id) == *self.selected_file.borrow() {
                    self.transcript_buffer.set_text(&text);
                }

                self.status_label
                    .set_text("Transcription complete. Result displayed.");
                self.sidebar_status_label
                    .set_text("Last job completed successfully.");
                self.transcribe_button.set_sensitive(true);
            }
            UiEvent::FileFailed { file_id, message } => {
                self.update_file_status(file_id, FileStatus::Failed(message.clone()), None);
                self.status_label
                    .set_text(&format!("Transcription failed: {message}"));
                self.sidebar_status_label
                    .set_text(&format!("Last job failed: {message}"));
                self.transcribe_button.set_sensitive(true);
            }
            UiEvent::ShowMessage(message) => {
                self.status_label.set_text(&message);
                self.sidebar_status_label.set_text(&message);
            }
        }
    }

    fn populate_models(&self, models: Vec<ModelInfo>) {
        self.model_combo.remove_all();

        for model in models {
            let label = model
                .display_name
                .clone()
                .unwrap_or_else(|| model.name.clone());
            self.model_combo.append(Some(&model.name), &label);
        }

        if self.model_combo.active_id().is_none() {
            self.model_combo.set_active(Some(0));
        }

        self.status_label
            .set_text("Models loaded. Select an audio file to begin.");

        if self.selected_file.borrow().is_some() {
            self.transcribe_button.set_sensitive(true);
        }
    }

    fn show_error(&self, message: &str) {
        self.status_label.set_text(message);

        let dialog = MessageDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .text("Operation failed")
            .secondary_text(message)
            .build();

        dialog.add_button("Close", ResponseType::Close);
        dialog.connect_response(|dialog, _| dialog.close());
        dialog.present();
    }
}
