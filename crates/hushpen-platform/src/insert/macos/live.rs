//! A live edit addresses one accessibility element. No key is sent to the front app.

use super::guard::{self, MacProbe, attribute};
use crate::insert::{LiveError, LiveSpan, Target};
use core_foundation::array::CFArray;
use core_foundation::base::{CFRange, CFType, CFTypeID, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use hushpen_core::insert::guard::check;
use std::ffi::c_void;
use std::ops::Range;

const SELECTED_RANGE: &str = "AXSelectedTextRange";
const SELECTED_TEXT: &str = "AXSelectedText";
const RANGE_TYPE: u32 = 4; // kAXValueTypeCFRange
const CANNOT_COMPLETE: i32 = -25204; // An AX timeout can follow a delivered write.
const ATTRIBUTE_UNSUPPORTED: i32 = -25205;
const NO_VALUE: i32 = -25212;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementGetPid(element: CFTypeRef, pid: *mut i32) -> i32;
    fn AXValueGetTypeID() -> CFTypeID;
    fn AXValueCreate(kind: u32, value: *const c_void) -> CFTypeRef;
    fn AXValueGetValue(value: CFTypeRef, kind: u32, output: *mut c_void) -> u8;
    fn AXUIElementIsAttributeSettable(element: CFTypeRef, name: CFStringRef, out: *mut u8) -> i32;
    fn AXUIElementSetAttributeValue(element: CFTypeRef, name: CFStringRef, value: CFTypeRef)
    -> i32;
}

fn text(field: &CFType, name: &str) -> Option<String> {
    Some(
        attribute(field, name)?
            .downcast_into::<CFString>()?
            .to_string(),
    )
}

fn selection(field: &CFType) -> Option<CFRange> {
    // The primary range alone cannot prove where a multi-cursor editor will write.
    match guard::read_attribute(field, "AXSelectedTextRanges") {
        Ok(ranges) if ranges.downcast::<CFArray>()?.len() <= 1 => {}
        Err(ATTRIBUTE_UNSUPPORTED | NO_VALUE) => {}
        _ => return None,
    }
    let value = attribute(field, SELECTED_RANGE)?;
    let mut range = CFRange::init(0, 0);
    // SAFETY: check the CF type before unpacking a CFRange into its matching output buffer.
    unsafe {
        if value.type_of() != AXValueGetTypeID()
            || AXValueGetValue(value.as_CFTypeRef(), RANGE_TYPE, (&raw mut range).cast()) == 0
        {
            return None;
        }
    }
    (range.location >= 0 && range.length >= 0).then_some(range)
}

fn set(field: &CFType, name: &str, value: CFTypeRef) -> i32 {
    let name = CFString::new(name);
    // SAFETY: callers pass live CF objects of the attribute's documented type.
    unsafe { AXUIElementSetAttributeValue(field.as_CFTypeRef(), name.as_concrete_TypeRef(), value) }
}

fn select(field: &CFType, range: CFRange) -> bool {
    // SAFETY: AXValueCreate copies the CFRange and returns a retained AXValue.
    let raw = unsafe { AXValueCreate(RANGE_TYPE, (&raw const range).cast()) };
    if raw.is_null() {
        return false;
    }
    let value = unsafe { CFType::wrap_under_create_rule(raw) };
    set(field, SELECTED_RANGE, value.as_CFTypeRef()) == 0
}

fn writable(field: &CFType, name: &str) -> bool {
    let name = CFString::new(name);
    let mut writable = 0;
    // SAFETY: both CF objects are live and writable is a valid Boolean output.
    let status = unsafe {
        AXUIElementIsAttributeSettable(
            field.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            &mut writable,
        )
    };
    status == 0 && writable != 0
}

