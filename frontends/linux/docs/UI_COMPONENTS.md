# ASR Pro Linux Frontend UI Components

This document provides a comprehensive overview of the UI components used in the ASR Pro Linux frontend. It covers the structure, functionality, and implementation details of all user interface components.

## Table of Contents

- [UI Architecture Overview](#ui-architecture-overview)
- [Main Window Components](#main-window-components)
  - [MainWindow](#mainwindow)
  - [HeaderBar](#headerbar)
  - [MenuBar](#menubar)
  - [StatusBar](#statusbar)
- [Panel Components](#panel-components)
  - [FilePanel](#filepanel)
  - [ModelPanel](#modelpanel)
  - [TranscriptionPanel](#transcriptionpanel)
- [Dialog Components](#dialog-components)
  - [SettingsDialog](#settingsdialog)
  - [AboutDialog](#aboutdialog)
  - [ProgressDialog](#progressdialog)
- [Widget Components](#widget-components)
  - [FileDropWidget](#filedropwidget)
  - [FileListWidget](#filelistwidget)
  - [ModelSelectorWidget](#modelselectorwidget)
  - [ModelDetailsWidget](#modeldetailswidget)
  - [TranscriptionTextWidget](#transcriptiontextwidget)
  - [TranscriptionControlsWidget](#transcriptioncontrolswidget)
  - [AudioPreviewWidget](#audiopreviewwidget)
  - [WaveformWidget](#waveformwidget)
- [Styling and Theming](#styling-and-theming)
- [Component Communication](#component-communication)
- [Custom Widget Development](#custom-widget-development)

## UI Architecture Overview

The ASR Pro frontend follows a hierarchical component architecture based on GTK4. The UI is organized into a main window containing various panels and widgets, each with specific responsibilities.

### Component Hierarchy

```
ApplicationWindow
├── HeaderBar
│   ├── MenuButton
│   ├── Title
│   └── WindowControls
├── Box (Vertical)
│   ├── Paned (Horizontal)
│   │   ├── Notebook (Left)
│   │   │   ├── FilePanel
│   │   │   └── ModelPanel
│   │   └── Paned (Vertical, Right)
│   │       ├── TranscriptionPanel
│   │       └── AudioPreviewWidget
│   └── StatusBar
```

### MVVM Pattern in UI

Each UI component follows the MVVM pattern:
- **Model**: Data structures from `src/models/`
- **View**: GTK4 widgets and layouts
- **ViewModel**: UI controllers that handle interactions

```rust
pub struct ComponentController {
    app_state: Arc<AppState>,
    // UI-specific state
    selected_items: RefCell<Vec<Uuid>>,
    // Service references
    service: Arc<ServiceType>,
}
```

## Main Window Components

### MainWindow

The main application window that contains all other components.

#### Structure

```rust
pub struct MainWindow {
    window: ApplicationWindow,
    app_state: Arc<AppState>,
    
    // Main components
    header_bar: HeaderBar,
    main_box: Box,
    main_paned: Paned,
    notebook: Notebook,
    status_bar: StatusBar,
    
    // Panels
    file_panel: FilePanel,
    model_panel: ModelPanel,
    transcription_panel: TranscriptionPanel,
    
    // Services
    transcription_service: Arc<TranscriptionService>,
    file_manager: Arc<FileManager>,
}
```

#### Key Methods

```rust
impl MainWindow {
    pub fn new(app: &Application, app_state: Arc<AppState>) -> AppResult<Self>
    pub fn setup_layout(&mut self) -> AppResult<()>
    pub fn connect_events(&self)
    pub fn show(&self)
    pub fn on_transcribe_clicked(&self)
    pub fn on_settings_clicked(&self)
    pub fn on_about_clicked(&self)
}
```

#### Event Handling

The main window handles high-level application events:

```rust
fn connect_events(&self) {
    // Transcription button
    self.transcribe_button.connect_clicked(clone!(@strong self.app_state => move |_| {
        // Handle transcription request
    }));
    
    // Window close event
    self.window.connect_close_request(clone!(@strong self.app_state => move |_| {
        // Handle application shutdown
        glib::Propagation::Proceed
    }));
    
    // Keyboard shortcuts
    let accel_group = AccelGroup::new();
    self.window.add_accel_group(&accel_group);
    
    // Ctrl+T: Start transcription
    self.transcribe_button.add_accelerator("clicked", &accel_group, gdk::Key::t, gdk::ModifierType::CONTROL_MASK, AccelFlags::VISIBLE);
}
```

### HeaderBar

The header bar provides a modern title bar with integrated controls.

#### Structure

```rust
pub struct HeaderBar {
    header_bar: gtk4::HeaderBar,
    title_label: Label,
    subtitle_label: Label,
    menu_button: MenuButton,
    transcribe_button: Button,
}
```

#### Implementation

```rust
impl HeaderBar {
    pub fn new() -> Self {
        let header_bar = gtk4::HeaderBar::builder()
            .title_widget(&create_title_widget())
            .build();
        
        // Add menu button
        let menu_button = MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Application Menu")
            .build();
        
        // Add transcribe button
        let transcribe_button = Button::builder()
            .label("Transcribe")
            .css_classes(vec!["suggested-action".to_string()])
            .build();
        
        header_bar.pack_start(&menu_button);
        header_bar.pack_end(&transcribe_button);
        
        Self {
            header_bar,
            title_label,
            subtitle_label,
            menu_button,
            transcribe_button,
        }
    }
    
    pub fn set_subtitle(&self, text: &str) {
        self.subtitle_label.set_text(text);
    }
    
    pub fn set_sensitive(&self, sensitive: bool) {
        self.transcribe_button.set_sensitive(sensitive);
    }
}
```

### MenuBar

The application menu providing access to various actions.

#### Structure

```rust
pub struct MenuBar {
    menu_model: MenuModel,
    file_section: MenuSection,
    edit_section: MenuSection,
    view_section: MenuSection,
    help_section: MenuSection,
}
```

#### Implementation

```rust
impl MenuBar {
    pub fn new() -> Self {
        let menu_model = Menu::new();
        
        // File menu
        let file_section = Menu::new();
        file_section.append(Some("Open Files"), Some("app.open"));
        file_section.append(Some("Save Project"), Some("app.save"));
        file_section.append(Some("Export"), Some("app.export"));
        
        // Edit menu
        let edit_section = Menu::new();
        edit_section.append(Some("Select All"), Some("app.select_all"));
        edit_section.append(Some("Clear Selection"), Some("app.clear_selection"));
        
        // Add sections to main menu
        menu_model.append_section(Some("_File"), &file_section);
        menu_model.append_section(Some("_Edit"), &edit_section);
        
        Self {
            menu_model,
            file_section,
            edit_section,
            view_section,
            help_section,
        }
    }
}
```

### StatusBar

The status bar displays application status and progress information.

#### Structure

```rust
pub struct StatusBar {
    status_bar: Box,
    status_label: Label,
    progress_bar: ProgressBar,
    connection_status_icon: Image,
    connection_status_label: Label,
}
```

#### Implementation

```rust
impl StatusBar {
    pub fn new() -> Self {
        let status_bar = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .margin_start(6)
            .margin_end(6)
            .margin_top(3)
            .margin_bottom(3)
            .build();
        
        let status_label = Label::builder()
            .halign(Align::Start)
            .ellipsize(EllipsizeMode::End)
            .hexpand(true)
            .build();
        
        let progress_bar = ProgressBar::builder()
            .visible(false)
            .width_request(200)
            .build();
        
        let connection_status_icon = Image::builder()
            .icon_name("network-offline-symbolic")
            .build();
        
        let connection_status_label = Label::builder()
            .label("Offline")
            .build();
        
        status_bar.append(&status_label);
        status_bar.append(&progress_bar);
        status_bar.append(&connection_status_icon);
        status_bar.append(&connection_status_label);
        
        Self {
            status_bar,
            status_label,
            progress_bar,
            connection_status_icon,
            connection_status_label,
        }
    }
    
    pub fn set_status(&self, message: &str) {
        self.status_label.set_text(message);
    }
    
    pub fn set_progress(&self, fraction: f64) {
        if fraction >= 0.0 && fraction <= 1.0 {
            self.progress_bar.set_fraction(fraction);
            self.progress_bar.set_visible(true);
        } else {
            self.progress_bar.set_visible(false);
        }
    }
    
    pub fn set_connection_status(&self, connected: bool, message: &str) {
        if connected {
            self.connection_status_icon.set_from_icon_name(Some("network-transmit-receive-symbolic"));
        } else {
            self.connection_status_icon.set_from_icon_name(Some("network-offline-symbolic"));
        }
        self.connection_status_label.set_text(message);
    }
}
```

## Panel Components

### FilePanel

The file panel manages and displays audio files for transcription.

#### Structure

```rust
pub struct FilePanel {
    widget: Box,
    app_state: Arc<AppState>,
    
    // UI components
    toolbar: Toolbar,
    file_list: FileListWidget,
    file_drop: FileDropWidget,
    
    // State
    selected_files: RefCell<Vec<Uuid>>,
    drag_active: Cell<bool>,
}
```

#### Implementation

```rust
impl FilePanel {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let widget = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        
        // Create toolbar
        let toolbar = Toolbar::new();
        
        // Create file drop area
        let file_drop = FileDropWidget::new(app_state.clone());
        
        // Create file list
        let file_list = FileListWidget::new(app_state.clone());
        
        // Assemble panel
        widget.append(&toolbar.widget);
        widget.append(&file_drop.widget);
        widget.append(&file_list.widget);
        
        Self {
            widget,
            app_state,
            toolbar,
            file_list,
            file_drop,
            selected_files: RefCell::new(Vec::new()),
            drag_active: Cell::new(false),
        }
    }
    
    pub fn add_files(&self, files: Vec<PathBuf>) -> AppResult<()> {
        for file_path in files {
            // Add file to application state
            let file_id = self.app_state.add_file(file_path).await?;
            
            // Select the newly added file
            self.selected_files.borrow_mut().push(file_id);
        }
        
        // Update UI
        self.update_file_list();
        
        Ok(())
    }
    
    pub fn get_selected_files(&self) -> Vec<Uuid> {
        self.selected_files.borrow().clone()
    }
    
    fn update_file_list(&self) {
        // Refresh the file list display
        self.file_list.refresh();
    }
}
```

### ModelPanel

The model panel displays and manages AI transcription models.

#### Structure

```rust
pub struct ModelPanel {
    widget: Box,
    app_state: Arc<AppState>,
    
    // UI components
    model_selector: ModelSelectorWidget,
    model_details: ModelDetailsWidget,
    refresh_button: Button,
    download_button: Button,
    
    // State
    selected_model: RefCell<Option<Uuid>>,
}
```

#### Implementation

```rust
impl ModelPanel {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let widget = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        
        // Create model selector
        let model_selector = ModelSelectorWidget::new(app_state.clone());
        
        // Create model details
        let model_details = ModelDetailsWidget::new(app_state.clone());
        
        // Create action buttons
        let button_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();
        
        let refresh_button = Button::builder()
            .label("Refresh")
            .icon_name("view-refresh-symbolic")
            .build();
        
        let download_button = Button::builder()
            .label("Download")
            .icon_name("folder-download-symbolic")
            .sensitive(false)
            .build();
        
        button_box.append(&refresh_button);
        button_box.append(&download_button);
        
        // Assemble panel
        widget.append(&model_selector.widget);
        widget.append(&model_details.widget);
        widget.append(&button_box);
        
        Self {
            widget,
            app_state,
            model_selector,
            model_details,
            refresh_button,
            download_button,
            selected_model: RefCell::new(None),
        }
    }
    
    pub fn get_selected_model(&self) -> Option<Model> {
        let model_id = self.selected_model.borrow();
        match *model_id {
            Some(id) => self.app_state.get_model(id),
            None => None,
        }
    }
    
    fn on_model_selected(&self, model_id: Uuid) {
        *self.selected_model.borrow_mut() = Some(model_id);
        
        // Update model details
        self.model_details.show_model(model_id);
        
        // Update button states
        self.update_button_states();
    }
    
    fn update_button_states(&self) {
        let model_id = self.selected_model.borrow();
        match *model_id {
            Some(id) => {
                if let Some(model) = self.app_state.get_model(id) {
                    self.download_button.set_sensitive(!model.is_downloaded());
                }
            },
            None => {
                self.download_button.set_sensitive(false);
            }
        }
    }
}
```

### TranscriptionPanel

The transcription panel displays transcription results and controls.

#### Structure

```rust
pub struct TranscriptionPanel {
    widget: Box,
    app_state: Arc<AppState>,
    
    // UI components
    transcription_text: TranscriptionTextWidget,
    transcription_controls: TranscriptionControlsWidget,
    task_list: ListBox,
    
    // State
    selected_task: RefCell<Option<Uuid>>,
}
```

#### Implementation

```rust
impl TranscriptionPanel {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let widget = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        
        // Create transcription controls
        let transcription_controls = TranscriptionControlsWidget::new(app_state.clone());
        
        // Create task list
        let task_list = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();
        
        // Create text view
        let transcription_text = TranscriptionTextWidget::new(app_state.clone());
        
        // Create scrolled window for text
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .min_content_width(400)
            .min_content_height(300)
            .child(&transcription_text.widget)
            .build();
        
        // Assemble panel
        widget.append(&transcription_controls.widget);
        widget.append(&task_list);
        widget.append(&scrolled);
        
        Self {
            widget,
            app_state,
            transcription_text,
            transcription_controls,
            task_list,
            selected_task: RefCell::new(None),
        }
    }
    
    pub fn add_transcription_task(&self, task: TranscriptionTask) {
        // Create list item for task
        let row = adw::ActionRow::builder()
            .title(&task.file_name)
            .subtitle(&format!("Model: {}", task.model_name))
            .build();
        
        // Add status icon
        let status_icon = Image::builder()
            .icon_name(match task.status {
                TranscriptionStatus::Pending => "content-loading-symbolic",
                TranscriptionStatus::InProgress => "media-playback-start-symbolic",
                TranscriptionStatus::Completed => "task-complete-symbolic",
                TranscriptionStatus::Failed => "dialog-error-symbolic",
                TranscriptionStatus::Canceled => "media-playback-stop-symbolic",
            })
            .build();
        
        row.add_prefix(&status_icon);
        
        // Add progress bar for active tasks
        if matches!(task.status, TranscriptionStatus::InProgress) {
            let progress_bar = ProgressBar::builder()
                .fraction(task.progress)
                .margin_top(6)
                .margin_bottom(6)
                .build();
            
            let progress_box = Box::builder()
                .orientation(Orientation::Vertical)
                .build();
            
            progress_box.append(&progress_bar);
            row.add_suffix(&progress_box);
        }
        
        // Store task ID in row
        row.set_data("task_id", task.id);
        
        // Connect selection event
        let app_state = self.app_state.clone();
        row.connect_activated(clone!(@strong self.widget => move |row| {
            if let Some(task_id) = row.data::<Uuid>("task_id") {
                // Show transcription details
                show_transcription_details(&app_state, *task_id);
            }
        }));
        
        self.task_list.append(&row);
    }
    
    fn show_transcription_details(&self, task_id: Uuid) {
        *self.selected_task.borrow_mut() = Some(task_id);
        
        if let Some(task) = self.app_state.get_transcription_task(task_id) {
            if let Some(result) = &task.result {
                self.transcription_text.set_text(&result.text);
                self.transcription_text.set_segments(&result.segments);
            } else {
                self.transcription_text.set_text("Transcription not yet available");
            }
            
            // Update controls
            self.transcription_controls.update_for_task(&task);
        }
    }
}
```

## Dialog Components

### SettingsDialog

The settings dialog allows users to configure application preferences.

#### Structure

```rust
pub struct SettingsDialog {
    dialog: Dialog,
    app_state: Arc<AppState>,
    
    // UI components
    notebook: Notebook,
    general_page: Box,
    audio_page: Box,
    transcription_page: Box,
    advanced_page: Box,
    
    // Settings
    settings: RefCell<Settings>,
    modified_settings: RefCell<Settings>,
}
```

#### Implementation

```rust
impl SettingsDialog {
    pub fn new(parent: &ApplicationWindow, app_state: Arc<AppState>) -> Self {
        let dialog = Dialog::builder()
            .title("Settings")
            .modal(true)
            .transient_for(parent)
            .default_width(600)
            .default_height(500)
            .build();
        
        // Add action buttons
        dialog.add_button("Cancel", ResponseType::Cancel);
        dialog.add_button("Apply", ResponseType::Apply);
        dialog.add_button("Save", ResponseType::Accept);
        
        let settings = app_state.get_settings().clone();
        let modified_settings = settings.clone();
        
        // Create notebook for settings pages
        let notebook = Notebook::new();
        
        // Create settings pages
        let general_page = self.create_general_page(&modified_settings);
        let audio_page = self.create_audio_page(&modified_settings);
        let transcription_page = self.create_transcription_page(&modified_settings);
        let advanced_page = self.create_advanced_page(&modified_settings);
        
        // Add pages to notebook
        notebook_append_page(&notebook, &general_page, "General");
        notebook_append_page(&notebook, &audio_page, "Audio");
        notebook_append_page(&notebook, &transcription_page, "Transcription");
        notebook_append_page(&notebook, &advanced_page, "Advanced");
        
        // Add notebook to dialog
        let content_area = dialog.content_area();
        content_area.append(&notebook);
        
        Self {
            dialog,
            app_state,
            notebook,
            general_page,
            audio_page,
            transcription_page,
            advanced_page,
            settings: RefCell::new(settings),
            modified_settings: RefCell::new(modified_settings),
        }
    }
    
    fn create_general_page(&self, settings: &Settings) -> Box {
        let page = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        
        let group = adw::PreferencesGroup::builder()
            .title("Application")
            .build();
        
        // Theme selection
        let theme_row = adw::ActionRow::builder()
            .title("Theme")
            .subtitle("Select application theme")
            .build();
        
        let theme_combo = ComboBoxText::new();
        theme_combo.append("system", "System Default");
        theme_combo.append("light", "Light");
        theme_combo.append("dark", "Dark");
        
        match settings.general.theme {
            Theme::System => theme_combo.set_active_id(Some("system")),
            Theme::Light => theme_combo.set_active_id(Some("light")),
            Theme::Dark => theme_combo.set_active_id(Some("dark")),
        }
        
        theme_row.add_suffix(&theme_combo);
        group.add(&theme_row);
        
        // Auto-save setting
        let auto_save_row = adw::ActionRow::builder()
            .title("Auto-save Projects")
            .subtitle("Automatically save projects as you work")
            .build();
        
        let auto_save_switch = Switch::builder()
            .active(settings.general.auto_save)
            .build();
        
        auto_save_row.add_suffix(&auto_save_switch);
        auto_save_row.set_activatable_widget(Some(&auto_save_switch));
        group.add(&auto_save_row);
        
        page.append(&group);
        
        // Connect signals
        let modified_settings = self.modified_settings.clone();
        theme_combo.connect_changed(move |combo| {
            if let Some(id) = combo.active_id() {
                let theme = match id.as_str() {
                    "system" => Theme::System,
                    "light" => Theme::Light,
                    "dark" => Theme::Dark,
                    _ => Theme::System,
                };
                
                modified_settings.borrow_mut().general.theme = theme;
            }
        });
        
        let modified_settings = self.modified_settings.clone();
        auto_save_switch.connect_active_notify(move |switch| {
            modified_settings.borrow_mut().general.auto_save = switch.is_active();
        });
        
        page
    }
    
    pub fn show(&self) -> ResponseType {
        self.dialog.show();
        self.dialog.run()
    }
}
```

## Widget Components

### FileDropWidget

A widget that accepts drag-and-drop file operations.

#### Structure

```rust
pub struct FileDropWidget {
    widget: Box,
    app_state: Arc<AppState>,
    
    // UI components
    drop_target: DropTarget,
    icon: Image,
    label: Label,
    subtitle: Label,
    
    // State
    drag_active: Cell<bool>,
}
```

#### Implementation

```rust
impl FileDropWidget {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let widget = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .css_classes(vec!["file-drop-area".to_string()])
            .halign(Align::Center)
            .valign(Align::Center)
            .build();
        
        let icon = Image::builder()
            .icon_name("folder-documents-symbolic")
            .pixel_size(64)
            .build();
        
        let label = Label::builder()
            .label("Drop audio files here")
            .css_classes(vec!["title-2".to_string()])
            .build();
        
        let subtitle = Label::builder()
            .label("or click to select files")
            .css_classes(vec!["body".to_string()])
            .build();
        
        widget.append(&icon);
        widget.append(&label);
        widget.append(&subtitle);
        
        // Set up drag target
        let drop_target = DropTarget::new(FileType::static_type(), gdk::DragAction::COPY);
        
        let app_state_clone = app_state.clone();
        drop_target.connect_drop(move |_target, value, _x, _y| {
            if let Some(file_list) = value.get::<FileList>() {
                let mut file_paths = Vec::new();
                
                for i in 0..file_list.n_items() {
                    if let Some(file) = file_list.item(i) {
                        if let Some(file) = file.downcast_ref::<File>() {
                            if let Some(path) = file.path() {
                                file_paths.push(path);
                            }
                        }
                    }
                }
                
                // Add files to application
                let app_state = app_state_clone.clone();
                glib::spawn_future_local(async move {
                    for path in file_paths {
                        let _ = app_state.add_file(path).await;
                    }
                });
                
                true
            } else {
                false
            }
        });
        
        // Drag enter/leave handlers
        let widget_clone = widget.clone();
        drop_target.connect_enter(move |_target, _x, _y| {
            widget_clone.add_css_class("drag-active");
            gdk::DragAction::Copy
        });
        
        drop_target.connect_leave(move |_target| {
            widget.remove_css_class("drag-active");
        });
        
        widget.add_controller(&drop_target);
        
        // Click handler
        let gesture = GestureClick::new();
        let app_state_clone = app_state.clone();
        gesture.connect_pressed(move |_gesture, _n_press, _x, _y| {
            show_file_chooser(&app_state_clone);
        });
        
        Self {
            widget,
            app_state,
            drop_target,
            icon,
            label,
            subtitle,
            drag_active: Cell::new(false),
        }
    }
}

fn show_file_chooser(app_state: &AppState) {
    let file_chooser = FileChooserNative::builder()
        .title("Select Audio Files")
        .accept_label("Open")
        .cancel_label("Cancel")
        .modal(true)
        .action(FileChooserAction::Open)
        .select_multiple(true)
        .build();
    
    // Add file filter
    let filter = FileFilter::new();
    filter.add_mime_type("audio/*");
    filter.set_name(Some("Audio Files"));
    file_chooser.add_filter(&filter);
    
    file_chooser.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            if let Some(files) = dialog.files() {
                let mut file_paths = Vec::new();
                
                for i in 0..files.n_items() {
                    if let Some(file) = files.item(i) {
                        if let Some(file) = file.downcast_ref::<File>() {
                            if let Some(path) = file.path() {
                                file_paths.push(path);
                            }
                        }
                    }
                }
                
                // Add files to application state
                let app_state = app_state.clone();
                glib::spawn_future_local(async move {
                    for path in file_paths {
                        let _ = app_state.add_file(path).await;
                    }
                });
            }
        }
    });
    
    file_chooser.show();
}
```

### FileListWidget

A widget that displays a list of audio files.

#### Structure

```rust
pub struct FileListWidget {
    widget: ScrolledWindow,
    app_state: Arc<AppState>,
    
    // UI components
    list_view: ListView,
    selection_model: SingleSelection,
    
    // State
    file_store: RefCell<ListStore>,
}
```

#### Implementation

```rust
impl FileListWidget {
    pub fn new(app_state: Arc<AppState>) -> Self {
        // Create list store
        let file_store = ListStore::new::<glib::Object>();
        
        // Create selection model
        let selection_model = SingleSelection::new(Some(&file_store));
        selection_model.set_autoselect(false);
        selection_model.set_can_unselect(true);
        
        // Create signal factory
        let signal_factory = SignalListItemFactory::new();
        
        // Setup factory for creating list items
        signal_factory.connect_setup(move |_factory, list_item| {
            let box_ = Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(12)
                .margin_start(12)
                .margin_end(12)
                .margin_top(6)
                .margin_bottom(6)
                .build();
            
            let file_icon = Image::builder()
                .icon_name("audio-x-generic")
                .pixel_size(24)
                .build();
            
            let text_box = Box::builder()
                .orientation(Orientation::Vertical)
                .spacing(3)
                .hexpand(true)
                .build();
            
            let file_name = Label::builder()
                .halign(Align::Start)
                .ellipsize(EllipsizeMode::End)
                .css_classes(vec!["body".to_string()])
                .build();
            
            let file_details = Label::builder()
                .halign(Align::Start)
                .ellipsize(EllipsizeMode::End)
                .css_classes(vec!["caption".to_string()])
                .build();
            
            text_box.append(&file_name);
            text_box.append(&file_details);
            
            let status_icon = Image::builder()
                .icon_name("dialog-question")
                .pixel_size(20)
                .build();
            
            box_.append(&file_icon);
            box_.append(&text_box);
            box_.append(&status_icon);
            
            list_item.set_child(Some(&box_));
        });
        
        // Setup factory for binding data
        let app_state_clone = app_state.clone();
        signal_factory.connect_bind(move |_factory, list_item| {
            if let Some(file_item) = list_item.item() {
                if let Some(audio_file_id) = file_item.property::<Uuid>("file-id") {
                    if let Some(audio_file) = app_state_clone.get_audio_file(audio_file_id) {
                        if let Some(box_) = list_item.child() {
                            if let Ok(first_child) = box_.first_child() {
                                if let Ok(text_box) = first_child.next_sibling() {
                                    if let Ok(file_name) = text_box.first_child() {
                                        file_name.set_property("label", &audio_file.file_name);
                                    }
                                    
                                    if let Ok(file_details) = text_box.next_sibling() {
                                        let details = format!(
                                            "{:.1}s • {} • {}",
                                            audio_file.metadata.duration.as_secs_f32(),
                                            audio_file.metadata.sample_rate,
                                            audio_file.metadata.format
                                        );
                                        file_details.set_property("label", &details);
                                    }
                                }
                            }
                            
                            if let Ok(status_icon) = box_.last_child() {
                                let (icon_name, css_class) = match audio_file.status {
                                    FileStatus::Ready => ("task-complete-symbolic", "success"),
                                    FileStatus::Processing => ("content-loading-symbolic", "info"),
                                    FileStatus::Failed => ("dialog-error-symbolic", "error"),
                                    _ => ("dialog-question-symbolic", ""),
                                };
                                
                                status_icon.set_property("icon-name", icon_name);
                                
                                // Update CSS classes
                                status_icon.remove_css_class("success");
                                status_icon.remove_css_class("info");
                                status_icon.remove_css_class("error");
                                
                                if !css_class.is_empty() {
                                    status_icon.add_css_class(css_class);
                                }
                            }
                        }
                    }
                }
            }
        });
        
        // Create list view
        let list_view = ListView::new(Some(&selection_model), Some(&signal_factory));
        
        // Create scrolled window
        let widget = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .min_content_height(200)
            .child(&list_view)
            .build();
        
        // Setup selection handler
        selection_model.connect_selection_changed(move |model, _position, _n_items| {
            if let Some(selected) = model.selected_item() {
                if let Some(file_item) = selected.downcast_ref::<glib::Object>() {
                    if let Some(file_id) = file_item.property::<Uuid>("file-id") {
                        // Handle file selection
                        handle_file_selection(file_id);
                    }
                }
            }
        });
        
        Self {
            widget,
            app_state,
            list_view,
            selection_model,
            file_store: RefCell::new(file_store),
        }
    }
    
    pub fn refresh(&self) {
        let file_store = self.file_store.borrow_mut();
        file_store.remove_all();
        
        // Get files from app state
        let files = self.app_state.get_audio_files();
        
        for (file_id, audio_file) in files {
            let file_item = glib::Object::new::<glib::Object>();
            file_item.set_property("file-id", &file_id);
            
            file_store.append(&file_item);
        }
    }
    
    pub fn get_selected_files(&self) -> Vec<Uuid> {
        let mut selected_files = Vec::new();
        
        if let Some(selected) = self.selection_model.selected_item() {
            if let Some(file_item) = selected.downcast_ref::<glib::Object>() {
                if let Some(file_id) = file_item.property::<Uuid>("file-id") {
                    selected_files.push(file_id);
                }
            }
        }
        
        selected_files
    }
}
```

### ModelSelectorWidget

A widget for selecting AI transcription models.

#### Structure

```rust
pub struct ModelSelectorWidget {
    widget: Box,
    app_state: Arc<AppState>,
    
    // UI components
    model_list: ListBox,
    refresh_button: Button,
    
    // State
    selected_model: RefCell<Option<Uuid>>,
}
```

#### Implementation

```rust
impl ModelSelectorWidget {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let widget = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .build();
        
        // Create header with refresh button
        let header = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();
        
        let title = Label::builder()
            .label("Available Models")
            .css_classes(vec!["heading".to_string()])
            .hexpand(true)
            .halign(Align::Start)
            .build();
        
        let refresh_button = Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Refresh model list")
            .build();
        
        header.append(&title);
        header.append(&refresh_button);
        
        // Create model list
        let model_list = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .css_classes(vec!["navigation-sidebar".to_string()])
            .build();
        
        // Create scrolled window
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .min_content_height(300)
            .child(&model_list)
            .build();
        
        widget.append(&header);
        widget.append(&scrolled);
        
        Self {
            widget,
            app_state,
            model_list,
            refresh_button,
            selected_model: RefCell::new(None),
        }
    }
    
    pub fn refresh(&self) {
        // Clear existing items
        while let Some(row) = self.model_list.first_child() {
            self.model_list.remove(&row);
        }
        
        // Get models from app state
        let models = self.app_state.get_models();
        
        for model in &models {
            let row = adw::ActionRow::builder()
                .title(&model.display_name)
                .subtitle(&format!("{} • {:.1} MB", model.model_type, model.size_bytes as f64 / 1024.0 / 1024.0))
                .build();
            
            // Add status indicator
            let icon = Image::builder()
                .icon_name(if model.is_downloaded() {
                    "task-complete-symbolic"
                } else {
                    "folder-download-symbolic"
                })
                .build();
            
            row.add_prefix(&icon);
            
            // Add download indicator if not downloaded
            if !model.is_downloaded() {
                let download_label = Label::builder()
                    .label("Download")
                    .css_classes(vec!["accent".to_string()])
                    .build();
                
                row.add_suffix(&download_label);
            }
            
            // Store model ID
            row.set_data("model_id", model.id);
            
            // Connect selection event
            let app_state = self.app_state.clone();
            row.connect_activated(clone!(@strong self.selected_model => move |row| {
                if let Some(model_id) = row.data::<Uuid>("model_id") {
                    *selected_model.borrow_mut() = Some(*model_id);
                    
                    // Notify of selection change
                    app_state.set_selected_model_id(*model_id);
                }
            }));
            
            self.model_list.append(&row);
        }
        
        // Connect refresh button
        let app_state = self.app_state.clone();
        let model_list = self.model_list.clone();
        self.refresh_button.connect_clicked(move |_| {
            // Refresh model list
            glib::spawn_future_local({
                let app_state = app_state.clone();
                async move {
                    let _ = app_state.refresh_models().await;
                }
            });
        });
    }
}
```

### ModelDetailsWidget

A widget that displays detailed information about a selected model.

#### Structure

```rust
pub struct ModelDetailsWidget {
    widget: Box,
    app_state: Arc<AppState>,
    
    // UI components
    title_label: Label,
    description_label: Label,
    properties_group: adw::PreferencesGroup,
    performance_group: adw::PreferencesGroup,
    
    // State
    current_model: RefCell<Option<Uuid>>,
}
```

#### Implementation

```rust
impl ModelDetailsWidget {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let widget = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        
        let title_label = Label::builder()
            .label("Select a model to view details")
            .css_classes(vec!["title-3".to_string()])
            .halign(Align::Start)
            .build();
        
        let description_label = Label::builder()
            .label("")
            .css_classes(vec!["body".to_string()])
            .wrap(true)
            .wrap_mode(pango::WrapMode::Word)
            .halign(Align::Start)
            .build();
        
        let properties_group = adw::PreferencesGroup::builder()
            .title("Properties")
            .build();
        
        let performance_group = adw::PreferencesGroup::builder()
            .title("Performance")
            .build();
        
        widget.append(&title_label);
        widget.append(&description_label);
        widget.append(&properties_group);
        widget.append(&performance_group);
        
        Self {
            widget,
            app_state,
            title_label,
            description_label,
            properties_group,
            performance_group,
            current_model: RefCell::new(None),
        }
    }
    
    pub fn show_model(&self, model_id: Uuid) {
        *self.current_model.borrow_mut() = Some(model_id);
        
        if let Some(model) = self.app_state.get_model(model_id) {
            // Update basic information
            self.title_label.set_text(&model.display_name);
            
            let description = model.description
                .as_ref()
                .cloned()
                .unwrap_or_else(|| "No description available".to_string());
            
            self.description_label.set_text(&description);
            
            // Clear existing rows
            while let Some(row) = self.properties_group.first_child() {
                self.properties_group.remove(&row);
            }
            
            while let Some(row) = self.performance_group.first_child() {
                self.performance_group.remove(&row);
            }
            
            // Add property rows
            self.add_property_row("Type", &model.model_type.to_string());
            self.add_property_row("Size", &format!("{:.1} MB", model.size_bytes as f64 / 1024.0 / 1024.0));
            self.add_property_row("Languages", &format!("{} supported", model.supported_languages.len()));
            
            // Add performance rows
            if let Some(performance) = &model.performance {
                self.add_performance_row("Speed", format!("{:.1}x realtime", performance.speed_factor));
                self.add_performance_row("Accuracy", format!("{:.1}%", performance.accuracy * 100.0));
                self.add_performance_row("VRAM", format!("{:.1} GB", performance.vram_usage as f64 / 1024.0));
            }
        } else {
            // Clear display
            self.title_label.set_text("Model not found");
            self.description_label.set_text("");
            
            while let Some(row) = self.properties_group.first_child() {
                self.properties_group.remove(&row);
            }
        }
    }
    
    fn add_property_row(&self, title: &str, value: &str) {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(value)
            .build();
        
        self.properties_group.add(&row);
    }
    
    fn add_performance_row(&self, title: &str, value: String) {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(&value)
            .build();
        
        self.performance_group.add(&row);
    }
}
```

### TranscriptionTextWidget

A widget for displaying and editing transcription results.

#### Structure

```rust
pub struct TranscriptionTextWidget {
    widget: ScrolledWindow,
    app_state: Arc<AppState>,
    
    // UI components
    text_view: TextView,
    search_bar: SearchBar,
    search_entry: SearchEntry,
    
    // State
    current_task: RefCell<Option<Uuid>>,
    segments: RefCell<Vec<TranscriptionSegment>>,
}
```

#### Implementation

```rust
impl TranscriptionTextWidget {
    pub fn new(app_state: Arc<AppState>) -> Self {
        // Create text view
        let text_view = TextView::builder()
            .editable(true)
            .cursor_visible(true)
            wrap_mode(WrapMode::Word)
            left_margin(12)
            .right_margin(12)
            .top_margin(12)
            .bottom_margin(12)
            .build();
        
        // Create buffer with tag table for styling
        let buffer = TextBuffer::new(None);
        text_view.set_buffer(Some(&buffer));
        
        // Setup text tags
        self.setup_text_tags(&buffer);
        
        // Create search bar
        let search_bar = SearchBar::builder()
            .build();
        
        let search_entry = SearchEntry::builder()
            .placeholder_text("Search in transcription...")
            .build();
        
        search_bar.connect_entry(&search_entry);
        
        // Connect search functionality
        let text_view_clone = text_view.clone();
        let buffer_clone = buffer.clone();
        search_entry.connect_search_changed(move |entry| {
            let search_text = entry.text();
            
            if !search_text.is_empty() {
                // Highlight search results
                highlight_search_results(&buffer_clone, &search_text);
                
                // Jump to first match
                if let Some(start_iter) = find_next_match(&buffer_clone, &search_text, None) {
                    let end_iter = start_iter.clone();
                    end_iter.forward_chars(search_text.len());
                    
                    buffer_clone.select_range(&start_iter, &end_iter);
                    text_view_clone.scroll_to_iter(&start_iter, 0.0, false, 0.0, 0.0);
                }
            } else {
                // Clear highlights
                clear_search_highlights(&buffer_clone);
            }
        });
        
        // Create main container
        let vbox = Box::builder()
            .orientation(Orientation::Vertical)
            .build();
        
        vbox.append(&search_entry);
        vbox.append(&search_bar);
        vbox.append(&text_view);
        
        // Create scrolled window
        let widget = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .min_content_width(400)
            .child(&vbox)
            .build();
        
        Self {
            widget,
            app_state,
            text_view,
            search_bar,
            search_entry,
            current_task: RefCell::new(None),
            segments: RefCell::new(Vec::new()),
        }
    }
    
    fn setup_text_tags(&self, buffer: &TextBuffer) {
        // Tag for timestamps
        let timestamp_tag = TextTag::builder()
            .name("timestamp")
            .foreground("#3584e4")
            .weight(Pango::Weight::Bold)
            .build();
        
        buffer.tag_table().add(&timestamp_tag);
        
        // Tag for speaker labels
        let speaker_tag = TextTag::builder()
            .name("speaker")
            .foreground("#e01b24")
            .weight(Pango::Weight::Bold)
            .build();
        
        buffer.tag_table().add(&speaker_tag);
        
        // Tag for highlighted search results
        let highlight_tag = TextTag::builder()
            .name("highlight")
            .background("#f6d32d")
            .foreground("#000000")
            .build();
        
        buffer.tag_table().add(&highlight_tag);
    }
    
    pub fn set_text(&self, text: &str) {
        let buffer = self.text_view.buffer().unwrap();
        buffer.set_text(text);
    }
    
    pub fn set_segments(&self, segments: &[TranscriptionSegment]) {
        *self.segments.borrow_mut() = segments.to_vec();
        
        let buffer = self.text_view.buffer().unwrap();
        buffer.set_text("");
        
        let mut last_end = buffer.start_iter();
        
        for segment in segments {
            let iter = buffer.end_iter();
            
            // Add timestamp if available
            if let Some(timestamp) = &segment.timestamp {
                let timestamp_text = format!("[{}] ", format_timestamp(timestamp));
                buffer.insert(&mut last_end, &timestamp_text);
                
                // Apply timestamp tag
                let start_iter = buffer.end_iter();
                start_iter.backward_chars(timestamp_text.len());
                let end_iter = buffer.end_iter();
                buffer.apply_tag_by_name("timestamp", &start_iter, &end_iter);
                
                last_end = buffer.end_iter();
            }
            
            // Add speaker if available
            if let Some(speaker) = &segment.speaker {
                let speaker_text = format!("{}: ", speaker);
                buffer.insert(&mut last_end, &speaker_text);
                
                // Apply speaker tag
                let start_iter = buffer.end_iter();
                start_iter.backward_chars(speaker_text.len());
                let end_iter = buffer.end_iter();
                buffer.apply_tag_by_name("speaker", &start_iter, &end_iter);
                
                last_end = buffer.end_iter();
            }
            
            // Add text
            buffer.insert(&mut last_end, &segment.text);
            buffer.insert(&mut last_end, "\n\n");
        }
    }
    
    pub fn get_text(&self) -> String {
        let buffer = self.text_view.buffer().unwrap();
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
    }
}

fn format_timestamp(timestamp: &chrono::DateTime<chrono::Utc>) -> String {
    timestamp.format("%H:%M:%S").to_string()
}

fn highlight_search_results(buffer: &TextBuffer, search_text: &str) {
    clear_search_highlights(buffer);
    
    let (start, end) = buffer.bounds();
    let mut current = start.clone();
    
    while current < end {
        let mut match_start = current.clone();
        let mut match_end = current.clone();
        
        if buffer.forward_search(
            search_text,
            TextSearchFlags::CASE_INSENSITIVE,
            &mut match_start,
            &mut match_end,
            Some(&current)
        ) {
            buffer.apply_tag_by_name("highlight", &match_start, &match_end);
            current = match_end;
        } else {
            break;
        }
    }
}

fn clear_search_highlights(buffer: &TextBuffer) {
    if let Some(tag) = buffer.tag_table().lookup("highlight") {
        buffer.remove_tag(&tag, &buffer.start_iter(), &buffer.end_iter());
    }
}

fn find_next_match(
    buffer: &TextBuffer,
    search_text: &str,
    start_from: Option<&TextIter>
) -> Option<TextIter> {
    let start = start_from.copied().unwrap_or_else(|| buffer.start_iter());
    let (buffer_start, buffer_end) = buffer.bounds();
    
    let mut match_start = start.clone();
    let mut match_end = start.clone();
    
    if buffer.forward_search(
        search_text,
        TextSearchFlags::CASE_INSENSITIVE,
        &mut match_start,
        &mut match_end,
        Some(&start)
    ) {
        Some(match_start)
    } else {
        // Wrap around if no match found
        match_start = buffer_start;
        match_end = buffer_start;
        
        if buffer.forward_search(
            search_text,
            TextSearchFlags::CASE_INSENSITIVE,
            &mut match_start,
            &mut match_end,
            None
        ) {
            Some(match_start)
        } else {
            None
        }
    }
}
```

This document provides a detailed overview of the UI components used in the ASR Pro Linux frontend. Each component is designed with modularity and reusability in mind, following GTK4 best practices and the MVVM architectural pattern. The components are interconnected through the central AppState and communicate via events and callbacks, ensuring a clean separation of concerns and maintainable code.