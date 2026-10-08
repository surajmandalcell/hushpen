//! What macOS says about whether a paste may be sent: key posting access, secure event input,
//! and a focused password field. Every call only reads.

// Plain C calls on memory this module owns.
#![allow(unsafe_code)]

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use hushpen_core::insert::guard::Probe;
use objc2_core_graphics::CGPreflightPostEventAccess;

const FOCUSED_ELEMENT: &str = "AXFocusedUIElement";
const SUBROLE: &str = "AXSubrole";
const SECURE_TEXT_FIELD: &str = "AXSecureTextField";
/// A hung app must not hold the paste: the default wait is 6 s.
const AX_TIMEOUT_SECONDS: f32 = 0.25;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn IsSecureEventInputEnabled() -> u8;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, seconds: f32) -> i32;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
}

pub(super) struct MacProbe;

impl Probe for MacProbe {
    fn post_event_allowed(&self) -> bool {
        CGPreflightPostEventAccess()
    }

    fn secure_event_input(&self) -> bool {
        // SAFETY: a C call with no arguments that only reads the session state.
        unsafe { IsSecureEventInputEnabled() != 0 }
    }

    fn focused_secure_field(&self) -> bool {
        focused_subrole().is_some_and(|subrole| subrole == SECURE_TEXT_FIELD)
    }
}

/// The value of one accessibility attribute, or `None` when it has none, or the process may not
/// use the accessibility API (the call then fails with `kAXErrorAPIDisabled`).
fn attribute(element: &CFType, name: &str) -> Option<CFType> {
    let name = CFString::new(name);
    let mut value: CFTypeRef = std::ptr::null();
    // SAFETY: `element` and `name` are live CF objects; `value` is a valid out pointer.
    let status = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            &mut value,
        )
    };
    if status != 0 || value.is_null() {
        return None;
    }
    // SAFETY: "Copy" returns a +1 reference that this wrapper now owns.
    Some(unsafe { CFType::wrap_under_create_rule(value) })
}

fn focused_subrole() -> Option<String> {
    // SAFETY: "Create" returns a +1 reference that this wrapper now owns.
    let system = unsafe {
        let raw = AXUIElementCreateSystemWide();
        if raw.is_null() {
            return None;
        }
        CFType::wrap_under_create_rule(raw)
    };
    // SAFETY: `system` is a live accessibility element.
    unsafe { AXUIElementSetMessagingTimeout(system.as_CFTypeRef(), AX_TIMEOUT_SECONDS) };
    let focused = attribute(&system, FOCUSED_ELEMENT)?;
    let subrole = attribute(&focused, SUBROLE)?;
    subrole
        .downcast_into::<CFString>()
        .map(|text| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The calls only read the system, so they run on any Mac without a prompt or a window.
    #[test]
    fn the_real_checks_answer_without_asking_anything() {
        let probe = MacProbe;
        let _ = probe.post_event_allowed();
        let _ = probe.secure_event_input();
        let _ = probe.focused_secure_field();
    }
}
