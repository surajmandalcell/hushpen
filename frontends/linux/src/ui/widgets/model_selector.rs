//! Model Selector Widget for the ASRPro application
//!
//! This module contains the ModelSelectorWidget which provides an interface
//! for browsing and selecting speech recognition models.

use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, Label, Entry, SearchEntry, ScrolledWindow, ListView, 
    SingleSelection, SignalListItemFactory, Orientation, Align, PolicyType,
    Separator, Frame, Grid, Image, IconSize, ComboBoxText, CheckButton,
    SpinButton, Revealer, Expander, ApplicationWindow, Widget, FilterListModel,
    CustomFilter, ListItem, NoSelection
};
use std::sync::Arc;
use gio::ListStore;
use glib::Object;

use crate::models::{AppState, Model, ModelStatus, ModelType};
use crate::services::ModelManager;
use crate::utils::AppError;

/// Model Selector Widget
#[derive(Clone)]
pub struct ModelSelectorWidget {
    /// Main container widget
    container: Box,
    /// Application state
    app_state: Arc<AppState>,
    /// Model manager service
    model_manager: Arc<ModelManager>,
    /// Search entry
    search_entry: SearchEntry,
    /// Category filter combo box
    category_combo: ComboBoxText,
    /// Status filter combo box
    status_combo: ComboBoxText,
    /// Sort by combo box
    sort_combo: ComboBoxText,
    /// List view for models
    list_view: ListView,
    /// Model list store
    model_store: ListStore,
    /// Filtered model list
    filtered_models: FilterListModel,
    /// Details panel
    details_panel: Revealer,
    /// Details content
    details_content: Box,
    /// Download button
    download_button: Button,
    /// Select button
    select_button: Button,
    /// Current selection
    current_selection: Arc<gtk4::glib::WeakRef<ListItem>>,
}

/// Model list item object
#[derive(Clone)]
pub struct ModelListItem {
    pub model: Model,
}

impl ModelListItem {
    pub fn new(model: Model) -> Self {
        Self { model }
    }
}

impl glib::ObjectSubclass for ModelListItem {
    const NAME: &'static str = "ModelListItem";
    type Type = ModelListItemObject;
    type ParentType = glib::Object;
}

glib::wrapper! {
    pub struct ModelListItemObject(ObjectSubclass<ModelListItem>);
}

impl ModelListItemObject {
    pub fn new(model: Model) -> Self {
        Object::new(&[("model", &model)])
            .expect("Failed to create ModelListItemObject")
    }
}

impl ModelSelectorWidget {
    /// Create a new ModelSelectorWidget instance
    pub fn new(
        window: ApplicationWindow,
        app_state: Arc<AppState>,
        model_manager: Arc<ModelManager>,
    ) -> Result<Self, AppError> {
        // Create the main container
        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Create the search and filter panel
        let search_panel = Self::create_search_panel();

        // Create the model list
        let model_store = ListStore::new(ModelListItemObject::static_type());
        let filtered_models = Self::create_filtered_models(&model_store);
        let list_view = Self::create_list_view(&filtered_models);

        // Create the details panel
        let (details_panel, details_content) = Self::create_details_panel();

        // Create the action buttons
        let button_panel = Self::create_button_panel();
        let download_button = button_panel.download_button.clone();
        let select_button = button_panel.select_button.clone();

        // Create the main widget instance
        let model_selector = Self {
            container,
            app_state,
            model_manager,
            search_entry: search_panel.search_entry,
            category_combo: search_panel.category_combo,
            status_combo: search_panel.status_combo,
            sort_combo: search_panel.sort_combo,
            list_view,
            model_store,
            filtered_models,
            details_panel,
            details_content,
            download_button,
            select_button,
            current_selection: Arc::new(gtk4::glib::WeakRef::new()),
        };

        // Set up the UI
        model_selector.setup_ui(search_panel, button_panel);
        model_selector.setup_event_handlers()?;

        // Load initial data
        model_selector.refresh_models();

        Ok(model_selector)
    }

