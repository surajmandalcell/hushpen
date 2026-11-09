//! Wrap the existing PNG sizes in an ICO without changing their pixels.

use std::fs;
use std::path::Path;

pub fn write(assets: &Path, output: &Path) -> Result<(), String> {
    let mut images = Vec::new();
    for size in [16_u16, 24, 32, 48, 64, 128, 256] {
        let path = assets.join(format!("linux-icons/{size}x{size}.png"));
        let bytes = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        images.push((size, bytes));
    }
    let mut ico = vec![0, 0, 1, 0, 7, 0];
    let mut offset = 6_u32 + 16 * 7;
    for (size, bytes) in &images {
        let size = if *size == 256 { 0 } else { *size as u8 };
        ico.extend_from_slice(&[size, size, 0, 0, 1, 0, 32, 0]);
        let len = u32::try_from(bytes.len()).map_err(|_| "icon PNG is too large")?;
        ico.extend_from_slice(&len.to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset = offset.checked_add(len).ok_or("icon is too large")?;
    }
    for (_, bytes) in images {
        ico.extend_from_slice(&bytes);
    }
    fs::write(output, ico).map_err(|e| format!("cannot write {}: {e}", output.display()))
}
