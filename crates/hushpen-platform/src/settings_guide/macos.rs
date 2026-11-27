//! The guide on macOS: a borderless, non-activating `NSPanel` that never becomes key, with one
//! view that draws the whole panel. A timer on the main run loop reads the Settings window from
//! `CGWindowListCopyWindowInfo`, so the guide needs no Accessibility trust.

// AppKit subclasses, window-server reads, and Objective-C calls with no safe binding.
#![allow(unsafe_code)]

use crate::permissions;
use block2::RcBlock;
use hushpen_core::permission::{Access, Permission, Preflight};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{
    AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send,
};
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, NSAnimatablePropertyContainer, NSAnimationContext,
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua,
    NSAppearanceNameDarkAqua, NSApplicationActivationPolicy, NSAttributedStringNSExtendedStringDrawing,
    NSAutoresizingMaskOptions, NSBackingStoreType, NSBezierPath, NSBitmapImageFileType,
    NSBitmapImageRep, NSColor, NSCompositingOperation, NSDeviceRGBColorSpace, NSDragOperation,
    NSDraggingContext, NSDraggingFormation, NSDraggingItem, NSDraggingSession, NSDraggingSource,
    NSEvent, NSFloatingWindowLevel, NSFont, NSFontAttributeName, NSFontWeight, NSFontWeightBold, NSFontWeightHeavy,
    NSFontWeightLight, NSFontWeightRegular, NSForegroundColorAttributeName,
    NSGraphicsContext, NSImage, NSImageSymbolConfiguration, NSPanel, NSPasteboard,
    NSPasteboardType, NSPasteboardTypeFileURL, NSPasteboardTypeString, NSPasteboardTypeURL,
    NSPasteboardWriting, NSRunningApplication, NSScreen, NSStringDrawingOptions, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowStyleMask, NSWorkspace,
};
use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID};
use objc2_foundation::{
    NSAffineTransform, NSArray, NSAttributedString, NSDictionary, NSFileManager,
    NSMutableAttributedString, NSNumber, NSPoint, NSProcessInfo, NSRect, NSRunLoop,
    NSRunLoopCommonModes, NSSize, NSString, NSTimer, NSURL, ns_string,
};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Placement, copied from PermissionFlow's FloatingDropPanel so the panel sits in the same place.
const SIDEBAR_WIDTH: f64 = 230.0;
const SCREEN_INSET: f64 = 12.0;
const MIN_WIDTH: f64 = 240.0;
/// The tracker ticks 10 times a second; while Settings is still it reads once every 10 ticks.
const TICK: f64 = 0.1;
const STILL_EVERY: u64 = 10;
/// Settings that moved within this long counts as moving.
const MOVING_FOR: Duration = Duration::from_secs(1);
const LIFETIME: Duration = Duration::from_secs(600);
/// Missed reads that close the guide: 2 once Settings was seen, 100 (10 s) while it opens.
const MISSES_SEEN: u32 = 2;
const MISSES_OPENING: u32 = 100;
const FLY_SECONDS: f64 = 0.45;
const FRAME_RATE: f64 = 60.0;

// The SwiftUI panel's geometry, in points.
const PAD_TOP: f64 = 8.0;
const PAD_BOTTOM: f64 = 12.0;
const PAD_SIDE: f64 = 12.0;
const ROW_GAP: f64 = 6.0;
const HEADER_GAP: f64 = 3.0;
const PANEL_RADIUS: f64 = 18.0;
const CARD_RADIUS: f64 = 16.0;
const CARD_PAD: f64 = 6.0;
const CARD_GAP: f64 = 8.0;
const ICON: f64 = 32.0;
const ICON_RADIUS: f64 = 10.0;
const HINT_TRAIL: f64 = 6.0;
const DRAG_ICON: f64 = 56.0;
const DRAG_SLOP: f64 = 4.0;
const ARROW_BOX: NSSize = NSSize::new(18.5, 22.0);
const CLOSE_BOX: NSSize = NSSize::new(24.0, 23.0);
const DRAG_ALPHA: f64 = 0.72;
// The close button: SwiftUI's palette `xmark.circle.fill` in a borderless button, measured from
// its 2x render. AppKit drops the palette circle offscreen, so the disc is drawn as a path.
const CLOSE_DISC: f64 = 18.0;
const CLOSE_DISC_ALPHA: f64 = 0.09;
const CLOSE_MARK: f64 = 8.0;
// The arrow wiggle: 0.22 s each way, +-12 degrees, 1 pt down to 2 pt up.
const WIGGLE_HALF: f64 = 0.22;
const WIGGLE_DEGREES: f64 = 12.0;

