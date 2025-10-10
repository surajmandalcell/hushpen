# ASR Pro Linux Frontend Architecture

This document provides a comprehensive overview of the ASR Pro Linux frontend architecture, focusing on the Model-View-ViewModel (MVVM) pattern implementation and the overall system design.

## Table of Contents

- [Overview](#overview)
- [MVVM Pattern Implementation](#mvvm-pattern-implementation)
  - [Model Layer](#model-layer)
  - [View Layer](#view-layer)
  - [ViewModel Layer](#viewmodel-layer)
- [Component Architecture](#component-architecture)
- [Data Flow](#data-flow)
- [State Management](#state-management)
- [Threading Model](#threading-model)
- [Communication Patterns](#communication-patterns)
- [Error Handling](#error-handling)
- [Performance Considerations](#performance-considerations)
- [Extensibility](#extensibility)

## Overview

The ASR Pro Linux frontend is built using Rust and GTK4, following the Model-View-ViewModel (MVVM) architectural pattern. This design provides:

- **Separation of Concerns**: Clear boundaries between UI, business logic, and data
- **Testability**: Components can be tested in isolation
- **Maintainability**: Code is organized and easy to understand
- **Reusability**: Components can be reused across different parts of the application
- **Responsive UI**: Asynchronous operations don't block the user interface

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        GTK4 UI Layer                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ MainWindow  │  │   Dialogs   │  │      Widgets        │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    ViewModel Layer                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │UI Controllers│  │State Mgmt   │  │   Event Handlers    │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Model Layer                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │Data Models  │  │   State     │  │      Services       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   External Systems                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Backend    │  │ File System │  │   System Services   │  │
│  │    API      │  │             │  │                     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## MVVM Pattern Implementation

### Model Layer

The Model layer represents the data and business logic of the application. It's responsible for:

- Data structures and entities
- Business rules and validation
- External system communication
- Data persistence

#### Key Components

**Data Models** (`src/models/`)
```rust
// Audio file representation
pub struct AudioFile {
    pub id: Uuid,
    pub file_path: PathBuf,
    pub metadata: AudioMetadata,
    pub status: FileStatus,
}

// Transcription task
pub struct TranscriptionTask {
    pub id: Uuid,
    pub file_id: Uuid,
    pub model_id: Uuid,
    pub status: TranscriptionStatus,
    pub result: Option<TranscriptionResult>,
}

// Application state
pub struct AppState {
    pub audio_files: Arc<RwLock<HashMap<Uuid, AudioFile>>>,
    pub transcription_tasks: Arc<RwLock<HashMap<Uuid, TranscriptionTask>>>,
    pub models: Arc<RwLock<Vec<Model>>>,
    pub settings: Arc<RwLock<Settings>>,
}
```

**Services** (`src/services/`)
```rust
// Backend API communication
pub struct BackendClient {
    client: reqwest::Client,
    base_url: String,
}

// File operations
pub struct FileManager {
    app_state: Arc<AppState>,
}

// Model management
pub struct ModelManager {
    backend_client: Arc<BackendClient>,
}
```

### View Layer

The View layer is responsible for presenting the user interface and capturing user interactions. It's implemented using GTK4 widgets and follows a declarative approach.

#### Key Components

**Main Window** (`src/ui/main_window.rs`)
```rust
pub struct MainWindow {
    window: ApplicationWindow,
    app_state: Arc<AppState>,
    
    // UI components
    header_bar: HeaderBar,
    main_paned: Paned,
    file_panel: FilePanel,
    model_panel: ModelPanel,
    transcription_panel: TranscriptionPanel,
    status_bar: StatusBar,
}
```

**Custom Widgets** (`src/ui/widgets/`)
- `FileDropWidget`: Drag-and-drop file handling
- `TranscriptionTextWidget`: Text display with editing capabilities
- `WaveformWidget`: Audio visualization
- `ModelSelectorWidget`: Model selection interface

#### View Characteristics

- **Passive**: Views don't contain business logic
- **Observable**: Views observe state changes and update accordingly
- **Declarative**: UI structure is defined declaratively
- **Reusable**: Widgets are designed for reuse across the application

### ViewModel Layer

The ViewModel layer acts as a bridge between the Model and View layers. It's responsible for:

- Managing UI state
- Handling user interactions
- Coordinating between models
- Transforming data for display

#### Key Components

**UI Controllers** (`src/ui/`)
```rust
pub struct FilePanelController {
    app_state: Arc<AppState>,
    file_service: Arc<FileManager>,
    // UI state
    selected_files: RefCell<Vec<Uuid>>,
    drag_active: Cell<bool>,
}

impl FilePanelController {
    pub fn handle_file_drop(&self, file_paths: Vec<PathBuf>) {
        // Handle file drop logic
    }
    
    pub fn handle_file_selection(&self, file_id: Uuid) {
        // Handle file selection logic
    }
}
```

**State Management**
```rust
pub struct UIState {
    pub current_tab: Cell<TabType>,
    pub selected_model: RefCell<Option<Uuid>>,
    pub transcription_progress: RefCell<HashMap<Uuid, f32>>,
    pub status_message: RefCell<Option<String>>,
}
```

## Component Architecture

### Modular Structure

The application is organized into logical modules, each with specific responsibilities:

```
src/
├── main.rs              # Application entry point
├── models/              # Data models and state
│   ├── mod.rs
│   ├── app_state.rs     # Application state management
│   ├── audio_file.rs    # Audio file model
│   ├── transcription.rs # Transcription models
│   ├── model.rs         # AI model definitions
│   ├── settings.rs      # Configuration models
│   ├── api.rs           # API data structures
│   └── websocket.rs     # WebSocket message models
├── services/            # Business logic and external communication
│   ├── mod.rs
│   ├── backend_client.rs # HTTP API client
│   ├── file_manager.rs   # File operations
│   ├── model_manager.rs  # Model management
│   ├── transcription_service.rs # Transcription logic
│   ├── websocket_client.rs # WebSocket client
│   └── config_manager.rs # Configuration management
├── ui/                  # User interface components
│   ├── mod.rs
│   ├── main_window.rs   # Main application window
│   ├── menu_bar.rs      # Application menu
│   ├── file_panel.rs    # File management panel
│   ├── model_panel.rs   # Model selection panel
│   ├── transcription_panel.rs # Transcription results panel
│   ├── settings_dialog.rs # Settings configuration dialog
│   ├── style.rs         # UI styling
│   └── widgets/         # Reusable UI widgets
│       ├── file_drop.rs
│       ├── file_list.rs
│       ├── model_selector.rs
│       ├── transcription_text.rs
│       └── waveform.rs
└── utils/               # Utility functions and helpers
    ├── mod.rs
    ├── error.rs         # Error handling
    ├── file_utils.rs    # File utilities
    ├── audio_processor.rs # Audio processing
    └── platform.rs      # Platform integration
```

### Component Relationships

```
┌─────────────────┐    observes    ┌─────────────────┐
│    MainWindow   │◄───────────────│   AppState      │
└─────────────────┘                └─────────────────┘
         │                                   │
         │ contains                          │ provides
         ▼                                   ▼
┌─────────────────┐                ┌─────────────────┐
│   UI Panels     │                │   Services      │
│                 │                │                 │
│ ┌─────────────┐ │                │ ┌─────────────┐ │
│ │ FilePanel   │ │                │ │FileManager  │ │
│ └─────────────┘ │                │ └─────────────┘ │
│ ┌─────────────┐ │                │ ┌─────────────┐ │
│ │ModelPanel   │ │                │ │ModelManager │ │
│ └─────────────┘ │                │ └─────────────┘ │
│ ┌─────────────┐ │                │ ┌─────────────┐ │
│ │Transcript   │ │                │ │Transcript   │ │
│ │Panel        │ │                │ │Service      │ │
│ └─────────────┘ │                │ └─────────────┘ │
└─────────────────┘                └─────────────────┘
```

## Data Flow

### Unidirectional Data Flow

The application follows a unidirectional data flow pattern:

1. **User Action**: User interacts with the UI
2. **Event Handler**: View captures the event and notifies ViewModel
3. **State Update**: ViewModel updates the application state
4. **Model Update**: State changes are propagated to the Model layer
5. **Service Execution**: Services are called to perform business logic
6. **State Notification**: Models notify observers of state changes
7. **UI Update**: View observes state changes and updates the UI

### Data Flow Example

```rust
// 1. User clicks "Transcribe" button
transcribe_button.connect_clicked(move |_| {
    // 2. Event handler in ViewModel
    let app_state = ui_controller.app_state.clone();
    let selected_files = ui_controller.get_selected_files();
    
    // 3. Update state
    for file_id in selected_files {
        let task = TranscriptionTask::new(file_id, model_id);
        app_state.add_transcription_task(task);
    }
    
    // 4. Execute service
    let transcription_service = ui_controller.transcription_service.clone();
    for file_id in selected_files {
        transcription_service.start_transcription(file_id, model_id);
    }
});

// 5. Service updates model
impl TranscriptionService {
    pub async fn start_transcription(&self, file_id: Uuid, model_id: Uuid) {
        // Update task status
        let task = self.app_state.get_transcription_task(task_id);
        task.update_status(TranscriptionStatus::InProgress);
        
        // Notify observers
        self.app_state.notify_task_changed(task_id);
    }
}

// 6. UI observes and updates
impl TranscriptionPanel {
    pub fn setup_observers(&self, app_state: &AppState) {
        app_state.on_task_changed(|task| {
            // 7. Update UI
            self.update_task_display(task);
        });
    }
}
```

## State Management

### Centralized State

The application uses a centralized state management approach through the `AppState` struct:

```rust
pub struct AppState {
    // Core data
    pub audio_files: Arc<RwLock<HashMap<Uuid, AudioFile>>>,
    pub transcription_tasks: Arc<RwLock<HashMap<Uuid, TranscriptionTask>>>,
    pub models: Arc<RwLock<Vec<Model>>>,
    pub settings: Arc<RwLock<Settings>>,
    
    // UI state
    pub current_tab: Cell<TabType>,
    pub selected_files: RefCell<Vec<Uuid>>,
    pub status_message: RefCell<Option<String>>,
    
    // Observers
    observers: RefCell<Vec<Box<dyn Fn(&AppStateChange)>>>,
}
```

### State Access Patterns

**Read Operations**
```rust
// Read-only access
let files = app_state.audio_files.read().await;
for (id, file) in files.iter() {
    println!("File {}: {}", id, file.file_path.display());
}
```

**Write Operations**
```rust
// Write access with notification
{
    let mut files = app_state.audio_files.write().await;
    files.insert(file_id, new_file);
} // Lock is released here

// Notify observers
app_state.notify_files_changed();
```

### Reactive Updates

The application uses a reactive pattern for UI updates:

```rust
impl AppState {
    pub fn on_files_changed<F>(&self, callback: F)
    where
        F: Fn(&HashMap<Uuid, AudioFile>) + 'static,
    {
        let observer = Box::new(move |change: &AppStateChange| {
            if let AppStateChange::FilesChanged(files) = change {
                callback(files);
            }
        });
        
        self.observers.borrow_mut().push(observer);
    }
}
```

## Threading Model

### Main Thread UI Operations

GTK4 requires all UI operations to be performed on the main thread. The application uses several patterns to handle this:

**Async UI Updates**
```rust
use glib::MainContext;

// Perform work in background
let context = MainContext::default();
context.spawn_local(async move {
    // Background work
    let result = some_async_operation().await;
    
    // Update UI on main thread
    glib::idle_add_once(move || {
        // Safe to update UI here
        update_ui_with_result(result);
    });
});
```

**Channel-based Communication**
```rust
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::channel(100);

// Background thread
tokio::spawn(async move {
    let result = expensive_operation().await;
    tx.send(result).await.unwrap();
});

// Main thread
glib::spawn_future_local(async move {
    while let Some(result) = rx.recv().await {
        update_ui(result);
    }
});
```

### Background Processing

CPU-intensive operations are performed in background threads:

```rust
use tokio::task;

pub async fn process_audio(file_path: PathBuf) -> Result<AudioData, Error> {
    task::spawn_blocking(move || {
        // CPU-intensive work
        let audio_data = load_and_process_audio(file_path)?;
        Ok(audio_data)
    }).await?
}
```

## Communication Patterns

### Service Communication

Services communicate with each other through well-defined interfaces:

```rust
pub trait TranscriptionService {
    async fn start_transcription(&self, file_id: Uuid, model_id: Uuid) -> Result<Uuid, Error>;
    async fn get_transcription(&self, task_id: Uuid) -> Result<TranscriptionResult, Error>;
    async fn cancel_transcription(&self, task_id: Uuid) -> Result<(), Error>;
}
```

### Event-Driven Architecture

The application uses an event-driven architecture for loose coupling:

```rust
pub enum AppEvent {
    FileAdded(AudioFile),
    FileRemoved(Uuid),
    TranscriptionStarted(Uuid),
    TranscriptionProgress(Uuid, f32),
    TranscriptionCompleted(Uuid, TranscriptionResult),
    ErrorOccurred(Error),
}

pub trait EventHandler {
    fn handle_event(&self, event: &AppEvent);
}
```

## Error Handling

### Comprehensive Error Types

The application defines comprehensive error types for different contexts:

```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Backend connection failed: {source}")]
    BackendConnection { source: reqwest::Error },
    
    #[error("File operation failed: {path} - {source}")]
    FileOperation { path: PathBuf, source: std::io::Error },
    
    #[error("Transcription failed: {message}")]
    Transcription { message: String },
    
    #[error("Configuration error: {message}")]
    Configuration { message: String },
}
```

### Error Propagation

Errors are propagated using the `?` operator and `Result` types:

```rust
pub async fn start_transcription(&self, file_id: Uuid) -> AppResult<Uuid> {
    let file = self.get_file(file_id).await?;
    let model = self.get_selected_model().await?;
    
    let task_id = self.backend_client
        .start_transcription(&file.path, &model.name)
        .await?;
    
    Ok(task_id)
}
```

### User-Friendly Error Messages

Technical errors are converted to user-friendly messages:

```rust
impl AppError {
    pub fn user_message(&self) -> String {
        match self {
            AppError::BackendConnection { .. } => {
                "Failed to connect to the backend. Please check that the backend is running."
            },
            AppError::FileOperation { path, .. } => {
                format!("Could not access file: {}", path.display())
            },
            // ... other cases
        }
    }
}
```

## Performance Considerations

### Memory Management

**Efficient Data Structures**
```rust
// Use Arc<RwLock<T>> for shared data
pub struct AppState {
    pub files: Arc<RwLock<HashMap<Uuid, AudioFile>>>,
}

// Avoid unnecessary clones
impl AudioFile {
    pub fn path(&self) -> &Path {
        &self.file_path
    }
}
```

**Lazy Loading**
```rust
pub struct Model {
    pub id: Uuid,
    pub name: String,
    // Lazy loaded properties
    pub details: OnceCell<ModelDetails>,
    pub performance: OnceCell<PerformanceMetrics>,
}
```

### Async Operations

**Non-blocking Operations**
```rust
// Use async for I/O operations
pub async fn load_audio_metadata(path: &Path) -> Result<AudioMetadata, Error> {
    let file = tokio::fs::File::open(path).await?;
    // ... process file
}
```

**Concurrent Processing**
```rust
// Process multiple files concurrently
let tasks: Vec<_> = files.into_iter()
    .map(|file| process_file(file))
    .collect();

let results = futures::future::join_all(tasks).await;
```

## Extensibility

### Plugin Architecture

The application is designed to be extensible through a plugin architecture:

```rust
pub trait Plugin {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn initialize(&mut self, context: &PluginContext) -> Result<(), Error>;
    fn shutdown(&mut self) -> Result<(), Error>;
}

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}
```

### Configuration System

The configuration system supports extensions:

```rust
pub trait ConfigProvider {
    fn load_config(&self) -> Result<Config, Error>;
    fn save_config(&self, config: &Config) -> Result<(), Error>;
    fn watch_changes<F>(&self, callback: F) where F: Fn(&Config) + 'static;
}
```

### Service Registration

Services can be registered and discovered dynamically:

```rust
pub struct ServiceRegistry {
    services: HashMap<TypeId, Box<dyn Any>>,
}

impl ServiceRegistry {
    pub fn register<T: 'static>(&mut self, service: T) {
        self.services.insert(TypeId::of::<T>(), Box::new(service));
    }
    
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.services.get(&TypeId::of::<T>())
            .and_then(|s| s.downcast_ref())
    }
}
```

This architecture provides a solid foundation for the ASR Pro Linux frontend, ensuring maintainability, testability, and extensibility while delivering a responsive and user-friendly experience.