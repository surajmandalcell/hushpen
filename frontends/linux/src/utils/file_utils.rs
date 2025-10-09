//! File utility functions for the ASRPro application
//!
//! This module provides helper functions for file operations, validation,
//! and metadata extraction used throughout the application.

use std::path::{Path, PathBuf};
use std::fs;
use std::time::Duration;
use mime_guess::from_path;

use crate::models::{AudioFileType, AudioMetadata};
use crate::utils::{AppError, AppResult};

/// Supported audio file extensions
pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "flac", "aac", "ogg", "webm", "m4a"
];

/// Maximum file size in bytes (default: 100MB)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Minimum file size in bytes (default: 1KB)
pub const MIN_FILE_SIZE: u64 = 1024;

/// Detect the audio file type from a file path
pub fn detect_audio_file_type(file_path: &Path) -> AudioFileType {
    file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AudioFileType::from_extension(ext))
        .unwrap_or(AudioFileType::Unknown)
}

/// Check if a file is a supported audio format
pub fn is_supported_audio_file(file_path: &Path) -> bool {
    detect_audio_file_type(file_path).is_supported()
}

/// Validate a file for audio processing
pub fn validate_audio_file(file_path: &Path, max_size: Option<u64>) -> AppResult<()> {
    // Check if file exists
    if !file_path.exists() {
        return Err(AppError::file(format!(
            "File does not exist: {}",
            file_path.display()
        )));
    }

    // Check if it's a file (not a directory)
    if !file_path.is_file() {
        return Err(AppError::file(format!(
            "Path is not a file: {}",
            file_path.display()
        )));
    }

    // Check file extension
    if !is_supported_audio_file(file_path) {
        return Err(AppError::file(format!(
            "Unsupported file format: {}",
            file_path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("unknown")
        )));
    }

    // Check file size
    let metadata = fs::metadata(file_path)
        .map_err(|e| AppError::file_with_source(
            format!("Failed to read file metadata: {}", file_path.display()),
            e
        ))?;

    let file_size = metadata.len();
    let max_size = max_size.unwrap_or(DEFAULT_MAX_FILE_SIZE);

    if file_size < MIN_FILE_SIZE {
        return Err(AppError::file(format!(
            "File is too small ({} bytes, minimum is {} bytes)",
            file_size, MIN_FILE_SIZE
        )));
    }

    if file_size > max_size {
        return Err(AppError::file(format!(
            "File is too large ({} bytes, maximum is {} bytes)",
            file_size, max_size
        )));
    }

    Ok(())
}

/// Get the MIME type for a file
pub fn get_mime_type(file_path: &Path) -> String {
    from_path(file_path)
        .first_or_octet_stream()
        .to_string()
}

/// Extract basic metadata from an audio file
pub fn extract_audio_metadata(file_path: &Path) -> AppResult<AudioMetadata> {
    // Validate the file first
    validate_audio_file(file_path, None)?;

    // Get file metadata
    let file_metadata = fs::metadata(file_path)
        .map_err(|e| AppError::file_with_source(
            format!("Failed to read file metadata: {}", file_path.display()),
            e
        ))?;

    let file_size = file_metadata.len();
    let file_type = detect_audio_file_type(file_path);

    // In a real implementation, you would use a library like symphonia or rodio
    // to extract actual audio metadata. For now, we'll provide reasonable defaults.
    let metadata = AudioMetadata {
        duration: Duration::from_secs(60), // Default 1 minute
        sample_rate: 16000, // Default sample rate
        channels: 1, // Default mono
        bit_rate: Some(128000), // Default 128kbps
        format: file_type,
        file_size,
        title: None,
        artist: None,
        album: None,
        date: None,
        genre: None,
    };

    Ok(metadata)
}

/// Get a human-readable file size
pub fn format_file_size(size_bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = size_bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size_bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Get a human-readable duration
pub fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

/// Create a unique filename in a directory
pub fn create_unique_filename(directory: &Path, filename: &str) -> PathBuf {
    let path = directory.join(filename);
    
    if !path.exists() {
        return path;
    }

    // Extract filename and extension
    let (stem, extension) = if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            (stem.to_string(), Some(ext.to_string()))
        } else {
            (stem.to_string(), None)
        }
    } else {
        ("file".to_string(), None)
    };

    // Try appending numbers until we find an unused filename
    for i in 1..1000 {
        let new_filename = if let Some(ref ext) = extension {
            format!("{}_{}.{}", stem, i, ext)
        } else {
            format!("{}_{}", stem, i)
        };

        let new_path = directory.join(&new_filename);
        if !new_path.exists() {
            return new_path;
        }
    }

    // If we get here, we couldn't find a unique filename
    // Just return the original path and let the caller handle the conflict
    path
}

/// Check if a directory exists and is writable
pub fn ensure_directory_exists(directory: &Path) -> AppResult<()> {
    if directory.exists() {
        if !directory.is_dir() {
            return Err(AppError::file(format!(
                "Path exists but is not a directory: {}",
                directory.display()
            )));
        }

        // Check if directory is writable
        let test_file = directory.join(".asrpro_write_test");
        match fs::write(&test_file, "test") {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
                Ok(())
            }
            Err(e) => Err(AppError::file_with_source(
                format!("Directory is not writable: {}", directory.display()),
                e
            ))
        }
    } else {
        // Try to create the directory
        fs::create_dir_all(directory)
            .map_err(|e| AppError::file_with_source(
                format!("Failed to create directory: {}", directory.display()),
                e
            ))
    }
}

