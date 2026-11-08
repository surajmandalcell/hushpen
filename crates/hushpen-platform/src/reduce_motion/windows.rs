#![allow(unsafe_code)]

use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::BOOL;

pub(super) fn system_reduce_motion() -> Option<bool> {
    let mut animations = BOOL(1);
    // SAFETY: this query writes a BOOL to the live output pointer.
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut animations as *mut BOOL).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok()?;
    Some(!animations.as_bool())
}
