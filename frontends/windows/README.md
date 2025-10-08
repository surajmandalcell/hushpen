# ASR Pro Windows Frontend

Native WinUI/WPF frontend for Windows.

## Overview

This Windows frontend provides a native interface for the ASRPro application, allowing users to transcribe audio files using the backend API. The frontend currently includes a basic UI structure with placeholders for backend integration.

## Features

- Native Windows interface built with WPF (with WinUI migration path)
- Transcribe button with progress indicator
- Results area for displaying transcribed text
- Backend health status indicator
- Audio file browser
- Settings placeholder

## Requirements

- Windows 10 version 1903 or later
- .NET 6.0 Runtime or later
- Visual Studio 2022 or Visual Studio Build Tools

## Build Instructions

### Using Visual Studio

1. Open the solution in Visual Studio:
   ```
   Open frontends/windows/ASRPro.sln
   ```

2. Select your desired target platform in the solution configuration
3. Build the solution (Build > Build Solution or Ctrl+Shift+B)
4. Run the project (Debug > Start Debugging or F5)

### Using .NET CLI

1. Navigate to the Windows frontend directory:
   ```bash
   cd frontends/windows
   ```

2. Restore dependencies:
   ```bash
   dotnet restore
   ```

3. Build the project:
   ```bash
   dotnet build
   ```

4. Run the application:
   ```bash
   dotnet run --project ASRPro
   ```

## Dependencies

The following dependencies are included:

- Microsoft.Extensions.Http - For HTTP client functionality
- Microsoft.Extensions.DependencyInjection - For dependency injection
- Newtonsoft.Json - For JSON serialization

Additional dependencies will be added when implementing full functionality:
- Microsoft.WindowsAppSDK - For WinUI3 features (when migrating from WPF)
- Audio processing libraries for handling audio files

## Backend Integration

The frontend is designed to connect to the ASRPro backend API running at `http://localhost:8000`. The following endpoints will be used:

- `GET /health` - Check backend service health
- `POST /transcribe` - Submit audio files for transcription

## Project Structure

```
frontends/windows/
├── ASRPro.sln                  # Visual Studio solution file
├── README.md                   # This file
├── ASRPro/                     # Main application project
│   ├── ASRPro.csproj          # Project file
│   ├── App.xaml               # Application definition
│   ├── App.xaml.cs            # Application code-behind
│   ├── MainWindow.xaml        # Main window definition
│   └── MainWindow.xaml.cs     # Main window code-behind
└── Packages/                  # NuGet package cache
```

## Development Notes

- The project is currently using WPF with a clear migration path to WinUI3
- Dependency injection is configured in App.xaml.cs
- The UI is designed to match the functionality of the GTK4 and SwiftUI frontends
- Backend API integration is marked with placeholder comments in the source code
- Global styles are defined in App.xaml for consistent UI appearance

## Future Development

When implementing full functionality:

1. Add HTTP client for backend communication
2. Implement audio file selection and validation
3. Add real-time transcription progress updates
4. Implement error handling for network issues
5. Add support for different audio formats
6. Include settings window for backend configuration
7. Migrate from WPF to WinUI3 for modern Windows UI
8. Add toast notifications for transcription status
9. Implement file drag-and-drop functionality
10. Add keyboard shortcuts for common actions