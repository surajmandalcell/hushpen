//! Asks where to save a file. The test hook answers first when it has a path queued. Then the
//! platform dialog opens: the portal on Linux, the save panel on macOS. When the portal cannot
//! open on Linux, `rfd` tries the portal again and then zenity.

use gpui_kit::AsyncApp;
use std::path::PathBuf;

/// `Ok(None)` is a cancelled dialog, which is not an error.
pub async fn choose(
    cx: &mut AsyncApp,
    directory: PathBuf,
    name: String,
) -> Result<Option<PathBuf>, String> {
    #[cfg(feature = "test-automation")]
    if let Some(paths) = hushpen_testhook::dialogs::take_answer() {
        return Ok(paths.into_iter().next());
    }
    let asked = cx.update(|cx| cx.prompt_for_new_path(&directory, Some(&name)));
    match asked.await {
        Ok(Ok(chosen)) => Ok(chosen),
        Ok(Err(error)) => {
            log::warn!("the save dialog of the system did not open: {error}");
            fallback(cx, directory, name).await
        }
        Err(_) => Err("The save dialog was closed before it answered.".to_owned()),
    }
}

#[cfg(target_os = "linux")]
async fn fallback(
    cx: &mut AsyncApp,
    directory: PathBuf,
    name: String,
) -> Result<Option<PathBuf>, String> {
    // The dialog blocks until the user answers, so it runs off the UI thread.
    Ok(cx
        .background_executor()
        .spawn(async move {
            rfd::FileDialog::new()
                .set_title("Export transcripts")
                .set_directory(directory)
                .set_file_name(name)
                .save_file()
        })
        .await)
}

#[cfg(not(target_os = "linux"))]
async fn fallback(
    _cx: &mut AsyncApp,
    _directory: PathBuf,
    _name: String,
) -> Result<Option<PathBuf>, String> {
    Err("The save dialog could not be opened.".to_owned())
}
