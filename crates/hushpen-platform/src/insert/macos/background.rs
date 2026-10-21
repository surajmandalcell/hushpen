//! Test builds only: puts text into, reads text from, and presses menu items of an app in the
//! background through the accessibility API. An inactive app such as TextEdit ignores a key event
//! posted to its process and shows its Edit menu items as disabled, but its focused text area
//! takes a new selected text.

// Plain C calls on memory this module owns.
#![allow(unsafe_code)]

use super::guard::{attribute, with_timeout};
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;

const MENU_BAR: &str = "AXMenuBar";
const CHILDREN: &str = "AXChildren";
const ENABLED: &str = "AXEnabled";
const CMD_CHAR: &str = "AXMenuItemCmdChar";
const CMD_MODIFIERS: &str = "AXMenuItemCmdModifiers";
const PRESS: &str = "AXPress";
const FOCUSED_ELEMENT: &str = "AXFocusedUIElement";
const SELECTED_TEXT: &str = "AXSelectedText";
/// An app that does not answer must not hold the paste for long.
const AX_TIMEOUT_SECONDS: f32 = 1.0;
/// The menu bar, its items, their menus, and the items of those menus.
const MAX_DEPTH: usize = 3;
/// No real menu bar has this many items in its top-level menus.
const MAX_ITEMS: usize = 2000;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFStringRef) -> i32;
    fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
}

/// What a menu item says about its key equivalent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Keys {
    pub cmd_char: Option<String>,
    /// 0 means Command alone.
    pub modifiers: Option<i64>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Miss {
    NoApp,
    NoMenuBar,
    NoItem,
    Disabled,
    PressFailed(i32),
    NoFocusedElement,
    SetFailed(i32),
}

impl std::fmt::Display for Miss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Miss::NoApp => f.write_str("the process has no accessibility element"),
            Miss::NoMenuBar => f.write_str("the app shows no menu bar"),
            Miss::NoItem => f.write_str("no menu item has this key"),
            Miss::Disabled => f.write_str("the menu item with this key is disabled"),
            Miss::PressFailed(code) => write!(f, "the press failed with AX error {code}"),
            Miss::NoFocusedElement => f.write_str("the app has no focused element"),
            Miss::SetFailed(code) => {
                write!(f, "the selected text was refused with AX error {code}")
            }
        }
    }
}

fn is_command(keys: &Keys, letter: char) -> bool {
    keys.modifiers == Some(0)
        && keys.cmd_char.as_deref().is_some_and(|text| {
            let mut chars = text.chars();
            chars
                .next()
                .is_some_and(|first| first.eq_ignore_ascii_case(&letter))
                && chars.next().is_none()
        })
}

/// The first enabled item whose key is Command and `letter`.
pub(super) fn pick<T>(items: impl IntoIterator<Item = (T, Keys)>, letter: char) -> Result<T, Miss> {
    let mut disabled = false;
    for (item, keys) in items {
        if !is_command(&keys, letter) {
            continue;
        }
        if keys.enabled {
            return Ok(item);
        }
        disabled = true;
    }
    Err(if disabled {
        Miss::Disabled
    } else {
        Miss::NoItem
    })
}

fn children(element: &CFType) -> Vec<CFType> {
    let Some(array) = attribute(element, CHILDREN)
        .and_then(|value| value.downcast_into::<CFArray<*const c_void>>())
    else {
        return Vec::new();
    };
    array
        .get_all_values()
        .into_iter()
        .filter(|raw| !raw.is_null())
        // SAFETY: the array holds live CF objects; "get" rule retains each one.
        .map(|raw| unsafe { CFType::wrap_under_get_rule(raw) })
        .collect()
}

fn keys(item: &CFType) -> Keys {
    Keys {
        cmd_char: attribute(item, CMD_CHAR)
            .and_then(|value| value.downcast_into::<CFString>())
            .map(|text| text.to_string()),
        modifiers: attribute(item, CMD_MODIFIERS)
            .and_then(|value| value.downcast_into::<CFNumber>())
            .and_then(|number| number.to_i64()),
        enabled: attribute(item, ENABLED)
            .and_then(|value| value.downcast_into::<CFBoolean>())
            .is_some_and(bool::from),
    }
}

/// The items of the menus of the menu bar, at most `MAX_DEPTH` levels down.
fn menu_items(bar: &CFType) -> Vec<CFType> {
    let mut level = vec![bar.clone()];
    for _ in 0..MAX_DEPTH {
        let mut next = Vec::new();
        for element in &level {
            next.extend(children(element));
            if next.len() >= MAX_ITEMS {
                break;
            }
        }
        next.truncate(MAX_ITEMS);
        level = next;
    }
    level
}