/// Accessibility and Input Monitoring take a dragged app; the Microphone list holds only apps
/// that asked, so it gets one plain line.
fn takes_drag(pane: Permission) -> bool {
    pane != Permission::Microphone
}

fn pane_name(pane: Permission) -> &'static str {
    match pane {
        // System Settings renamed the page in macOS 27; the link stays the same.
        Permission::Accessibility
            if NSProcessInfo::processInfo()
                .operatingSystemVersion()
                .majorVersion
                >= 27 =>
        {
            "Device Control and Data Access"
        }
        Permission::Accessibility => "Accessibility",
        Permission::InputMonitoring => "Input Monitoring",
        Permission::Microphone => "Microphone",
    }
}

fn granted(preflight: &dyn Preflight, pane: Permission) -> bool {
    let access = match pane {
        Permission::Microphone => preflight.microphone(),
        Permission::Accessibility => preflight.post_event(),
        Permission::InputMonitoring => preflight.listen_event(),
    };
    access == Access::Granted
}

/// The flat fill the offscreen snapshot draws for the material: SwiftUI's `ultraThinMaterial`
/// recipe (a darken or lighten layer, then a tint) over the page, without the blur.
struct StandIn {
    page: Retained<NSColor>,
    blend: Retained<NSColor>,
    blend_op: NSCompositingOperation,
    tint: Retained<NSColor>,
}

struct ViewState {
    drag: bool,
    header: Retained<NSAttributedString>,
    line: Retained<NSAttributedString>,
    hint: Retained<NSAttributedString>,
    icon: Retained<NSImage>,
    arrow: Option<Retained<NSImage>>,
    close: Option<Retained<NSImage>>,
    hand: Option<Retained<NSImage>>,
    bundle: Retained<NSURL>,
    wiggle_from: Option<Instant>,
    stand_in: Option<StandIn>,
    down: Cell<Option<NSPoint>>,
    dragged: Cell<bool>,
}

struct Layout {
    height: f64,
    arrow: NSRect,
    header: NSRect,
    close: NSRect,
    card: NSRect,
    line: NSRect,
    hint: NSRect,
}

define_class!(
    #[unsafe(super(NSPanel, objc2_app_kit::NSWindow, objc2_app_kit::NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "HushpenSettingsGuidePanel"]
    struct GuidePanel;

    impl GuidePanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            false
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            false
        }
    }
);

define_class!(
    #[unsafe(super(NSView, objc2_app_kit::NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "HushpenSettingsGuideView"]
    #[ivars = ViewState]
    struct GuideView;

    impl GuideView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw();
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            self.ivars().down.set(Some(self.point_of(event)));
            self.ivars().dragged.set(false);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let state = self.ivars();
            let Some(down) = state.down.get() else { return };
            let at = self.point_of(event);
            let layout = self.layout(self.bounds().size.width);
            if !state.drag || state.dragged.get() || !contains(layout.card, down) {
                return;
            }
            if (at.x - down.x).hypot(at.y - down.y) > DRAG_SLOP {
                state.dragged.set(true);
                self.begin_drag(event);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let close = self.layout(self.bounds().size.width).close;
            let down = self.ivars().down.take();
            if !self.ivars().dragged.get()
                && down.is_some_and(|down| contains(close, down))
                && contains(close, self.point_of(event))
            {
                request_close();
            }
        }
    }

    unsafe impl NSObjectProtocol for GuideView {}

    unsafe impl NSDraggingSource for GuideView {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operations(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            NSDragOperation::Copy
        }

        #[unsafe(method(ignoreModifierKeysForDraggingSession:))]
        fn ignore_modifier_keys(&self, _session: &NSDraggingSession) -> bool {
            true
        }

        #[unsafe(method(draggingSession:willBeginAtPoint:))]
        fn drag_began(&self, _session: &NSDraggingSession, _at: NSPoint) {
            // The panel lets the drop through to the Settings list under it.
            if let Some(window) = self.window() {
                window.setIgnoresMouseEvents(true);
                window.setAlphaValue(DRAG_ALPHA);
                window.orderBack(None);
            }
        }

        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        fn drag_ended(&self, _session: &NSDraggingSession, _at: NSPoint, _operation: NSDragOperation) {
            self.ivars().down.set(None);
            self.ivars().dragged.set(false);
            if let Some(window) = self.window() {
                window.setIgnoresMouseEvents(false);
                window.setAlphaValue(1.0);
                window.orderFrontRegardless();
            }
        }
    }
);