fn focused(target: &Target) -> Option<CFType> {
    #[cfg(feature = "test-automation")]
    let field = match crate::insert::test_target::bundle() {
        Some(_) => super::background::focused(super::key_pid(target)?).ok()?,
        None => guard::focused_element()?,
    };
    #[cfg(not(feature = "test-automation"))]
    let field = guard::focused_element()?;
    guard::with_timeout(&field, guard::AX_TIMEOUT_SECONDS);
    let mut pid = 0;
    // SAFETY: field is the focused AXUIElement and pid is a valid output.
    let status = unsafe { AXUIElementGetPid(field.as_CFTypeRef(), &mut pid) };
    (status == 0 && u64::try_from(pid).ok() == target.window).then_some(field)
}

fn editable_focus(target: &Target) -> Option<CFType> {
    if check(&MacProbe).is_some() {
        return None;
    }
    let field = focused(target)?;
    match guard::read_attribute(&field, "AXSubrole") {
        Ok(value) if value.downcast::<CFString>()?.to_string() != "AXSecureTextField" => {
            Some(field)
        }
        Err(ATTRIBUTE_UNSUPPORTED | NO_VALUE) => Some(field),
        _ => None,
    }
}

/// AX uses UTF-16 offsets. A caret in the middle of a surrogate pair is not a Rust string boundary.
fn byte_at_utf16(text: &str, offset: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == offset {
            return Some(byte);
        }
        units += ch.len_utf16();
        if units > offset {
            return None;
        }
    }
    (units == offset).then_some(text.len())
}

struct Span {
    target: Target,
    field: CFType,
    expected: String,
    draft: Range<usize>,
    caret: CFRange,
}

// SAFETY: the only CF object retained across threads is an AXUIElement (a remote object reference).
// The span has one owner. All writes use &mut self.
unsafe impl Send for Span {}

pub(super) fn begin(target: &Target) -> Option<Box<dyn LiveSpan>> {
    let field = editable_focus(target)?;
    if !writable(&field, SELECTED_TEXT) || !writable(&field, SELECTED_RANGE) {
        return None;
    }
    let caret = selection(&field)?;
    if caret.length != 0 {
        return None;
    }
    let expected = text(&field, "AXValue")?;
    let start = byte_at_utf16(&expected, caret.location as usize)?;
    Some(Box::new(Span {
        target: target.clone(),
        field,
        expected,
        draft: start..start,
        caret,
    }))
}

impl Span {
    fn matches(&self, value: &str, range: CFRange) -> bool {
        editable_focus(&self.target).as_ref() == Some(&self.field)
            && text(&self.field, "AXValue").as_deref() == Some(value)
            && selection(&self.field) == Some(range)
    }
}

/// A busy target can miss one AX read (0.25 s timeout). Read once more before calling it an edit.
fn twice(check: impl Fn() -> bool) -> bool {
    if check() {
        return true;
    }
    std::thread::sleep(std::time::Duration::from_millis(30));
    let again = check();
    #[cfg(feature = "test-automation")]
    log::info!("TEST_INSERT live-recheck held={again}");
    again
}

impl LiveSpan for Span {
    fn unchanged(&self) -> bool {
        twice(|| self.matches(&self.expected, self.caret))
    }

    fn at_caret(&self) -> bool {
        twice(|| {
            focused(&self.target).as_ref() == Some(&self.field)
                && selection(&self.field) == Some(self.caret)
        })
    }

