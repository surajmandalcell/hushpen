#![allow(unsafe_code)]

use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

pub(super) fn total_ram_bytes() -> Option<u64> {
    let mut memory = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: memory has the required size and is writable for the call.
    unsafe { GlobalMemoryStatusEx(&mut memory) }.ok()?;
    Some(memory.ullTotalPhys)
}