define_class!(
    /// The app bundle on the drag pasteboard, written the way a Finder file drag looks, which
    /// System Settings accepts more reliably than a bare URL (PermissionFlow's finding).
    #[unsafe(super(NSObject))]
    #[name = "HushpenSettingsGuideBundleWriter"]
    #[ivars = Retained<NSURL>]
    struct BundleWriter;

    unsafe impl NSObjectProtocol for BundleWriter {}

    unsafe impl NSPasteboardWriting for BundleWriter {
        #[unsafe(method_id(writableTypesForPasteboard:))]
        fn writable_types(&self, _pasteboard: &NSPasteboard) -> Retained<NSArray<NSPasteboardType>> {
            // SAFETY: AppKit's pasteboard type constants are immutable statics.
            let (file, url, string) =
                unsafe { (NSPasteboardTypeFileURL, NSPasteboardTypeURL, NSPasteboardTypeString) };
            NSArray::from_slice(&[
                file,
                url,
                ns_string!("NSFilenamesPboardType"),
                ns_string!("com.apple.pasteboard.promised-file-url"),
                string,
            ])
        }

        #[unsafe(method_id(pasteboardPropertyListForType:))]
        fn property_list(&self, kind: &NSPasteboardType) -> Option<Retained<AnyObject>> {
            property_list_of(self.ivars(), kind)
        }
    }
);

fn property_list_of(url: &NSURL, kind: &NSPasteboardType) -> Option<Retained<AnyObject>> {
    let path = url.path()?;
    // SAFETY: AppKit's pasteboard type constants are immutable statics.
    let string = unsafe { NSPasteboardTypeString };
    if kind.isEqualToString(ns_string!("NSFilenamesPboardType")) {
        Some(any(NSArray::from_retained_slice(&[path])))
    } else if kind.isEqualToString(string) {
        Some(any(path))
    } else {
        url.absoluteString().map(any)
    }
}

fn any<T: objc2::Message>(object: Retained<T>) -> Retained<AnyObject> {
    // SAFETY: every Objective-C object is an `AnyObject`.
    unsafe { Retained::cast_unchecked(object) }
}

fn contains(rect: NSRect, point: NSPoint) -> bool {
    point.x >= rect.origin.x
        && point.y >= rect.origin.y
        && point.x <= rect.origin.x + rect.size.width
        && point.y <= rect.origin.y + rect.size.height
}

fn overlaps(a: NSRect, b: NSRect) -> bool {
    a.origin.x < b.origin.x + b.size.width
        && b.origin.x < a.origin.x + a.size.width
        && a.origin.y < b.origin.y + b.size.height
        && b.origin.y < a.origin.y + a.size.height
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

fn text(parts: &[(&str, &NSFont)], color: &NSColor) -> Retained<NSAttributedString> {
    let whole = NSMutableAttributedString::new();
    for (part, font) in parts {
        // SAFETY: the AppKit attribute keys are immutable statics.
        let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
        let values: [&AnyObject; 2] = [font, color];
        let attributes = NSDictionary::from_slices(&keys, &values);
        // SAFETY: a font and a color under their own keys.
        let run = unsafe { NSAttributedString::new_with_attributes(&NSString::from_str(part), &attributes) };
        whole.appendAttributedString(&run);
    }
    Retained::into_super(whole)
}

fn measure(string: &NSAttributedString, width: f64) -> NSSize {
    let options =
        NSStringDrawingOptions::UsesLineFragmentOrigin | NSStringDrawingOptions::UsesFontLeading;
    let size = string
        .boundingRectWithSize_options_context(NSSize::new(width, 10_000.0), options, None)
        .size;
    NSSize::new(size.width.ceil(), size.height.ceil())
}

fn draw_text(string: &NSAttributedString, into: NSRect) {
    let options =
        NSStringDrawingOptions::UsesLineFragmentOrigin | NSStringDrawingOptions::UsesFontLeading;
    string.drawWithRect_options_context(into, options, None);
}

/// An SF Symbol at a point size and weight, in its palette colors, or one hierarchical color.
fn symbol(name: &str, size: f64, weight: NSFontWeight, colors: &[&NSColor]) -> Option<Retained<NSImage>> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None)?;
    let sized = NSImageSymbolConfiguration::configurationWithPointSize_weight(size, weight);
    let colored = match colors {
        [one] => NSImageSymbolConfiguration::configurationWithHierarchicalColor(one),
        many => NSImageSymbolConfiguration::configurationWithPaletteColors(&NSArray::from_slice(many)),
    };
    image.imageWithSymbolConfiguration(&sized.configurationByApplyingConfiguration(&colored))
}

