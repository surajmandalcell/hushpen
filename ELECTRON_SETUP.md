# Electron Setup for ASR Pro

This document outlines the Electron setup that replaces Tauri for the ASR Pro application.

## Project Structure

```
asrpro/
├── electron/
│   ├── main.ts              # Main Electron process
│   ├── preload.ts           # Preload script for security
│   ├── types.ts             # TypeScript type definitions
│   ├── package.json         # Electron-specific dependencies
│   ├── tsconfig.json        # TypeScript configuration
│   └── backend/
│       ├── server.ts        # Express server setup
│       ├── routes/          # API routes
│       ├── services/        # Backend services
│       └── utils/           # Utility functions
├── src/                     # React frontend (unchanged)
├── resources/               # Application icons and assets
└── electron.vite.config.ts  # Vite configuration for Electron
```

## Key Changes

### 1. Main Process (electron/main.ts)
- Creates the main application window
- Handles window controls and system tray
- Sets up secure communication with renderer process
- Will start the backend server when implemented

### 2. Preload Script (electron/preload.ts)
- Provides secure API to the renderer process
- Exposes window controls, file operations, and audio recording
- Implements event handling for real-time updates

### 3. Backend Server (electron/backend/server.ts)
- Express server for API endpoints
- WebSocket server for real-time communication
- Placeholder for Docker, audio, and config services

### 4. Configuration Updates
- Updated package.json with Electron dependencies and scripts
- Modified vite.config.ts for Electron compatibility
- Added electron-builder configuration for packaging

## Development Scripts

- `npm run electron:dev` - Start Electron in development mode
- `npm run electron:build` - Build Electron for production
- `npm run electron:build:win` - Build for Windows
- `npm run electron:build:mac` - Build for macOS
- `npm run electron:build:linux` - Build for Linux

## Migration Status

The Electron setup is now complete and ready for backend migration. The following components are ready:

- ✅ Electron main process
- ✅ Secure preload script
- ✅ Backend server structure
- ✅ Build configuration
- ✅ Development scripts

## Next Steps

1. Implement the backend services by migrating from the Python sidecar
2. Update the React frontend to use the Electron API instead of Tauri
3. Test the complete application
4. Remove Tauri dependencies (optional, after successful migration)

## Notes

- The Tauri configuration and files are preserved for reference during migration
- The React frontend structure remains unchanged
- The Python sidecar and Docker configurations are still available for reference