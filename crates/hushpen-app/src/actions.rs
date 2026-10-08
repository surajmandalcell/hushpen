//! The commands that the tray menu, the macOS app menu, and their key bindings share. Each one
//! becomes the same pipeline event or window change that a click in the app would make.

use gpui_kit::actions;

actions!(
    hushpen,
    [
        ToggleDictation,
        PasteLastTranscript,
        ShowHushpen,
        ShowSettings,
        ShowAbout,
        HideHushpen,
        QuitHushpen,
    ]
);
