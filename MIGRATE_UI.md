# ASR Pro UI Migration to React + Tauri

## Complete Feature Inventory for Frontend Migration

### 1. Core Application Features

#### 1.1 Main Window Management
- [ ] Frameless window with custom rounded corners
- [ ] Custom soft shadow rendering (CSS box-shadow equivalent)
- [ ] Window dragging from shadow areas
- [ ] Window controls (close, minimize, hide)
- [ ] System tray integration with menu
- [ ] Window position persistence across sessions
- [ ] High-DPI/Retina display support
- [ ] Window opacity control (configurable transparency)

#### 1.2 Navigation System
- [ ] Sidebar navigation with icons
- [ ] Page switching without full app reload
- [ ] Active section highlighting
- [ ] Navigation state management
- [ ] Breadcrumb support (if needed)

### 2. Settings Pages

#### 2.1 General Settings Page
- [ ] Launch at Login toggle
- [ ] Start Minimized toggle
- [ ] Auto-unload Model toggle (30 minutes)
- [ ] Settings persistence to local storage/config

#### 2.2 Models/Dictation Engine Page
- [ ] Current Model dropdown selection
- [ ] Language selection dropdown (Auto-detect + 10 languages)
- [ ] Processing Device selection (CPU, GPU CUDA, GPU Metal)
- [ ] Real-time Processing toggle
- [ ] Model Storage Location folder picker
- [ ] Model Cards display with:
  - Model name and description
  - File size information
  - Download status badges
  - Download/Select action buttons
- [ ] Model download progress tracking
- [ ] Model installation status management

#### 2.3 Microphone Settings Page
- [ ] Audio input device selection dropdown
- [ ] Device information display (name, channels, sample rate)
- [ ] Microphone permission status indicator
- [ ] Test microphone functionality
- [ ] Audio format settings (sample rate, channels)
- [ ] Audio quality settings

#### 2.4 Keyboard/Hotkey Settings Page
- [ ] Global hotkey combination input field
- [ ] Hotkey validation and testing
- [ ] Enable overlay toggle
- [ ] Auto-paste toggle
- [ ] Hotkey conflict detection
- [ ] Current hotkey display

#### 2.5 Transcribe Files Page
- [ ] Beautiful drag-and-drop zone with:
  - Animated gradient background
  - Hover effects and animations
  - File format support display
  - Click to browse functionality
- [ ] File processing progress display:
  - File name display
  - Progress bar with gradient
  - Status messages (Preparing, Converting, Transcribing, Saving)
  - Percentage completion
- [ ] Multi-file support (queue system)
- [ ] Output directory management
- [ - ] SRT file generation and saving
- [ ] Error handling and user feedback

#### 2.6 About Page
- [ ] Application information
- [ ] Version display
- [ ] License information
- [ ] Links and credits

### 3. Real-time Features

#### 3.1 Recording Overlay
- [ ] Full-screen overlay system
- [ ] Recording status indicator
- [ ] Visual feedback during recording
- [ ] Transcribing status display
- [ ] ESC key cancellation support
- [ ] Animated transitions

#### 3.2 System Tray Integration
- [ ] Tray icon with theme awareness
- [ ] Context menu with:
  - Show/Hide application
  - Settings
  - About
  - Quit
- [ ] Tray icon notifications
- [ ] Minimize to tray functionality

### 4. UI Components Library

#### 4.1 Layout Components
- [ ] Main window layout with sidebar and content area
- [ ] Responsive design considerations
- [ ] Scrollable content areas
- [ ] Proper spacing and margins

#### 4.2 Form Controls
- [ ] Custom toggle switch component
- [ ] Dropdown/select component with search
- [ ] File picker component
- [ ] Button component with states (normal, hover, pressed, disabled)
- [ ] Progress bar component
- [ ] Input field component

#### 4.3 Display Components
- [ ] Model card component
- [ ] Status badge component
- [ ] Icon component with SVG support
- [ ] Typography components (headings, labels, descriptions)
- [ ] Loading spinner component
- [ ] Toast/notification component

#### 4.4 Interactive Components
- [ ] Drag and drop zone component
- [ ] Recording overlay component
- [ ] System tray menu component
- [ ] Window controls component

### 5. Styling and Theming

#### 5.1 Dark Theme Implementation
- [ ] Color palette definition:
  - Primary background: #1e1e1e
  - Secondary background: #2d2d30
  - Card background: #252526
  - Primary text: #ffffff
  - Secondary text: #cccccc
  - Accent blue: #007acc
  - Success green: #4caf50
  - Warning yellow: #ff9800
  - Border colors: #3e3e42
- [ ] CSS custom properties for theming
- [ ] Component-level styling
- [ ] Hover and focus states
- [ ] Transition animations

#### 5.2 Typography
- [ ] Font family: Roboto or DM Sans
- [ ] Font size scale (12px to 20px)
- [ ] Font weights (Normal 400, Medium 500)
- [ ] Line height and spacing
- [ ] Responsive typography

#### 5.3 Animations and Transitions
- [ ] Smooth hover effects
- [ ] Page transition animations
- [ ] Loading animations
- [ ] Progress bar animations
- [ ] Overlay fade in/out

### 6. State Management