fn centered(image: &NSImage, in_box: NSRect) -> NSRect {
    let size = image.size();
    rect(
        in_box.origin.x + (in_box.size.width - size.width) / 2.0,
        in_box.origin.y + (in_box.size.height - size.height) / 2.0,
        size.width,
        size.height,
    )
}

fn rounded(frame: NSRect, radius: f64) -> Retained<NSBezierPath> {
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(frame, radius, radius)
}

/// A filled rounded box with a 1 pt hairline drawn inside its edge, so no side is clipped.
fn card_box(frame: NSRect, radius: f64, fill: Option<&NSColor>, stroke: &NSColor) {
    if let Some(fill) = fill {
        fill.setFill();
        rounded(frame, radius).fill();
    }
    let inner = rect(
        frame.origin.x + 0.5,
        frame.origin.y + 0.5,
        frame.size.width - 1.0,
        frame.size.height - 1.0,
    );
    let path = rounded(inner, radius - 0.5);
    path.setLineWidth(1.0);
    stroke.setStroke();
    path.stroke();
}

impl GuideView {
    fn new(mtm: MainThreadMarker, pane: Permission, app: &Path, stand_in: Option<StandIn>) -> Retained<Self> {
        let path = NSString::from_str(&app.to_string_lossy());
        let bundle = NSURL::fileURLWithPath(&path);
        let shown = NSFileManager::defaultManager().displayNameAtPath(&path).to_string();
        let name = shown.strip_suffix(".app").unwrap_or(&shown).to_owned();
        let regular = NSFont::systemFontOfSize(14.0);
        let bold = NSFont::boldSystemFontOfSize(14.0);
        let label = NSColor::labelColor();
        let secondary = NSColor::secondaryLabelColor();
        let drag = takes_drag(pane);
        let pane_title = pane_name(pane);
        let header = if drag {
            text(
                &[
                    ("Drag ", &regular),
                    (&shown, &bold),
                    (" to the list above to allow ", &regular),
                    (pane_title, &bold),
                    (".", &regular),
                ],
                &label,
            )
        } else {
            text(
                &[
                    ("Allow ", &regular),
                    (&name, &bold),
                    (" in ", &regular),
                    (pane_title, &bold),
                    (".", &regular),
                ],
                &label,
            )
        };
        let card_text = if drag {
            name.clone()
        } else {
            format!("Turn on the switch next to {name}.")
        };
        // SAFETY: AppKit's font weight constants are immutable statics.
        let (w_regular, w_bold, w_light) = unsafe {
            (NSFontWeightRegular, NSFontWeightBold, NSFontWeightLight)
        };
        let line = text(&[(&card_text, &NSFont::systemFontOfSize(16.0))], &label);
        let hint = text(&[("Drag", &NSFont::systemFontOfSize_weight(8.0, w_light))], &secondary);
        let wiggle = !NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
            && stand_in.is_none();
        let state = ViewState {
            drag,
            header,
            line,
            hint,
            icon: NSWorkspace::sharedWorkspace().iconForFile(&path),
            arrow: symbol("arrowshape.up.fill", 14.0, w_bold, &[&NSColor::controlAccentColor()]),
            // SAFETY: an immutable AppKit static.
            close: symbol("xmark", CLOSE_MARK, unsafe { NSFontWeightHeavy }, &[&secondary, &secondary]),
            hand: symbol("hand.draw", 14.0, w_regular, &[&secondary, &secondary]),
            bundle,
            wiggle_from: wiggle.then(Instant::now),
            stand_in,
            down: Cell::new(None),
            dragged: Cell::new(false),
        };
        let this = Self::alloc(mtm).set_ivars(state);
        // SAFETY: `initWithFrame:` of NSView on a freshly allocated view.
        unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] }
    }

    fn point_of(&self, event: &NSEvent) -> NSPoint {
        self.convertPoint_fromView(event.locationInWindow(), None)
    }

    fn layout(&self, width: f64) -> Layout {
        let state = self.ivars();
        let header_x = PAD_SIDE + ARROW_BOX.width + HEADER_GAP;
        let header_width = width - header_x - HEADER_GAP - CLOSE_BOX.width - PAD_SIDE;
        let header = measure(&state.header, header_width);
        let row = header.height.max(CLOSE_BOX.height);
        let card_y = PAD_TOP + row + ROW_GAP;
        let card_width = width - PAD_SIDE * 2.0;
        let hint_width = if state.drag {
            let hand = state.hand.as_ref().map_or(0.0, |hand| hand.size().width);
            measure(&state.hint, 100.0).width.max(hand)
        } else {
            0.0
        };
        let line_x = PAD_SIDE + CARD_PAD + ICON + CARD_GAP;
        let line_width = card_width - CARD_PAD * 2.0 - ICON - CARD_GAP
            - if state.drag { CARD_GAP + hint_width + HINT_TRAIL } else { 0.0 };
        let line = measure(&state.line, line_width);
        let card_height = (ICON + CARD_PAD * 2.0).max(line.height + CARD_PAD * 2.0);
        Layout {
            height: card_y + card_height + PAD_BOTTOM,
            arrow: rect(PAD_SIDE, PAD_TOP, ARROW_BOX.width, ARROW_BOX.height),
            header: rect(header_x, PAD_TOP, header_width, header.height),
            close: rect(width - PAD_SIDE - CLOSE_BOX.width, PAD_TOP, CLOSE_BOX.width, CLOSE_BOX.height),
            card: rect(PAD_SIDE, card_y, card_width, card_height),
            line: rect(line_x, card_y + (card_height - line.height) / 2.0, line_width, line.height),
            hint: rect(
                PAD_SIDE + card_width - CARD_PAD - HINT_TRAIL - hint_width,
                card_y,
                hint_width,
                card_height,
            ),
        }
    }

    fn draw(&self) {
        let state = self.ivars();
        let bounds = self.bounds();
        let layout = self.layout(bounds.size.width);
        let label = NSColor::labelColor();
        if let Some(stand_in) = &state.stand_in {
            stand_in.page.setFill();
            NSBezierPath::fillRect(bounds);
            if let Some(context) = NSGraphicsContext::currentContext() {
                context.setCompositingOperation(stand_in.blend_op);
                stand_in.blend.setFill();
                rounded(bounds, PANEL_RADIUS).fill();
                context.setCompositingOperation(NSCompositingOperation::SourceOver);
            }
            stand_in.tint.setFill();
            rounded(bounds, PANEL_RADIUS).fill();
        }
        card_box(bounds, PANEL_RADIUS, None, &label.colorWithAlphaComponent(0.14));
        self.draw_arrow(layout.arrow);
        draw_text(&state.header, layout.header);
        let disc = layout.close;
        let disc = rect(
            disc.origin.x + (disc.size.width - CLOSE_DISC) / 2.0,
            disc.origin.y + (disc.size.height - CLOSE_DISC) / 2.0,
            CLOSE_DISC,
            CLOSE_DISC,
        );
        label.colorWithAlphaComponent(CLOSE_DISC_ALPHA).setFill();
        NSBezierPath::bezierPathWithOvalInRect(disc).fill();
        if let Some(close) = &state.close {
            close.drawInRect(centered(close, layout.close));
        }
        let card_fill = NSColor::textBackgroundColor().colorWithAlphaComponent(0.65);
        card_box(layout.card, CARD_RADIUS, Some(&card_fill), &label.colorWithAlphaComponent(0.085));
        let icon = rect(layout.card.origin.x + CARD_PAD, layout.card.origin.y + CARD_PAD, ICON, ICON);
        NSGraphicsContext::saveGraphicsState_class();
        rounded(icon, ICON_RADIUS).addClip();
        state.icon.drawInRect(icon);
        NSGraphicsContext::restoreGraphicsState_class();
        draw_text(&state.line, layout.line);
        if state.drag {
            self.draw_hint(layout.hint);
        }
    }

    /// The header arrow, wiggling like PermissionFlow's unless Reduce Motion is on.
    fn draw_arrow(&self, frame: NSRect) {
        let Some(arrow) = &self.ivars().arrow else { return };
        let (degrees, lift) = self.ivars().wiggle_from.map_or((0.0, 0.0), |from| {
            let swing = (std::f64::consts::PI * from.elapsed().as_secs_f64() / WIGGLE_HALF).cos();
            (-WIGGLE_DEGREES * swing, 1.0 - 3.0 * (1.0 - swing) / 2.0)
        });
        let at = centered(arrow, frame);
        let (mid_x, mid_y) = (at.origin.x + at.size.width / 2.0, at.origin.y + at.size.height / 2.0);
        NSGraphicsContext::saveGraphicsState_class();
        let turn = NSAffineTransform::transform();
        turn.translateXBy_yBy(mid_x, mid_y + lift);
        turn.rotateByDegrees(degrees);
        turn.translateXBy_yBy(-mid_x, -mid_y);
        turn.concat();
        arrow.drawInRect(at);
        NSGraphicsContext::restoreGraphicsState_class();
    }

    /// The hand and the word "Drag", stacked and centered on the right of the card.
    fn draw_hint(&self, frame: NSRect) {
        let state = self.ivars();
        let hand = state.hand.as_ref().map_or(NSSize::ZERO, |hand| hand.size());
        let word = measure(&state.hint, 100.0);
        let top = frame.origin.y + (frame.size.height - hand.height - word.height) / 2.0;
        let mid = frame.origin.x + frame.size.width / 2.0;
        if let Some(image) = &state.hand {
            image.drawInRect(rect(mid - hand.width / 2.0, top, hand.width, hand.height));
        }
        draw_text(&state.hint, rect(mid - word.width / 2.0, top + hand.height, word.width, word.height));
    }

    fn begin_drag(&self, event: &NSEvent) {
        let state = self.ivars();
        let writer = BundleWriter::alloc().set_ivars(state.bundle.clone());
        // SAFETY: `init` of NSObject on a freshly allocated object.
        let writer: Retained<BundleWriter> = unsafe { msg_send![super(writer), init] };
        let item = NSDraggingItem::initWithPasteboardWriter(
            NSDraggingItem::alloc(),
            ProtocolObject::from_ref(&*writer),
        );
        let icon = NSWorkspace::sharedWorkspace().iconForFile(&state.bundle.path().unwrap_or_default());
        icon.setSize(NSSize::new(DRAG_ICON, DRAG_ICON));
        let at = self.point_of(event);
        let half = DRAG_ICON / 2.0;
        // SAFETY: an NSImage is valid dragging contents.
        unsafe { item.setDraggingFrame_contents(rect(at.x - half, at.y - half, DRAG_ICON, DRAG_ICON), Some(&icon)) };
        let session = self.beginDraggingSessionWithItems_event_source(
            &NSArray::from_retained_slice(&[item]),
            event,
            ProtocolObject::from_ref(self),
        );
        session.setAnimatesToStartingPositionsOnCancelOrFail(true);
        session.setDraggingFormation(NSDraggingFormation::None);
    }
}