/// Get the total size of all files in a directory
pub fn get_directory_size(directory: &Path) -> AppResult<u64> {
    if !directory.exists() || !directory.is_dir() {
        return Err(AppError::file(format!(
            "Directory does not exist or is not a directory: {}",
            directory.display()
        )));
    }

    let mut total_size = 0u64;

    let entries = fs::read_dir(directory)
        .map_err(|e| AppError::file_with_source(
            format!("Failed to read directory: {}", directory.display()),
            e
        ))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| AppError::file_with_source(
                format!("Failed to read directory entry"),
                e
            ))?;

        let path = entry.path();
        if path.is_file() {
            if let Ok(metadata) = fs::metadata(&path) {
                total_size += metadata.len();
            }
        } else if path.is_dir() {
            // Recursively add subdirectory sizes
            if let Ok(size) = get_directory_size(&path) {
                total_size += size;
            }
        }
    }

    Ok(total_size)
}

/// Clean up temporary files in a directory
pub fn cleanup_temp_files(directory: &Path, max_age: Duration) -> AppResult<usize> {
    if !directory.exists() || !directory.is_dir() {
        return Ok(0);
    }

    let now = std::time::SystemTime::now();
    let mut cleaned_count = 0;

    let entries = fs::read_dir(directory)
        .map_err(|e| AppError::file_with_source(
            format!("Failed to read directory: {}", directory.display()),
            e
        ))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| AppError::file_with_source(
                format!("Failed to read directory entry"),
                e
            ))?;

        let path = entry.path();
        if path.is_file() {
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            if fs::remove_file(&path).is_ok() {
                                cleaned_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(cleaned_count)
}

/// Check if a file is likely to be an audio file based on its content
pub fn is_audio_file_by_content(file_path: &Path) -> AppResult<bool> {
    // This is a simplified check - in a real implementation, you would
    // examine the file header/magic bytes to determine the file type
    
    // For now, just check the extension
    Ok(is_supported_audio_file(file_path))
}

/// Get the audio quality category based on bitrate
pub fn get_audio_quality_category(bitrate: Option<u32>) -> &'static str {
    match bitrate {
        None => "Unknown",
        Some(b) if b < 64000 => "Low",
        Some(b) if b < 128000 => "Medium",
        Some(b) if b < 256000 => "High",
        Some(_) => "Very High",
    }
}

/// Calculate the estimated transcription time based on audio duration
pub fn estimate_transcription_time(duration: Duration) -> Duration {
    // This is a rough estimate - actual transcription time depends on
    // model complexity, hardware, and other factors
    let multiplier = 0.1; // Assume transcription is 10x faster than real-time
    Duration::from_millis((duration.as_millis() as f64 * multiplier) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_detect_audio_file_type() {
        assert_eq!(
            detect_audio_file_type(Path::new("test.mp3")),
            AudioFileType::Mp3
        );
        assert_eq!(
            detect_audio_file_type(Path::new("test.wav")),
            AudioFileType::Wav
        );
        assert_eq!(
            detect_audio_file_type(Path::new("test.unknown")),
            AudioFileType::Unknown
        );
    }

    #[test]
    fn test_is_supported_audio_file() {
        assert!(is_supported_audio_file(Path::new("test.mp3")));
        assert!(is_supported_audio_file(Path::new("test.wav")));
        assert!(!is_supported_audio_file(Path::new("test.txt")));
        assert!(!is_supported_audio_file(Path::new("test.unknown")));
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1536), "1.50 KB");
        assert_eq!(format_file_size(2 * 1024 * 1024), "2.00 MB");
        assert_eq!(format_file_size(2 * 1024 * 1024 * 1024), "2.00 GB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "00:30");
        assert_eq!(format_duration(Duration::from_secs(90)), "01:30");
        assert_eq!(format_duration(Duration::from_secs(3661)), "01:01:01");
    }

    #[test]
    fn test_get_audio_quality_category() {
        assert_eq!(get_audio_quality_category(None), "Unknown");
        assert_eq!(get_audio_quality_category(Some(32000)), "Low");
        assert_eq!(get_audio_quality_category(Some(96000)), "Medium");
        assert_eq!(get_audio_quality_category(Some(192000)), "High");
        assert_eq!(get_audio_quality_category(Some(320000)), "Very High");
    }

    #[test]
    fn test_ensure_directory_exists() {
        let temp_dir = tempdir().unwrap();
        let new_dir = temp_dir.path().join("test_dir");
        
        // Directory doesn't exist yet
        assert!(!new_dir.exists());
        
        // Create it
        assert!(ensure_directory_exists(&new_dir).is_ok());
        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
        
        // Try again (should succeed)
        assert!(ensure_directory_exists(&new_dir).is_ok());
    }

    #[test]
    fn test_create_unique_filename() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create a file
        fs::write(&file_path, "test").unwrap();
        
        // Create a unique filename
        let unique_path = create_unique_filename(temp_dir.path(), "test.txt");
        assert_ne!(file_path, unique_path);
        assert_eq!(unique_path.file_name().unwrap().to_str().unwrap(), "test_1.txt");
    }

    #[test]
    fn test_estimate_transcription_time() {
        let duration = Duration::from_secs(60);
        let estimated = estimate_transcription_time(duration);
        assert_eq!(estimated, Duration::from_secs(6)); // 10% of original
    }
}