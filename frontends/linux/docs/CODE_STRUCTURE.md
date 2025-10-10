# ASR Pro Linux Frontend Code Structure

This document provides a detailed overview of the code structure and organization of the ASR Pro Linux frontend. It explains the purpose of each module, their relationships, and the conventions used throughout the codebase.

## Table of Contents

- [Project Structure](#project-structure)
- [Module Organization](#module-organization)
- [Naming Conventions](#naming-conventions)
- [File Organization](#file-organization)
- [Module Dependencies](#module-dependencies)
- [Code Organization Patterns](#code-organization-patterns)
- [Testing Structure](#testing-structure)
- [Resource Management](#resource-management)

## Project Structure

```
frontends/linux/
├── Cargo.toml              # Rust project configuration
├── meson.build             # Build system configuration
├── README.md               # User documentation
├── src/                    # Source code
│   ├── main.rs             # Application entry point
│   ├── models/             # Data models and state management
│   ├── services/           # Business logic and external communication
│   ├── ui/                 # User interface components
│   ├── utils/              # Utility functions and helpers
│   └── resources/          # Resource management (optional)
├── data/                   # Application resources
│   ├── resources.xml       # GResource definition
│   ├── style.css           # Application styling
│   ├── icons/              # Application icons
│   └── com.asrpro.ASRPro.desktop # Desktop entry
├── scripts/                # Build and utility scripts
│   ├── install.sh          # Installation script
│   └── generate_icons.sh   # Icon generation script
├── tests/                  # Test files
│   ├── unit/               # Unit tests
│   ├── integration/        # Integration tests
│   └── common/             # Test utilities
└── docs/                   # Documentation
    ├── ARCHITECTURE.md     # Architecture overview
    ├── CODE_STRUCTURE.md   # This document
    └── API.md              # API documentation
```

## Module Organization

### Application Entry Point (`main.rs`)

The `main.rs` file serves as the application entry point and is responsible for:

- Parsing command-line arguments
- Initializing the GTK application
- Setting up the application state
- Building the user interface
- Handling application lifecycle events

```rust
// Key responsibilities in main.rs:
// 1. Parse command-line arguments
// 2. Initialize logging
// 3. Create GTK application
// 4. Initialize application state
// 5. Build and show UI
// 6. Run the application event loop
```

### Models Module (`src/models/`)

The models module contains all data structures and state management logic:

#### `mod.rs`
- Module declarations and re-exports
- Common type aliases
- Public API for the models module

#### `app_state.rs`
- `AppState` struct - Centralized application state
- State management methods
- Observer pattern implementation
- Thread-safe state access

```rust
pub struct AppState {
    // Core data collections
    pub audio_files: Arc<RwLock<HashMap<Uuid, AudioFile>>>,
    pub transcription_tasks: Arc<RwLock<HashMap<Uuid, TranscriptionTask>>>,
    pub models: Arc<RwLock<Vec<Model>>>,
    pub settings: Arc<RwLock<Settings>>,
    
    // UI state
    pub current_tab: Cell<TabType>,
    pub selected_files: RefCell<Vec<Uuid>>,
    pub status_message: RefCell<Option<String>>,
    
    // Event handling
    observers: RefCell<Vec<Box<dyn AppStateObserver>>>,
}
```

#### `audio_file.rs` / `file.rs`
- `AudioFile` struct - Audio file metadata and status
- `AudioMetadata` struct - Technical audio information
- File validation and processing logic
- File format support detection

#### `transcription.rs`
- `TranscriptionTask` struct - Transcription job information
- `TranscriptionResult` struct - Transcription output
- `TranscriptionStatus` enum - Task state tracking
- Progress tracking and status updates

#### `model.rs`
- `Model` struct - AI model information
- `ModelType` enum - Model categories
- `ModelStatus` enum - Model availability
- Model performance metrics

#### `settings.rs`
- `Settings` struct - Application configuration
- Various setting sub-structures (General, Audio, etc.)
- Settings validation and migration
- Configuration persistence

#### `api.rs`
- API request/response structures
- Backend communication data models
- Serialization/deserialization logic
- API version compatibility

#### `websocket.rs`
- WebSocket message structures
- Event type definitions
- Message parsing and formatting
- Connection state tracking

### Services Module (`src/services/`)

The services module contains business logic and external system communication:

#### `mod.rs`
- Service trait definitions
- Common service utilities
- Service factory functions
- Error handling for services

#### `backend_client.rs`
- `BackendClient` struct - HTTP API communication
- API endpoint implementations
- Request/response handling
- Authentication and security

```rust
pub struct BackendClient {
    client: reqwest::Client,
    base_url: String,
    auth_token: Option<String>,
    timeout: Duration,
}

impl BackendClient {
    pub async fn health_check(&self) -> AppResult<HealthResponse>
    pub async fn get_models(&self) -> AppResult<Vec<Model>>
    pub async fn start_transcription(&self, request: TranscriptionRequest) -> AppResult<Uuid>
    pub async fn get_transcription_status(&self, task_id: Uuid) -> AppResult<TranscriptionStatus>
}
```

#### `file_manager.rs`
- `FileManager` struct - File operations
- File validation and processing
- Metadata extraction
- File system operations

#### `model_manager.rs`
- `ModelManager` struct - AI model management
- Model downloading and caching
- Model version management
- Model performance tracking

#### `transcription_service.rs`
- `TranscriptionService` struct - Transcription orchestration
- Task lifecycle management
- Progress tracking
- Result processing

#### `websocket_client.rs`
- `WebSocketClient` struct - Real-time communication
- Connection management
- Message handling
- Event dispatching

#### `config_manager.rs`
- `ConfigManager` struct - Configuration management
- Settings persistence
- Configuration validation
- Migration handling

### UI Module (`src/ui/`)

The UI module contains all user interface components:

#### `mod.rs`
- UI component exports
- Common UI utilities
- Theme management
- UI initialization

#### `main_window.rs`
- `MainWindow` struct - Primary application window
- Window layout and structure
- Menu bar and toolbar
- Window event handling

```rust
pub struct MainWindow {
    window: ApplicationWindow,
    app_state: Arc<AppState>,
    
    // Main components
    header_bar: HeaderBar,
    main_paned: Paned,
    notebook: Notebook,
    
    // Panels
    file_panel: FilePanel,
    model_panel: ModelPanel,
    transcription_panel: TranscriptionPanel,
    
    // Status
    status_bar: StatusBar,
}
```

#### `menu_bar.rs`
- Application menu structure
- Menu item actions
- Keyboard shortcuts
- Menu state management

#### `file_panel.rs`
- `FilePanel` struct - File management interface
- File list display
- Drag-and-drop handling
- File selection management

#### `model_panel.rs`
- `ModelPanel` struct - Model selection interface
- Model list display
- Model details view
- Model management actions

#### `transcription_panel.rs`
- `TranscriptionPanel` struct - Results display
- Text view with editing
- Progress tracking
- Export functionality

#### `settings_dialog.rs`
- `SettingsDialog` struct - Configuration interface
- Settings categories
- Input validation
- Settings persistence

#### `style.rs`
- Theme management
- CSS styling
- Icon management
- Visual customization

#### Widgets Subdirectory (`src/ui/widgets/`)

Contains reusable UI components:

##### `mod.rs`
- Widget exports
- Common widget utilities
- Widget factory functions

##### `file_drop.rs`
- `FileDropWidget` - Drag-and-drop file handling
- Visual feedback for drag operations
- File validation on drop
- Multiple file support

##### `file_list.rs`
- `FileListWidget` - File display and selection
- List view with columns
- Sorting and filtering
- Context menu support

##### `model_selector.rs`
- `ModelSelectorWidget` - Model selection interface
- Model information display
- Download progress
- Model comparison

##### `transcription_text.rs`
- `TranscriptionTextWidget` - Text display and editing
- Syntax highlighting
- Timestamp navigation
- Search and replace

##### `waveform.rs`
- `WaveformWidget` - Audio visualization
- Waveform rendering
- Playback controls
- Time selection

### Utils Module (`src/utils/`)

The utils module contains utility functions and helpers:

#### `mod.rs`
- Utility exports
- Common functions
- Error type definitions

#### `error.rs`
- `AppError` enum - Application error types
- Error conversion utilities
- User-friendly error messages
- Error logging

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

pub type AppResult<T> = Result<T, AppError>;
```

#### `file_utils.rs`
- File system utilities
- Path manipulation
- File format detection
- Metadata extraction

#### `audio_processor.rs`
- Audio processing utilities
- Format conversion
- Audio analysis
- Quality checks

#### `platform.rs`
- Platform-specific code
- Desktop environment detection
- System integration
- OS-specific features

## Naming Conventions

### Files and Directories
- **Directories**: `snake_case` (e.g., `ui/widgets/`)
- **Files**: `snake_case.rs` (e.g., `main_window.rs`)
- **Module files**: `mod.rs` for module declarations
- **Test files**: `*_tests.rs` or `test_*.rs`

### Rust Code
- **Structs**: `PascalCase` (e.g., `AudioFile`, `TranscriptionTask`)
- **Enums**: `PascalCase` (e.g., `TranscriptionStatus`, `ModelType`)
- **Functions**: `snake_case` (e.g., `start_transcription`, `get_selected_files`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_TIMEOUT`, `MAX_FILE_SIZE`)
- **Modules**: `snake_case` (e.g., `file_manager`, `transcription_service`)

### GTK Widgets
- **Widget Structs**: `PascalCase` + `Widget` suffix (e.g., `FileDropWidget`)
- **Widget Properties**: `snake_case` (e.g., `file_path`, `is_selected`)
- **Signal Handlers**: `on_*` prefix (e.g., `on_file_drop`, `on_transcribe_clicked`)

### Error Types
- **Error Enum**: `PascalCase` + `Error` suffix (e.g., `AppError`)
- **Error Variants**: `PascalCase` (e.g., `BackendConnection`, `FileOperation`)
- **Result Type**: `AppResult<T>` alias for `Result<T, AppError>`

## File Organization

### Single Responsibility
Each file should have a single, well-defined responsibility:
- `audio_file.rs`: Only contains audio file related code
- `backend_client.rs`: Only contains backend API communication
- `file_drop.rs`: Only contains drag-and-drop widget implementation

### Module Organization
- Related functionality is grouped into modules
- Public APIs are exposed through `mod.rs`
- Private implementation details are kept in separate files
- Re-exports are used to provide clean public APIs

### Dependencies and Imports
```rust
// Standard library imports first
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// External crate imports next
use gtk4::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};

// Internal module imports last
use crate::models::AudioFile;
use crate::services::BackendClient;
use crate::utils::AppError;
```

## Module Dependencies

### Dependency Graph
```
main.rs
├── models::AppState
├── services (uses models)
│   ├── BackendClient
│   ├── FileManager
│   └── TranscriptionService
├── ui (uses models and services)
│   ├── MainWindow
│   ├── FilePanel
│   └── Widgets
└── utils (used by all modules)
    ├── AppError
    ├── file_utils
    └── platform
```

### Dependency Rules
1. **Models** should not depend on other modules
2. **Services** can depend on models but not on UI
3. **UI** can depend on models and services
4. **Utils** can be used by any module
5. **No circular dependencies** are allowed

### Interface Segregation
```rust
// Services define traits for their interfaces
pub trait TranscriptionService {
    async fn start_transcription(&self, file_id: Uuid) -> AppResult<Uuid>;
    async fn get_transcription(&self, task_id: Uuid) -> AppResult<TranscriptionResult>;
}

// Implementations can be swapped easily
pub struct RemoteTranscriptionService { /* ... */ }
pub struct LocalTranscriptionService { /* ... */ }
```

## Code Organization Patterns

### Factory Pattern
```rust
// Service factory for creating configured instances
pub struct ServiceFactory;

impl ServiceFactory {
    pub fn create_backend_client(config: &BackendConfig) -> AppResult<BackendClient> {
        Ok(BackendClient::new(config.url.clone(), config.api_key.clone()))
    }
    
    pub fn create_file_manager(app_state: Arc<AppState>) -> FileManager {
        FileManager::new(app_state)
    }
}
```

### Builder Pattern
```rust
// Builder for complex objects
pub struct TranscriptionRequestBuilder {
    file_id: Option<Uuid>,
    model_id: Option<Uuid>,
    language: Option<String>,
    include_timestamps: bool,
}

impl TranscriptionRequestBuilder {
    pub fn new() -> Self { /* ... */ }
    pub fn file_id(mut self, id: Uuid) -> Self { /* ... */ }
    pub fn model_id(mut self, id: Uuid) -> Self { /* ... */ }
    pub fn build(self) -> AppResult<TranscriptionRequest> { /* ... */ }
}
```

### Observer Pattern
```rust
// Observer pattern for state changes
pub trait AppStateObserver {
    fn on_files_changed(&self, files: &HashMap<Uuid, AudioFile>);
    fn on_transcription_updated(&self, task: &TranscriptionTask);
}

impl AppState {
    pub fn add_observer(&self, observer: Box<dyn AppStateObserver>) {
        self.observers.borrow_mut().push(observer);
    }
    
    fn notify_observers(&self, event: AppStateEvent) {
        for observer in self.observers.borrow().iter() {
            observer.handle_event(&event);
        }
    }
}
```

## Testing Structure

### Unit Tests
```rust
// Unit tests are co-located with the code
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_audio_file_creation() {
        let file = AudioFile::new(PathBuf::from("test.mp3"));
        assert_eq!(file.file_type(), AudioFileType::Mp3);
    }
    
    #[tokio::test]
    async fn test_backend_client_health_check() {
        let client = BackendClient::new("http://localhost:8000".to_string(), None);
        let result = client.health_check().await;
        assert!(result.is_ok());
    }
}
```

### Integration Tests
```
tests/
├── integration/
│   ├── backend_tests.rs      # Backend integration tests
│   ├── file_operations.rs    # File operation tests
│   └── ui_tests.rs          # UI integration tests
├── common/
│   ├── test_utils.rs        # Test utilities
│   └── mocks.rs            # Mock implementations
└── fixtures/
    ├── audio/               # Test audio files
    └── configs/             # Test configurations
```

### Test Utilities
```rust
// Common test utilities
pub struct TestApp {
    pub app_state: Arc<AppState>,
    pub backend_client: MockBackendClient,
    pub temp_dir: TempDir,
}

impl TestApp {
    pub fn new() -> Self {
        // Set up test environment
    }
    
    pub async fn add_test_file(&self, path: &str) -> Uuid {
        // Add test file to app state
    }
}
```

## Resource Management

### GResource Integration
```xml
<!-- data/resources.xml -->
<gresources>
  <gresource prefix="/com/asrpro/ASRPro">
    <file>style.css</file>
    <file>icons/16x16/com.asrpro.ASRPro.png</file>
    <file>icons/32x32/com.asrpro.ASRPro.png</file>
    <file>icons/scalable/com.asrpro.ASRPro.svg</file>
  </gresource>
</gresources>
```

### Resource Loading
```rust
// Resource loading utilities
pub struct Resources;

impl Resources {
    pub fn load_css() -> Result<CssProvider, Error> {
        let provider = CssProvider::new();
        provider.load_from_resource("/com/asrpro/ASRPro/style.css");
        Ok(provider)
    }
    
    pub fn load_icon(size: i32) -> Result<IconPaintable, Error> {
        let path = format!("/com/asrpro/ASRPro/icons/{}x{}/com.asrpro.ASRPro.png", size, size);
        IconPaintable::for_resource(&path)
    }
}
```

This code structure provides a solid foundation for the ASR Pro Linux frontend, ensuring maintainability, testability, and extensibility while following Rust and GTK4 best practices.