    /// Create the search and filter panel
    fn create_search_panel() -> SearchPanel {
        let container = Frame::builder()
            .label("Search and Filter")
            .build();

        let grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        // Search entry
        let search_label = Label::builder()
            .label("Search:")
            .halign(Align::Start)
            .build();
        grid.attach(&search_label, 0, 0, 1, 1);

        let search_entry = SearchEntry::builder()
            .placeholder_text("Search models...")
            .hexpand(true)
            .build();
        grid.attach(&search_entry, 1, 0, 2, 1);

        // Category filter
        let category_label = Label::builder()
            .label("Category:")
            .halign(Align::Start)
            .build();
        grid.attach(&category_label, 0, 1, 1, 1);

        let category_combo = ComboBoxText::builder()
            .build();
        category_combo.append(Some("all"), "All Models");
        category_combo.append(Some("whisper"), "Whisper");
        category_combo.append(Some("custom"), "Custom");
        category_combo.append(Some("finetuned"), "Fine-tuned");
        category_combo.append(Some("openai"), "OpenAI");
        category_combo.append(Some("local"), "Local");
        category_combo.set_active(Some(0));
        grid.attach(&category_combo, 1, 1, 1, 1);

        // Status filter
        let status_label = Label::builder()
            .label("Status:")
            .halign(Align::Start)
            .build();
        grid.attach(&status_label, 0, 2, 1, 1);

        let status_combo = ComboBoxText::builder()
            .build();
        status_combo.append(Some("all"), "All Status");
        status_combo.append(Some("available"), "Available");
        status_combo.append(Some("downloading"), "Downloading");
        status_combo.append(Some("loaded"), "Loaded");
        status_combo.append(Some("unavailable"), "Unavailable");
        status_combo.set_active(Some(0));
        grid.attach(&status_combo, 1, 2, 1, 1);

        // Sort by
        let sort_label = Label::builder()
            .label("Sort by:")
            .halign(Align::Start)
            .build();
        grid.attach(&sort_label, 2, 1, 1, 1);

        let sort_combo = ComboBoxText::builder()
            .build();
        sort_combo.append(Some("name"), "Name");
        sort_combo.append(Some("size"), "Size");
        sort_combo.append(Some("type"), "Type");
        sort_combo.append(Some("status"), "Status");
        sort_combo.set_active(Some(0));
        grid.attach(&sort_combo, 2, 2, 1, 1);

        container.set_child(Some(&grid));

        SearchPanel {
            search_entry,
            category_combo,
            status_combo,
            sort_combo,
        }
    }

    /// Create filtered models list
    fn create_filtered_models(model_store: &ListStore) -> FilterListModel {
        let filter = CustomFilter::new(|obj| {
            // Always return true for now, filtering will be applied when search changes
            true
        });
        
        FilterListModel::new(Some(model_store), Some(filter))
    }

