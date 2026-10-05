//! Element ids for the test hook. Every interactive element gets one, named
//! `<view>.<element>` or `<view>.<element>.<index>` (`sidebar.home`,
//! `history.row.3`). Pair the id with `.test_support()` on the element, which
//! is a no-op outside test builds.

use gpui_kit::SharedString;

pub fn id(view: &str, element: &str) -> SharedString {
    format!("{view}.{element}").into()
}

pub fn indexed(view: &str, element: &str, index: usize) -> SharedString {
    format!("{view}.{element}.{index}").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_join_view_element_and_index_with_dots() {
        assert_eq!(id("sidebar", "home"), "sidebar.home");
        assert_eq!(indexed("history", "row", 3), "history.row.3");
    }
}