#### 6.1 Application State
- [ ] Settings state management
- [ ] Model status state
- [ ] Recording state
- [ ] File processing state
- [ ] UI state (active page, sidebar state)

#### 6.2 Data Persistence
- [ ] Local storage for settings
- [ ] Configuration file management
- [ ] User preferences persistence

### 7. Communication Layer

#### 7.1 Python Sidecar API Integration
- [ ] HTTP client for Python sidecar communication
- [ ] WebSocket support for real-time updates
- [ ] API error handling
- [ ] Request/response serialization
- [ ] Connection status monitoring

#### 7.2 API Endpoints Integration
- [ ] Model listing and management
- [ ] Transcription requests
- [ ] File upload/download
- [ ] Settings synchronization
- [ ] Status monitoring

### 8. Platform Integration

#### 8.1 Tauri-specific Features
- [ ] Window management API usage
- [ ] System tray API integration
- [ ] Global hotkey registration
- [ ] File system access
- [ ] Native dialogs
- [ ] Application lifecycle management

#### 8.2 Cross-platform Considerations
- [ ] macOS-specific behaviors
- [ ] Windows-specific behaviors
- [ ] Linux-specific behaviors
- [ ] Platform-specific UI adjustments

### 9. Performance Optimizations

#### 9.1 Rendering Performance
- [ ] Virtual scrolling for large lists
- [ ] Lazy loading of components
- [ ] Image optimization
- [ ] Bundle size optimization

#### 9.2 Memory Management
- [ ] Component cleanup
- [ ] Event listener management
- [ ] Large file handling optimization

### 10. Accessibility

#### 10.1 Keyboard Navigation
- [ ] Full keyboard accessibility
- [ ] Focus management
- [ ] Screen reader support
- [ ] ARIA labels and roles

#### 10.2 Visual Accessibility
- [ ] High contrast mode support
- [ ] Scalable UI
- [ ] Color blindness considerations
- [ ] Reduced motion support

## Implementation Strategy

### Phase 1: Project Setup
1. Initialize Tauri + React project
2. Set up build configuration
3. Configure TypeScript and ESLint
4. Set up CSS framework/styling solution

### Phase 2: Core Infrastructure
1. Implement window management
2. Set up state management (Redux/Zustand)
3. Create API communication layer
4. Implement routing system

### Phase 3: UI Components
1. Create component library
2. Implement layout system
3. Build form controls
4. Create display components

### Phase 4: Pages Implementation
1. Implement General settings
2. Implement Models page
3. Implement Microphone settings
4. Implement Keyboard settings
5. Implement Transcribe Files page
6. Implement About page

### Phase 5: Advanced Features
1. Implement recording overlay
2. Add system tray integration
3. Implement drag-and-drop
4. Add animations and transitions

### Phase 6: Integration and Testing
1. Integrate with Python sidecar
2. Test all features
3. Performance optimization
4. Cross-platform testing

## Technology Stack Recommendations

### Frontend
- **Framework**: React 18+ with TypeScript
- **State Management**: Zustand or Redux Toolkit
- **Styling**: CSS Modules + Styled Components or Tailwind CSS
- **Build Tool**: Vite
- **Routing**: React Router
- **Icons**: React Icons (SVG)
- **Forms**: React Hook Form + Zod validation

### Tauri Integration
- **Window Management**: Tauri window API
- **System Tray**: Tauri tray API
- **File System**: Tauri fs API
- **Dialogs**: Tauri dialog API
- **Global Hotkeys**: Tauri global shortcut API

### Communication
- **HTTP Client**: Axios or Fetch API
- **WebSocket**: Native WebSocket API or Socket.io
- **Serialization**: JSON

## File Structure Proposal

```
src/
├── components/
│   ├── common/
│   │   ├── Button/
│   │   ├── ToggleSwitch/
│   │   ├── Dropdown/
│   │   ├── FilePicker/
│   │   └── ProgressBar/
│   ├── layout/
│   │   ├── MainWindow/
│   │   ├── Sidebar/
│   │   └── ContentArea/
│   ├── pages/
│   │   ├── GeneralPage/
│   │   ├── ModelsPage/
│   │   ├── MicrophonePage/
│   │   ├── KeyboardPage/
│   │   ├── TranscribePage/
│   │   └── AboutPage/
│   └── overlays/
│       └── RecordingOverlay/
├── hooks/
├── services/
│   ├── api/
│   └── state/
├── utils/
├── styles/
├── types/
└── App.tsx
```

## Key Challenges and Solutions

### 1. Native Look and Feel
- **Challenge**: Replicating the exact native PyQt appearance
- **Solution**: Careful CSS styling with custom shadows, rounded corners, and animations

### 2. Real-time Communication
- **Challenge**: Coordinating between React frontend and Python sidecar
- **Solution**: WebSocket connection for real-time updates, HTTP for requests

### 3. File Processing
- **Challenge**: Large file uploads and processing feedback
- **Solution**: Chunked uploads, progress tracking, WebSocket status updates

### 4. Global Hotkeys
- **Challenge**: System-wide hotkey registration
- **Solution**: Tauri global shortcut API with proper error handling

### 5. Cross-platform Consistency
- **Challenge**: Maintaining consistent UI across platforms
- **Solution**: Platform-specific CSS adjustments and Tauri API abstractions
