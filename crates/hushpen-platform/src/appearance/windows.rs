use super::Scheme;
use windows::UI::ViewManagement::{UIColorType, UISettings};

pub(super) fn desktop_scheme() -> Option<Scheme> {
    let color = UISettings::new()
        .ok()?
        .GetColorValue(UIColorType::Background)
        .ok()?;
    Some(
        if u32::from(color.R) + u32::from(color.G) + u32::from(color.B) < 384 {
            Scheme::Dark
        } else {
            Scheme::Light
        },
    )
}
