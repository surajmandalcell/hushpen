//! Files that come to the app from the system: the Services menu item "Transcribe with
//! Hushpen" (declared as `NSMessage` `transcribeFiles` in Info.plist), and the Quick Look panel.
//! Both are AppKit only; elsewhere they do nothing.
#![allow(unsafe_code)]

use std::path::PathBuf;

/// Whether the app is the active (front) app. `open -g` delivers files without making it so.
pub fn app_is_active() -> bool {
    #[cfg(target_os = "macos")]
    return objc2::MainThreadMarker::new().is_some_and(|mtm| {
        objc2_app_kit::NSApplication::sharedApplication(mtm).isActive()
    });
    #[cfg(not(target_os = "macos"))]
    true
}

/// Makes the app the Services provider; `opened` gets the paths of each Services request.
pub fn serve(opened: Box<dyn Fn(Vec<PathBuf>)>) {
    #[cfg(target_os = "macos")]
    mac::serve(opened);
    #[cfg(not(target_os = "macos"))]
    drop(opened);
}

/// Shows `path` in the Quick Look panel, or closes the panel when it already shows it. Call it
/// outside a GPUI update: the panel takes key, and AppKit tells the window at once.
pub fn quick_look(path: PathBuf) {
    #[cfg(target_os = "macos")]
    mac::quick_look(path);
    #[cfg(not(target_os = "macos"))]
    drop(path);
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, NSObject, NSObjectProtocol};
    use objc2::{
        ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    };
    use objc2_app_kit::{NSApplication, NSPasteboard, NSUpdateDynamicServices};
    use objc2_foundation::{NSArray, NSInteger, NSString, NSURL};
    use std::cell::{OnceCell, RefCell};
    use std::path::PathBuf;

    // QLPreviewPanel lives in Quartz; the class is looked up by name at run time.
    #[link(name = "Quartz", kind = "framework")]
    unsafe extern "C" {}

    define_class!(
        /// Answers the Services menu item.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "HushpenServices"]
        #[ivars = Box<dyn Fn(Vec<PathBuf>)>]
        struct Services;

        impl Services {
            #[unsafe(method(transcribeFiles:userData:error:))]
            fn transcribe_files(
                &self,
                pasteboard: &NSPasteboard,
                _data: Option<&NSString>,
                _error: *mut *mut NSString,
            ) {
                let paths = file_urls(pasteboard);
                log::info!("services: {} files to transcribe", paths.len());
                if !paths.is_empty() {
                    (self.ivars())(paths);
                }
            }
        }
    );

    define_class!(
        /// The one file the Quick Look panel shows.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "HushpenQuickLookSource"]
        #[ivars = RefCell<Option<Retained<NSURL>>>]
        struct PreviewSource;

        impl PreviewSource {
            #[unsafe(method(numberOfPreviewItemsInPreviewPanel:))]
            fn count(&self, _panel: &AnyObject) -> NSInteger {
                NSInteger::from(self.ivars().borrow().is_some())
            }

            #[unsafe(method(previewPanel:previewItemAtIndex:))]
            fn item(&self, _panel: &AnyObject, _index: NSInteger) -> *mut AnyObject {
                self.ivars()
                    .borrow()
                    .as_ref()
                    .map_or(std::ptr::null_mut(), |url| Retained::as_ptr(url).cast_mut().cast())
            }
        }
    );

    thread_local! {
        // AppKit keeps both as weak references, so they live here for the app's lifetime.
        static SERVICES: OnceCell<Retained<Services>> = const { OnceCell::new() };
        static SOURCE: OnceCell<Retained<PreviewSource>> = const { OnceCell::new() };
    }

    pub fn serve(opened: Box<dyn Fn(Vec<PathBuf>)>) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let services: Retained<Services> = {
            let this = Services::alloc(mtm).set_ivars(opened);
            unsafe { msg_send![super(this), init] }
        };
        // Safety: the provider is kept alive in SERVICES for the life of the app.
        unsafe { NSApplication::sharedApplication(mtm).setServicesProvider(Some(&services)) };
        NSUpdateDynamicServices();
        SERVICES.with(|cell| {
            let _ = cell.set(services);
        });
    }

    fn file_urls(pasteboard: &NSPasteboard) -> Vec<PathBuf> {
        let classes = NSArray::from_slice(&[NSURL::class()]);
        let Some(objects) = (unsafe { pasteboard.readObjectsForClasses_options(&classes, None) })
        else {
            return Vec::new();
        };
        objects
            .iter()
            .filter_map(|object| object.downcast::<NSURL>().ok())
            .filter_map(|url| url.path())
            .map(|path| PathBuf::from(path.to_string()))
            .collect()
    }

    pub fn quick_look(path: PathBuf) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(class) = AnyClass::get(c"QLPreviewPanel") else {
            log::warn!("Quick Look is not available: no QLPreviewPanel class");
            return;
        };
        let source = SOURCE.with(|cell| {
            cell.get_or_init(|| {
                let this = PreviewSource::alloc(mtm).set_ivars(RefCell::new(None));
                unsafe { msg_send![super(this), init] }
            })
            .clone()
        });
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let panel: Option<Retained<AnyObject>> = unsafe { msg_send![class, sharedPreviewPanel] };
        let Some(panel) = panel else {
            return;
        };
        let visible: bool = unsafe { msg_send![&*panel, isVisible] };
        let same = source
            .ivars()
            .borrow()
            .as_ref()
            .is_some_and(|shown| shown.isEqual(Some(&url)));
        if visible && same {
            let _: () = unsafe { msg_send![&*panel, orderOut: None::<&AnyObject>] };
            return;
        }
        *source.ivars().borrow_mut() = Some(url);
        unsafe {
            let _: () = msg_send![&*panel, setDataSource: &*source];
            let _: () = msg_send![&*panel, reloadData];
            let _: () = msg_send![&*panel, makeKeyAndOrderFront: None::<&AnyObject>];
        }
    }
}
