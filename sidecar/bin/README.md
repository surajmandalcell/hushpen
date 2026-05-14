# ASR Engine Binary Output

`npm run sidecar:build` writes the platform-specific ASR engine executable into this directory.

Electron packages this directory as `resources/sidecar/bin`, and production release builds require the executable to exist before installers are created.