struct Guide {
    pane: Permission,
    panel: Retained<GuidePanel>,
    view: Retained<GuideView>,
    timers: Vec<Retained<NSTimer>>,
    preflight: Arc<dyn Preflight>,
    started: Instant,
    ticks: u64,
    last: Option<NSRect>,
    moved_at: Instant,
    misses: u32,
    source: Option<NSPoint>,
    closing: bool,
}

thread_local! {
    static GUIDE: RefCell<Option<Guide>> = const { RefCell::new(None) };
}

fn app_bundle() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| permissions::bundle_of(&exe))
        .unwrap_or_else(|| PathBuf::from(objc2_foundation::NSBundle::mainBundle().bundlePath().to_string()))
}

fn repeating(interval: f64, job: impl Fn() + 'static) -> Retained<NSTimer> {
    let block = RcBlock::new(move |_: NonNull<NSTimer>| job());
    // SAFETY: the block runs on the main run loop; the timer is invalidated when the guide closes.
    let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(interval, true, &block) };
    // Common modes keep it running while a drag or a menu tracks the mouse.
    // SAFETY: `NSRunLoopCommonModes` is an immutable static.
    unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
    timer
}

pub(super) fn show(pane: Permission) {
    let Some(mtm) = MainThreadMarker::new() else {
        log::warn!("the settings guide needs the main thread");
        return;
    };
    teardown();
    let view = GuideView::new(mtm, pane, &app_bundle(), None);
    let panel = make_panel(mtm, &view);
    let mut timers = vec![repeating(TICK, tick)];
    if view.ivars().wiggle_from.is_some() {
        let redraw = view.clone();
        timers.push(repeating(1.0 / FRAME_RATE, move || {
            let frame = redraw.layout(redraw.bounds().size.width).arrow;
            // The tilted arrow reaches a little past its box.
            redraw.setNeedsDisplayInRect(rect(frame.origin.x - 4.0, frame.origin.y - 4.0, frame.size.width + 8.0, frame.size.height + 8.0));
        }));
    }
    let now = Instant::now();
    GUIDE.with_borrow_mut(|guide| {
        *guide = Some(Guide {
            pane,
            panel,
            view,
            timers,
            preflight: permissions::system(),
            started: now,
            ticks: 0,
            last: None,
            moved_at: now,
            misses: 0,
            source: Some(NSEvent::mouseLocation()),
            closing: false,
        });
    });
    tick();
}