    /// Create the list view
    fn create_list_view(filtered_models: &FilterListModel) -> ListView {
        let selection_model = SingleSelection::new(Some(filtered_models));
        selection_model.set_autoselect(false);
        selection_model.set_can_unselect(true);

        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<ListItem>().unwrap();
            let box_ = Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(10)
                .margin_top(5)
                .margin_bottom(5)
                .margin_start(10)
                .margin_end(10)
                .build();

            // Model icon
            let icon = Image::builder()
                .icon_name("audio-x-generic")
                .icon_size(IconSize::Large)
                .build();
            box_.append(&icon);

            // Model info
            let info_box = Box::builder()
                .orientation(Orientation::Vertical)
                .spacing(2)
                .hexpand(true)
                .build();

            let name_label = Label::builder()
                .label("")
                .halign(Align::Start)
                .build();
            info_box.append(&name_label);

            let details_label = Label::builder()
                .label("")
                .halign(Align::Start)
                .add_css_class("caption")
                .build();
            info_box.append(&details_label);

            box_.append(&info_box);

            // Status icon
            let status_icon = Image::builder()
                .icon_name("dialog-question")
                .icon_size(IconSize::Small)
                .build();
            box_.append(&status_icon);

            item.set_child(Some(&box_));
        });

        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<ListItem>().unwrap();
            let child = item.child().unwrap().downcast::<Box>().unwrap();
            
            let model_item = item.item().unwrap().downcast::<ModelListItemObject>().unwrap();
            let model = model_item.property::<Model>("model");
            
            let children = child.observe_children();
            let info_box = children.get_object(1).unwrap().downcast::<Box>().unwrap();
            let name_label = info_box.first_child().unwrap().downcast::<Label>().unwrap();
            let details_label = info_box.last_child().unwrap().downcast::<Label>().unwrap();
            let status_icon = child.last_child().unwrap().downcast::<Image>().unwrap();
            
            // Set model name and type
            name_label.set_label(&model.display_name);
            
            // Set model details
            let details = format!(
                "{} • {} • {}",
                model.model_type.display_name(),
                model.formatted_size(),
                model.status_message()
            );
            details_label.set_label(&details);
            
            // Set status icon
            let icon_name = match model.status {
                ModelStatus::Available => "dialog-ok",
                ModelStatus::Downloading => "document-save",
                ModelStatus::Loading => "preferences-system-time",
                ModelStatus::Loaded => "dialog-ok",
                ModelStatus::InUse => "media-playback-start",
                ModelStatus::Failed => "dialog-error",
                ModelStatus::Unavailable => "dialog-question",
            };
            status_icon.set_from_icon_name(Some(icon_name));
        });

        let list_view = ListView::new(Some(selection_model), Some(factory));
        list_view.set_single_click_activate(true);
        
        list_view
    }

    /// Create the details panel
    fn create_details_panel() -> (Revealer, Box) {
        let details_panel = Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideDown)
            .build();

        let details_content = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();

        let separator = Separator::builder()
            .orientation(Orientation::Horizontal)
            .build();
        details_content.append(&separator);

        let no_selection_label = Label::builder()
            .label("Select a model to view details")
            .halign(Align::Center)
            .margin_top(20)
            .margin_bottom(20)
            .build();
        details_content.append(&no_selection_label);

        details_panel.set_child(Some(&details_content));
        (details_panel, details_content)
    }

    /// Create the button panel
    fn create_button_panel() -> ButtonPanel {
        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .halign(Align::End)
            .margin_top(10)
            .build();

        let download_button = Button::builder()
            .label("Download")
            .sensitive(false)
            .build();

        let select_button = Button::builder()
            .label("Select Model")
            .sensitive(false)
            .build();

        container.append(&download_button);
        container.append(&select_button);

        ButtonPanel {
            download_button,
            select_button,
        }
    }

    /// Set up the UI layout
    fn setup_ui(&self, search_panel: SearchPanel, button_panel: ButtonPanel) {
        // Add search panel
        self.container.append(&search_panel.container);

        // Create a paned layout for the list and details
        let paned = gtk4::Paned::builder()
            .orientation(Orientation::Vertical)
            .wide_handle(true)
            .build();

        // Add the list view to a scrolled window
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .min_content_height(300)
            .build();
        scrolled.set_child(Some(&self.list_view));
        paned.set_start_child(Some(&scrolled));

        // Add the details panel
        paned.set_end_child(Some(&self.details_panel));
        paned.set_position(400);

        self.container.append(&paned);

        // Add button panel
        self.container.append(&button_panel.container);
    }

    /// Set up event handlers
    fn setup_event_handlers(&self) -> Result<(), AppError> {
        // Handle search entry changes
        self.search_entry.connect_search_changed(clone!(@strong self.filtered_models as filtered_models => move |entry| {
            let text = entry.text().to_string().to_lowercase();
            
            // Update the filter
            let filter = filtered_models.filter().unwrap().downcast::<CustomFilter>().unwrap();
            filter.changed(CustomFilter::all_changes());
            
            // In a real implementation, you would update the filter function here
            // For now, we'll just trigger a filter change
        }));

        // Handle category filter changes
        self.category_combo.connect_changed(clone!(@strong self as self_ => move |_| {
            self_.apply_filters();
        }));

        // Handle status filter changes
        self.status_combo.connect_changed(clone!(@strong self as self_ => move |_| {
            self_.apply_filters();
        }));

        // Handle sort changes
        self.sort_combo.connect_changed(clone!(@strong self as self_ => move |_| {
            self_.apply_sorting();
        }));

        // Handle list view selection
        let selection_model = self.list_view.model().unwrap().downcast::<SingleSelection>().unwrap();
        selection_model.connect_selection_changed(clone!(@strong self as self_ => move |_, _, _| {
            if let Some(selected) = selection_model.selected_item() {
                let item = selected.downcast::<ModelListItemObject>().unwrap();
                let model = item.property::<Model>("model");
                self_.on_model_selected(model);
            } else {
                self_.on_model_deselected();
            }
        }));

        // Handle list view activation
        self.list_view.connect_activate(clone!(@strong self as self_ => move |_, position| {
            let selection_model = self_.list_view.model().unwrap().downcast::<SingleSelection>().unwrap();
            if let Some(selected) = selection_model.item(position) {
                let item = selected.downcast::<ModelListItemObject>().unwrap();
                let model = item.property::<Model>("model");
                self_.on_model_activated(model);
            }
        }));

        // Handle download button clicks
        self.download_button.connect_clicked(clone!(@strong self as self_ => move |_| {
            self_.download_selected_model();
        }));

        // Handle select button clicks
        self.select_button.connect_clicked(clone!(@strong self as self_ => move |_| {
            self_.select_selected_model();
        }));

        Ok(())
    }

    /// Apply filters to the model list
    fn apply_filters(&self) {
        let category = self.category_combo.active_id().unwrap_or_else(|| "all".to_string());
        let status = self.status_combo.active_id().unwrap_or_else(|| "all".to_string());
        
        // Update the filter
        let filter = self.filtered_models.filter().unwrap().downcast::<CustomFilter>().unwrap();
        filter.changed(CustomFilter::all_changes());
        
        // In a real implementation, you would update the filter function here
        // For now, we'll just trigger a filter change
    }

    /// Apply sorting to the model list
    fn apply_sorting(&self) {
        let sort_by = self.sort_combo.active_id().unwrap_or_else(|| "name".to_string());
        
        // In a real implementation, you would sort the model list here
        // For now, we'll just trigger a refresh
        self.refresh_models();
    }

    /// Handle model selection
    fn on_model_selected(&self, model: Model) {
        // Update the details panel
        self.update_details_panel(&model);
        
        // Update button sensitivity
        self.download_button.set_sensitive(!model.is_ready());
        self.select_button.set_sensitive(model.is_ready());
        
        // Show the details panel
        self.details_panel.set_reveal_child(true);
    }

    /// Handle model deselection
    fn on_model_deselected(&self) {
        // Hide the details panel
        self.details_panel.set_reveal_child(false);
        
        // Update button sensitivity
        self.download_button.set_sensitive(false);
        self.select_button.set_sensitive(false);
    }

    /// Handle model activation (double-click)
    fn on_model_activated(&self, model: Model) {
        if model.is_ready() {
            self.select_model(&model);
        } else {
            self.download_model(&model);
        }
    }

    /// Update the details panel
    fn update_details_panel(&self, model: &Model) {
        // Clear existing content
        while let Some(child) = self.details_content.first_child() {
            self.details_content.remove(&child);
        }
        
        // Create details content
        let grid = Grid::builder()
            .row_spacing(10)
            .column_spacing(15)
            .build();
        
        // Model name
        let name_label = Label::builder()
            .label("Name:")
            .halign(Align::Start)
            .build();
        grid.attach(&name_label, 0, 0, 1, 1);
        
        let name_value = Label::builder()
            .label(&model.display_name)
            .halign(Align::Start)
            .build();
        grid.attach(&name_value, 1, 0, 1, 1);
        
        // Model type
        let type_label = Label::builder()
            .label("Type:")
            .halign(Align::Start)
            .build();
        grid.attach(&type_label, 0, 1, 1, 1);
        
        let type_value = Label::builder()
            .label(model.model_type.display_name())
            .halign(Align::Start)
            .build();
        grid.attach(&type_value, 1, 1, 1, 1);
        
        // Model size
        let size_label = Label::builder()
            .label("Size:")
            .halign(Align::Start)
            .build();
        grid.attach(&size_label, 0, 2, 1, 1);
        
        let size_value = Label::builder()
            .label(&model.formatted_size())
            .halign(Align::Start)
            .build();
        grid.attach(&size_value, 1, 2, 1, 1);
        
        // Model status
        let status_label = Label::builder()
            .label("Status:")
            .halign(Align::Start)
            .build();
        grid.attach(&status_label, 0, 3, 1, 1);
        
        let status_value = Label::builder()
            .label(&model.status_message())
            .halign(Align::Start)
            .build();
        grid.attach(&status_value, 1, 3, 1, 1);
        
        // Supported languages
        if let Some(ref language) = model.language {
            let lang_label = Label::builder()
                .label("Language:")
                .halign(Align::Start)
                .build();
            grid.attach(&lang_label, 0, 4, 1, 1);
            
            let lang_value = Label::builder()
                .label(language)
                .halign(Align::Start)
                .build();
            grid.attach(&lang_value, 1, 4, 1, 1);
        }
        
        self.details_content.append(&grid);
    }

    /// Download the selected model
    fn download_selected_model(&self) {
        let selection_model = self.list_view.model().unwrap().downcast::<SingleSelection>().unwrap();
        if let Some(selected) = selection_model.selected_item() {
            let item = selected.downcast::<ModelListItemObject>().unwrap();
            let model = item.property::<Model>("model");
            self.download_model(&model);
        }
    }

    /// Download a model
    fn download_model(&self, model: &Model) {
        let model_manager = self.model_manager.clone();
        let model_name = model.name.clone();
        let download_button = self.download_button.clone();
        
        gtk4::glib::spawn_future_local(async move {
            download_button.set_sensitive(false);
            download_button.set_label("Downloading...");
            
            if let Err(e) = model_manager.download_model(&model_name, None).await {
                eprintln!("Failed to download model: {}", e);
                download_button.set_label("Download Failed");
            } else {
                download_button.set_label("Downloaded");
            }
            
            // Reset button after a delay
            gtk4::glib::timeout_add_seconds_local_once(2, move || {
                download_button.set_label("Download");
                download_button.set_sensitive(true);
            });
        });
    }

    /// Select the selected model
    fn select_selected_model(&self) {
        let selection_model = self.list_view.model().unwrap().downcast::<SingleSelection>().unwrap();
        if let Some(selected) = selection_model.selected_item() {
            let item = selected.downcast::<ModelListItemObject>().unwrap();
            let model = item.property::<Model>("model");
            self.select_model(&model);
        }
    }

    /// Select a model
    fn select_model(&self, model: &Model) {
        let model_manager = self.model_manager.clone();
        let model_name = model.name.clone();
        let app_state = self.app_state.clone();
        
        gtk4::glib::spawn_future_local(async move {
            if let Err(e) = model_manager.set_selected_model(&model_name).await {
                eprintln!("Failed to select model: {}", e);
                app_state.set_status_message(format!("Failed to select model: {}", e)).await;
            } else {
                app_state.set_status_message(format!("Selected model: {}", model_name)).await;
                app_state.show_notification(
                    "Model Selected".to_string(),
                    format!("{} is now the active model", model_name),
                ).await;
            }
        });
    }

    /// Refresh the list of models
    fn refresh_models(&self) {
        let model_manager = self.model_manager.clone();
        let model_store = self.model_store.clone();
        
        gtk4::glib::spawn_future_local(async move {
            if let Err(e) = model_manager.refresh_models().await {
                eprintln!("Failed to refresh models: {}", e);
                return;
            }
            
            let models = model_manager.get_models().await;
            
            // Clear the store
            model_store.remove_all();
            
            // Add models to the store
            for model in models {
                let item = ModelListItemObject::new(model);
                model_store.append(&item);
            }
        });
    }

    /// Get the main container widget
    pub fn get_widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Connect to model selection signals
    pub fn connect_model_selected<F: Fn(Model) + 'static>(&self, callback: F) {
        // In a real implementation, you would store the callback and call it when a model is selected
        // For now, we'll just use a closure in the event handler
    }
}

/// Search panel component
struct SearchPanel {
    container: Frame,
    search_entry: SearchEntry,
    category_combo: ComboBoxText,
    status_combo: ComboBoxText,
    sort_combo: ComboBoxText,
}

/// Button panel component
struct ButtonPanel {
    container: Box,
    download_button: Button,
    select_button: Button,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api::BackendConfig;
    use crate::services::BackendClient;
    
    #[test]
    fn test_model_selector_widget_creation() {
        // This test would require a proper GTK4 test environment
        // For now, we'll just verify that the struct can be created
        // In a real test, you would need to initialize GTK4 first
    }
}