    fn replace(&mut self, keep_bytes: usize, replacement: &str) -> Result<(), LiveError> {
        let draft = &self.expected[self.draft.clone()];
        let removed = draft.get(keep_bytes..).ok_or(LiveError::Changed)?;
        if removed == replacement {
            return self.unchanged().then_some(()).ok_or(LiveError::Changed);
        }
        let units = removed.encode_utf16().count() as isize;
        let range = CFRange::init(self.caret.location - units, units);
        let changed_start = self.draft.start + keep_bytes;
        let mut expected = self.expected.clone();
        expected.replace_range(changed_start..self.draft.end, replacement);
        let caret = CFRange::init(
            range.location + replacement.encode_utf16().count() as isize,
            0,
        );
        let value = CFString::new(replacement);
        if !self.unchanged() {
            return Err(LiveError::Changed);
        }
        if units != 0 && (!select(&self.field, range) || !self.matches(&self.expected, range)) {
            // A selection change may have landed even when the target timed out.
            #[cfg(feature = "test-automation")]
            log::warn!("TEST_INSERT live-miss select units={units}");
            return Err(LiveError::Uncertain);
        }
        let status = set(&self.field, SELECTED_TEXT, value.as_CFTypeRef());
        if !self.matches(&expected, caret) {
            #[cfg(feature = "test-automation")]
            log::warn!(
                "TEST_INSERT live-miss status={status} focus={} value={:?} selection={:?} want={:?}",
                editable_focus(&self.target).as_ref() == Some(&self.field),
                text(&self.field, "AXValue").map(|value| value == expected),
                selection(&self.field).map(|range| (range.location, range.length)),
                (caret.location, caret.length),
            );
            // Some fields acknowledge a write but keep the original text. Restore only our selection.
            if status != CANNOT_COMPLETE
                && self.matches(&self.expected, range)
                && (units == 0 || select(&self.field, self.caret))
                && self.unchanged()
            {
                return Err(LiveError::Changed);
            }
            return Err(LiveError::Uncertain);
        }
        #[cfg(feature = "test-automation")]
        log::info!(
            "TEST_INSERT live-edit removed={} inserted={}",
            removed.chars().count(),
            replacement.chars().count()
        );
        self.expected = expected;
        self.draft.end = changed_start + replacement.len();
        self.caret = caret;
        Ok(())
    }

    fn splice(&mut self, start: usize, end: usize, replacement: &str) -> Result<(), LiveError> {
        let draft = &self.expected[self.draft.clone()];
        if end == draft.len() {
            return self.replace(start, replacement);
        }
        let removed = draft.get(start..end).ok_or(LiveError::Changed)?;
        let after = draft.get(end..).ok_or(LiveError::Changed)?;
        let units = removed.encode_utf16().count() as isize;
        let added = replacement.encode_utf16().count() as isize;
        let range = CFRange::init(
            self.caret.location - after.encode_utf16().count() as isize - units,
            units,
        );
        let caret = CFRange::init(self.caret.location - units + added, 0);
        let mut expected = self.expected.clone();
        expected.replace_range(self.draft.start + start..self.draft.start + end, replacement);
        let value = CFString::new(replacement);
        if !self.unchanged() {
            return Err(LiveError::Changed);
        }
        if !select(&self.field, range) || !self.matches(&self.expected, range) {
            #[cfg(feature = "test-automation")]
            log::warn!("TEST_INSERT live-miss splice-select units={units}");
            return Err(LiveError::Uncertain);
        }
        set(&self.field, SELECTED_TEXT, value.as_CFTypeRef());
        // The write leaves the caret after the fixed word. Put it back at the end of the draft.
        if !select(&self.field, caret) || !self.matches(&expected, caret) {
            #[cfg(feature = "test-automation")]
            log::warn!("TEST_INSERT live-miss splice-write units={units}");
            return Err(LiveError::Uncertain);
        }
        #[cfg(feature = "test-automation")]
        log::info!(
            "TEST_INSERT live-edit removed={} inserted={} splice",
            removed.chars().count(),
            replacement.chars().count()
        );
        self.draft.end = self.draft.end + replacement.len() - removed.len();
        self.caret = caret;
        self.expected = expected;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_offsets_include_only_complete_unicode_scalars() {
        let text = "A🙂é";
        assert_eq!(
            (0..=5).map(|n| byte_at_utf16(text, n)).collect::<Vec<_>>(),
            [Some(0), Some(1), None, Some(5), Some(7), None]
        );
    }
}
