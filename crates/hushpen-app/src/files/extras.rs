//! The Mac extras of Transcribe: files from the Dock icon, Open With, and the Services menu;
//! Command V of a file or of text; the notification when the queue is through; and Quick Look
//! with Space on the selected row. Every way in ends where a drop on the window ends.

use super::Files;
use crate::main_window;
use crate::shell::Shell;
use crate::views::View;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use gpui_kit::component::input::Paste;
use gpui_kit::{
    App, ClipboardEntry, Entity, KeyBinding, SystemNotification, SystemNotificationAction,
    WeakEntity,
};
use std::path::PathBuf;

gpui_kit::actions!(hushpen, [QuickLook]);

/// The key context of the Transcribe queue, where Space opens Quick Look.
pub const QUEUE_CONTEXT: &str = "ImportQueue";

const DONE_TAG: &str = "queue-done";
const COPY_ACTION: &str = "copy";

/// Paths from outside the window: the Dock icon, Open With, and the Services menu. The flag
/// brings the window to the front even when Hushpen is not the active app.
pub type Opened = UnboundedSender<(Vec<PathBuf>, bool)>;

/// Where outside paths and pasted text land: the Manual view, as with a drop on the window.
#[derive(Clone)]
struct Route {
    shell: WeakEntity<Shell>,
    files: Entity<Files>,
}

impl Route {
    fn show(&self, cx: &mut App) {
        let _ = self
            .shell
            .update(cx, |shell, cx| shell.select(View::Manual, cx));
    }

    /// Text files open in Read aloud; the rest go in the queue.
    fn open(&self, paths: Vec<PathBuf>, cx: &mut App) {
        self.show(cx);
        let files = self.files.clone();
        // Read aloud fills its text area through the window, so this waits for the update to end.
        cx.defer(move |cx| files.update(cx, |files, cx| files.drop_paths(paths, cx)));
    }

    fn text(&self, text: String, cx: &mut App) {
        self.show(cx);
        let files = self.files.clone();
        cx.defer(move |cx| files.update(cx, |files, cx| files.paste_text(&text, cx)));
    }
}

/// Wires every extra to the window. `opened` carries what the platform asked the app to open,
/// from the start of the process on, so a file that launched Hushpen is not lost.
pub fn install(
    cx: &mut App,
    shell: WeakEntity<Shell>,
    files: Entity<Files>,
    sender: Opened,
    mut opened: UnboundedReceiver<(Vec<PathBuf>, bool)>,
) {
    let route = Route { shell, files };
    cx.bind_keys([
        // Inside a text field the field's own Command V wins; this one takes the rest.
        KeyBinding::new("secondary-v", Paste, None),
        KeyBinding::new("space", QuickLook, Some(QUEUE_CONTEXT)),
    ]);
    {
        let route = route.clone();
        cx.on_action(move |_: &Paste, cx| paste(&route, cx));
    }
    {
        let route = route.clone();
        cx.on_system_notification_response(move |response, cx| {
            if response.tag.as_ref() != DONE_TAG {
                return;
            }
            if response.action_id.as_deref() == Some(COPY_ACTION) {
                let import = route.files.read(cx).import().clone();
                import.update(cx, |import, cx| import.copy_all(cx));
            } else {
                main_window::show(cx);
                route.show(cx);
            }
        });
    }
    // A Services item is picked in another app, so Hushpen comes forward to show the queue.
    hushpen_platform::file_services::serve(Box::new(move |paths| {
        let _ = sender.unbounded_send((paths, true));
    }));
    cx.spawn(async move |cx| {
        while let Some((paths, front)) = opened.next().await {
            cx.update(|cx| {
                // The Dock and Open With make Hushpen active first; `open -g` does not, and
                // then the files queue without taking the front.
                if front || hushpen_platform::file_services::app_is_active() {
                    main_window::show(cx);
                }
                route.open(paths, cx);
            });
        }
    })
    .detach();
}

/// Command V outside a text field: copied files go in the queue, copied text in Read aloud.
fn paste(route: &Route, cx: &mut App) {
    let Some(item) = cx.read_from_clipboard() else {
        return;
    };
    let paths: Vec<PathBuf> = item
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            ClipboardEntry::ExternalPaths(paths) => Some(paths.paths().to_vec()),
            _ => None,
        })
        .flatten()
        .collect();
    if !paths.is_empty() {
        route.open(paths, cx);
    } else if let Some(text) = item.text().filter(|text| !text.trim().is_empty()) {
        route.text(text, cx);
    }
}

/// One notification for the whole queue, with a Copy button for every transcript.
pub fn queue_done_notification(body: String) -> SystemNotification {
    SystemNotification {
        tag: DONE_TAG.into(),
        title: "Transcripts are ready".into(),
        body: body.into(),
        actions: vec![SystemNotificationAction {
            id: COPY_ACTION.into(),
            label: "Copy".into(),
        }],
    }
}

pub use hushpen_platform::file_services::quick_look;

/// The paths of `file://` URLs, as the Dock and Open With send them. Other URLs are left out.
pub fn file_paths(urls: Vec<String>) -> Vec<PathBuf> {
    urls.iter()
        .filter_map(|url| url.strip_prefix("file://"))
        // A host such as `localhost` may sit before the path.
        .filter_map(|rest| rest.find('/').map(|at| &rest[at..]))
        .map(|path| PathBuf::from(percent_decode(path)))
        .collect()
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::file_paths;
    use std::path::PathBuf;

    #[test]
    fn file_urls_become_paths_and_other_urls_are_left_out() {
        let paths = file_paths(vec![
            "file:///Users/me/Talk%20one.m4a".to_owned(),
            "file://localhost/tmp/caf%C3%A9.wav".to_owned(),
            "https://localhost/a.wav".to_owned(),
            "file:///tmp/100%.mp3".to_owned(),
        ]);
        assert_eq!(
            paths,
            [
                PathBuf::from("/Users/me/Talk one.m4a"),
                PathBuf::from("/tmp/caf\u{e9}.wav"),
                PathBuf::from("/tmp/100%.mp3"),
            ]
        );
    }
}