fn make_panel(mtm: MainThreadMarker, view: &GuideView) -> Retained<GuidePanel> {
    let frame = rect(0.0, 0.0, 420.0, 96.0);
    // SAFETY: the designated NSPanel initializer.
    let panel: Retained<GuidePanel> = unsafe {
        msg_send![GuidePanel::alloc(mtm), initWithContentRect: frame,
            styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            backing: NSBackingStoreType::Buffered, defer: false]
    };
    // SAFETY: the guide keeps its own reference and closes the panel itself.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setLevel(NSFloatingWindowLevel);
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(true);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    panel.setHidesOnDeactivate(false);
    panel.setAnimationBehavior(NSWindowAnimationBehavior::UtilityWindow);
    let sizable = NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable;
    let content = NSView::initWithFrame(NSView::alloc(mtm), frame);
    let material = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), frame);
    material.setMaterial(NSVisualEffectMaterial::Popover);
    material.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    material.setState(NSVisualEffectState::Active);
    material.setAutoresizingMask(sizable);
    material.setWantsLayer(true);
    // SAFETY: plain CALayer setters on the view's own backing layer.
    unsafe {
        let layer: Option<Retained<AnyObject>> = msg_send![&*material, layer];
        if let Some(layer) = layer {
            let _: () = msg_send![&*layer, setCornerRadius: PANEL_RADIUS];
            let _: () = msg_send![&*layer, setCornerCurve: ns_string!("continuous")];
            let _: () = msg_send![&*layer, setMasksToBounds: true];
        }
    }
    view.setFrame(frame);
    view.setAutoresizingMask(sizable);
    content.addSubview(&material);
    content.addSubview(view);
    panel.setContentView(Some(&content));
    panel
}

