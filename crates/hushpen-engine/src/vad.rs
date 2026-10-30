//! The small, bundled Silero model and conservative speech boundaries.

use std::path::{Path, PathBuf};

use whisper_rs::WhisperVadParams;

use crate::EngineError;

const MODEL: &str = "ggml-silero-v5.1.2.bin";

fn candidates(exe_dir: &Path) -> [PathBuf; 3] {
    [
        exe_dir.join(MODEL),
        exe_dir.join("../Resources").join(MODEL),
        exe_dir.join("../lib/hushpen").join(MODEL),
    ]
}

fn locate(exe_dir: &Path) -> Option<PathBuf> {
    candidates(exe_dir).into_iter().find(|path| path.is_file())
}

pub fn model_path() -> Result<String, EngineError> {
    let exe = std::env::current_exe().map_err(|e| EngineError::LoadFailed(e.to_string()))?;
    let mut path = exe.parent().and_then(locate);
    // Cargo runs and tests do not have a package resource folder.
    if cfg!(debug_assertions) && path.is_none() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(MODEL);
        path = source.is_file().then_some(source);
    }
    path.and_then(|path| path.into_os_string().into_string().ok())
        .ok_or_else(|| EngineError::LoadFailed("the bundled speech detector is missing".into()))
}

pub fn params() -> WhisperVadParams {
    let mut params = WhisperVadParams::default();
    params.set_threshold(0.5);
    params.set_min_speech_duration(100);
    params.set_min_silence_duration(600);
    params.set_speech_pad(250);
    params.set_samples_overlap(0.1);
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_package_layout_has_a_model_location() {
        for (binary_dir, resource_dir) in [
            ("tar", "tar"),
            (
                "Hushpen.app/Contents/MacOS",
                "Hushpen.app/Contents/Resources",
            ),
            ("usr/lib/hushpen", "usr/lib/hushpen"),
            ("usr/bin", "usr/lib/hushpen"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let exe_dir = root.path().join(binary_dir);
            let resources = root.path().join(resource_dir);
            std::fs::create_dir_all(&exe_dir).unwrap();
            std::fs::create_dir_all(&resources).unwrap();
            assert!(locate(&exe_dir).is_none());
            let model = resources.join(MODEL);
            std::fs::write(&model, b"model").unwrap();
            assert_eq!(
                std::fs::canonicalize(locate(&exe_dir).unwrap()).unwrap(),
                std::fs::canonicalize(model).unwrap()
            );
        }
    }
}
