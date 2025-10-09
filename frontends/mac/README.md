# ASR Pro SwiftUI Frontend

Native SwiftUI frontend for macOS.

## Overview

This SwiftUI frontend provides a native macOS interface for the ASRPro application, allowing users to transcribe audio files using the backend API. The frontend currently includes a basic UI structure with placeholders for backend integration.

## Features

- Native macOS interface built with SwiftUI
- Transcribe button with loading state
- Results area for displaying transcribed text
- Backend health status indicator
- Placeholder for audio file selection

## Requirements

- macOS 12.0 or later
- Xcode 14.0 or later
- Swift 5.7 or later

## Build Instructions

### Using Xcode

1. Open the project in Xcode:
   ```bash
   open frontends/mac/ASRPro.xcodeproj
   ```

2. Select your desired target or device in the scheme toolbar
3. Build and run the project (Cmd+R)

### Using Swift Package Manager

1. Navigate to the SwiftUI frontend directory:
   ```bash
   cd frontends/mac
   ```

2. Build the package:
   ```bash
   swift build
   ```

3. Run the executable:
   ```bash
   swift run ASRPro
   ```

## Dependencies

The following dependencies will be added when implementing full functionality:

- Alamofire or URLSession for HTTP requests to the backend API
- Audio file handling libraries for importing and processing audio files

## Backend Integration

The frontend is designed to connect to the ASRPro backend API running at `http://localhost:8000`. The following endpoints will be used:

- `GET /health` - Check backend service health
- `POST /transcribe` - Submit audio files for transcription

## Project Structure

```
frontends/mac/
├── Package.swift                 # Swift package configuration
├── README.md                    # This file
├── ASRPro/                      # Main application source
│   ├── ASRProApp.swift          # App entry point
│   ├── ContentView.swift        # Main view
│   └── Info.plist              # App configuration
└── ASRPro.xcodeproj/           # Xcode project files
```

## Development Notes

- Microphone and document folder access permissions are configured in Info.plist
- Network security settings allow connections to localhost for development
- The UI is designed to match the functionality of the GTK4 frontend
- Backend API integration is marked with placeholder comments in the source code

## Future Development

When implementing full functionality:

1. Add HTTP client for backend communication
2. Implement audio file selection and upload
3. Add real-time transcription progress updates
4. Implement error handling for network issues
5. Add support for different audio formats
6. Include settings for backend configuration