fn request_close() {
    GUIDE.with_borrow_mut(|guide| {
        if let Some(guide) = guide {
            guide.panel.orderOut(None);
            guide.closing = true;
        }
    });
}

/// Closes the guide. It never runs inside an event of the panel itself: a close click only
/// marks the guide, and the next tick ends it.
fn teardown() {
    let Some(guide) = GUIDE.with_borrow_mut(Option::take) else { return };
    for timer in &guide.timers {
        timer.invalidate();
    }
    guide.panel.orderOut(None);
    guide.panel.close();
}

fn tick() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let done = GUIDE.with_borrow_mut(|guide| {
        let Some(guide) = guide else { return false };
        guide.ticks += 1;
        if guide.closing || guide.started.elapsed() >= LIFETIME {
            return true;
        }
        let moving = guide.last.is_none() || guide.moved_at.elapsed() < MOVING_FOR;
        let second = guide.ticks % STILL_EVERY == 1;
        if moving || second {
            match settings_frame(mtm) {
                Some(frame) => {
                    guide.misses = 0;
                    if guide.last != Some(frame) {
                        let source = if guide.last.is_none() { guide.source.take() } else { None };
                        place(mtm, guide, frame, source);
                        guide.moved_at = Instant::now();
                    }
                    guide.last = Some(frame);
                }
                None => {
                    guide.misses += 1;
                    let limit = if guide.last.is_none() { MISSES_OPENING } else { MISSES_SEEN };
                    if guide.misses >= limit {
                        return true;
                    }
                }
            }
        }
        second && granted(&*guide.preflight, guide.pane)
    });
    if done {
        teardown();
    }
}

fn place(mtm: MainThreadMarker, guide: &Guide, settings: NSRect, fly_from: Option<NSPoint>) {
    let screen = NSScreen::screens(mtm)
        .iter()
        .find(|screen| overlaps(screen.frame(), settings))
        .map_or(settings, |screen| screen.visibleFrame());
    let width = (settings.size.width - SIDEBAR_WIDTH)
        .max(MIN_WIDTH)
        .min(screen.size.width - SCREEN_INSET * 2.0);
    let height = guide.view.layout(width).height;
    let x = (screen.origin.x + SCREEN_INSET)
        .max((settings.origin.x + SIDEBAR_WIDTH).min(screen.origin.x + screen.size.width - width - SCREEN_INSET));
    let y = (screen.origin.y + SCREEN_INSET)
        .max((settings.origin.y - height).min(screen.origin.y + screen.size.height - height - SCREEN_INSET));
    let target = rect(x, y, width, height);
    let panel = &guide.panel;
    let reduce = NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion();
    match fly_from {
        Some(from) if !panel.isVisible() && !reduce => {
            panel.setFrame_display(
                rect(from.x - width / 2.0, from.y - height / 2.0, width, height),
                true,
            );
            panel.setAlphaValue(0.9);
            panel.orderFrontRegardless();
            NSAnimationContext::beginGrouping();
            let context = NSAnimationContext::currentContext();
            context.setDuration(FLY_SECONDS);
            // SAFETY: a named CAMediaTimingFunction from QuartzCore, which AppKit links.
            unsafe {
                let ease: Retained<AnyObject> =
                    msg_send![class!(CAMediaTimingFunction), functionWithName: ns_string!("easeOut")];
                let _: () = msg_send![&*context, setTimingFunction: &*ease];
            }
            panel.animator().setFrame_display(target, true);
            panel.animator().setAlphaValue(1.0);
            NSAnimationContext::endGrouping();
        }
        _ => {
            panel.setFrame_display(target, true);
            // A drag in progress keeps the panel behind the list it drops onto.
            if !panel.ignoresMouseEvents() {
                panel.orderFrontRegardless();
            }
        }
    }
}