/// Presses the menu item of `pid` whose key is Command and `letter`, the way a click does.
pub(super) fn press_command(pid: i32, letter: char) -> Result<(), Miss> {
    let app = app(pid)?;
    let bar = attribute(&app, MENU_BAR).ok_or(Miss::NoMenuBar)?;
    let items = menu_items(&bar).into_iter().map(|item| {
        let keys = keys(&item);
        (item, keys)
    });
    let item = pick(items, letter)?;
    let action = CFString::new(PRESS);
    // SAFETY: `item` and `action` are live CF objects.
    let status =
        unsafe { AXUIElementPerformAction(item.as_CFTypeRef(), action.as_concrete_TypeRef()) };
    if status == 0 {
        Ok(())
    } else {
        Err(Miss::PressFailed(status))
    }
}

/// The accessibility element of the app `pid`, with the short timeout.
fn app(pid: i32) -> Result<CFType, Miss> {
    // SAFETY: "Create" returns a +1 reference that this wrapper now owns.
    let app = unsafe {
        let raw = AXUIElementCreateApplication(pid);
        if raw.is_null() {
            return Err(Miss::NoApp);
        }
        CFType::wrap_under_create_rule(raw)
    };
    with_timeout(&app, AX_TIMEOUT_SECONDS);
    Ok(app)
}

/// The focused element of the app `pid`, which the app keeps while it is in the background.
fn focused(pid: i32) -> Result<CFType, Miss> {
    let focused = attribute(&app(pid)?, FOCUSED_ELEMENT).ok_or(Miss::NoFocusedElement)?;
    with_timeout(&focused, AX_TIMEOUT_SECONDS);
    Ok(focused)
}

/// Replaces the selection of the focused element of `pid` with `text`, as a paste does.
pub(super) fn set_selected_text(pid: i32, text: &str) -> Result<(), Miss> {
    let focused = focused(pid)?;
    let name = CFString::new(SELECTED_TEXT);
    let value = CFString::new(text);
    // SAFETY: `focused`, `name`, and `value` are live CF objects.
    let status = unsafe {
        AXUIElementSetAttributeValue(
            focused.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            value.as_CFTypeRef(),
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(Miss::SetFailed(status))
    }
}

/// The selected text of the focused element of `pid`, when it has any.
pub(super) fn selected_text(pid: i32) -> Result<Option<String>, Miss> {
    let text = attribute(&focused(pid)?, SELECTED_TEXT)
        .and_then(|value| value.downcast_into::<CFString>())
        .map(|text| text.to_string());
    Ok(text.filter(|text| !text.trim().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(cmd: &str, modifiers: i64, enabled: bool) -> Keys {
        Keys {
            cmd_char: Some(cmd.into()),
            modifiers: Some(modifiers),
            enabled,
        }
    }

    #[test]
    fn the_enabled_command_item_with_the_letter_is_picked_in_any_case() {
        let items = [
            ("Paste and Match Style", item("V", 0b1010, true)),
            ("Copy", item("C", 0, true)),
            ("Paste", item("v", 0, true)),
        ];
        assert_eq!(pick(items.clone(), 'V'), Ok("Paste"));
        assert_eq!(pick(items, 'c'), Ok("Copy"));
    }

    #[test]
    fn an_item_with_more_modifiers_or_no_modifier_value_is_not_the_chord() {
        let items = [
            ("Redo", item("Z", 1, true)),
            (
                "No modifiers",
                Keys {
                    cmd_char: Some("Z".into()),
                    modifiers: None,
                    enabled: true,
                },
            ),
            ("Zoom", item("ZZ", 0, true)),
        ];
        assert_eq!(pick(items, 'Z'), Err(Miss::NoItem));
    }

    #[test]
    fn a_disabled_match_is_skipped_and_reported_when_it_is_the_only_one() {
        assert_eq!(
            pick([("Undo", item("Z", 0, false))], 'Z'),
            Err(Miss::Disabled)
        );
        let items = [("Off", item("Z", 0, false)), ("Undo", item("Z", 0, true))];
        assert_eq!(pick(items, 'Z'), Ok("Undo"));
        assert_eq!(pick(Vec::<(&str, Keys)>::new(), 'Z'), Err(Miss::NoItem));
    }

    /// Asking a process that does not exist answers at once and changes nothing.
    #[test]
    fn a_missing_process_has_no_menu_bar_and_no_focused_element() {
        assert_eq!(press_command(i32::MAX, 'V'), Err(Miss::NoMenuBar));
        assert_eq!(
            set_selected_text(i32::MAX, "x"),
            Err(Miss::NoFocusedElement)
        );
        assert_eq!(selected_text(i32::MAX), Err(Miss::NoFocusedElement));
    }
}