/// System Settings' largest on-screen window, in AppKit coordinates, from window-server bounds.
fn settings_frame(mtm: MainThreadMarker) -> Option<NSRect> {
    let pid = NSRunningApplication::runningApplicationsWithBundleIdentifier(ns_string!(
        "com.apple.systempreferences"
    ))
    .iter()
    .find(|app| app.activationPolicy() != NSApplicationActivationPolicy::Prohibited)?
    .processIdentifier();
    let list = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    )?;
    // SAFETY: a CFArray of CFDictionary is toll-free bridged to NSArray of NSDictionary.
    let windows = unsafe { &*(&*list as *const _ as *const NSArray<NSDictionary<NSString, AnyObject>>) };
    let top = NSScreen::screens(mtm).firstObject()?.frame();
    let top = top.origin.y + top.size.height;
    let number = |dict: &NSDictionary<NSString, AnyObject>, key: &NSString| {
        dict.objectForKey(key)
            .and_then(|value| value.downcast::<NSNumber>().ok())
            .map(|value| value.doubleValue())
    };
    windows
        .iter()
        .filter(|window| {
            number(window, ns_string!("kCGWindowOwnerPID")) == Some(f64::from(pid))
                && number(window, ns_string!("kCGWindowLayer")).unwrap_or(0.0) == 0.0
                && number(window, ns_string!("kCGWindowAlpha")).unwrap_or(1.0) > 0.0
        })
        .filter_map(|window| {
            let bounds = window
                .objectForKey(ns_string!("kCGWindowBounds"))?
                .downcast::<NSDictionary>()
                .ok()?;
            // SAFETY: the bounds dictionary maps NSString keys to NSNumber values.
            let bounds = unsafe { Retained::cast_unchecked::<NSDictionary<NSString, AnyObject>>(bounds) };
            Some(rect(
                number(&bounds, ns_string!("X"))?,
                number(&bounds, ns_string!("Y"))?,
                number(&bounds, ns_string!("Width"))?,
                number(&bounds, ns_string!("Height"))?,
            ))
        })
        .filter(|frame| frame.size.width > 320.0 && frame.size.height > 240.0)
        .max_by(|a, b| (a.size.width * a.size.height).total_cmp(&(b.size.width * b.size.height)))
        // Window-server rects start at the top left of the main screen; the 3 pt matches
        // PermissionFlow.
        .map(|frame| rect(frame.origin.x, top - frame.origin.y - frame.size.height - 3.0, frame.size.width, frame.size.height))
}

pub(super) fn snapshot_png(pane: Permission, dark: bool, width: f64, app: &Path) -> Option<Vec<u8>> {
    let mtm = MainThreadMarker::new()?;
    let gray = |white: f64, alpha: f64| NSColor::colorWithWhite_alpha(white, alpha);
    let stand_in = if dark {
        StandIn {
            page: gray(0.118, 1.0),
            blend: gray(0.08, 1.0),
            blend_op: NSCompositingOperation::Lighten,
            tint: gray(0.157, 0.40),
        }
    } else {
        StandIn {
            page: gray(1.0, 1.0),
            blend: gray(0.975, 1.0),
            blend_op: NSCompositingOperation::Darken,
            tint: gray(0.965, 0.36),
        }
    };
    let view = GuideView::new(mtm, pane, app, Some(stand_in));
    // SAFETY: the appearance name constants are immutable statics.
    let name = unsafe { if dark { NSAppearanceNameDarkAqua } else { NSAppearanceNameAqua } };
    view.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
    let height = view.layout(width).height;
    let bounds = rect(0.0, 0.0, width, height);
    view.setFrame(bounds);
    // SAFETY: AppKit allocates the pixel buffer (null planes); the sizes are positive.
    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            (width * 2.0) as isize,
            (height * 2.0) as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    bitmap.setSize(bounds.size);
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
    // SAFETY: PNG with no extra properties.
    let png = unsafe { bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }?;
    Some(png.to_vec())
}
