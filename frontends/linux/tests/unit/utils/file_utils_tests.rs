//! Unit tests for file utility functions

use std::path::Path;
use std::time::Duration;
use tempfile::{tempdir, NamedTempFile};
use std::io::Write;

use asrpro_gtk4::utils::file_utils::{
    detect_audio_file_type, is_supported_audio_file, validate_audio_file,
    get_mime_type, extract_audio_metadata, format_file_size, format_duration,
    create_unique_filename, ensure_directory_exists, get_directory_size,
    cleanup_temp_files, is_audio_file_by_content, get_audio_quality_category,
    estimate_transcription_time, SUPPORTED_AUDIO_EXTENSIONS,
    DEFAULT_MAX_FILE_SIZE, MIN_FILE_SIZE,
};
use asrpro_gtk4::models::{AudioFileType, AudioMetadata};

#[cfg(test)]
mod tests {
    use super::*;

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
            detect_audio_file_type(Path::new("test.flac")),
            AudioFileType::Flac
        );
        assert_eq!(
            detect_audio_file_type(Path::new("test.aac")),
            AudioFileType::Aac
        );
        assert_eq!(
            detect_audio_file_type(Path::new("test.ogg")),
            AudioFileType::Ogg
        );
        assert_eq!(
            detect_audio_file_type(Path::new("test.webm")),
            AudioFileType::Webm
        );
        assert_eq!(
            detect_audio_file_type(Path::new("test.m4a")),
            AudioFileType::M4a
        );
        assert_eq!(
            detect_audio_file_type(Path::new("test.unknown")),
            AudioFileType::Unknown
        );
        assert_eq!(
            detect_audio_file_type(Path::new("test")),
            AudioFileType::Unknown
        );
        assert_eq!(
            detect_audio_file_type(Path::new("")),
            AudioFileType::Unknown
        );
    }

    #[test]
    fn test_is_supported_audio_file() {
        assert!(is_supported_audio_file(Path::new("test.mp3")));
        assert!(is_supported_audio_file(Path::new("test.wav")));
        assert!(is_supported_audio_file(Path::new("test.flac")));
        assert!(is_supported_audio_file(Path::new("test.aac")));
        assert!(is_supported_audio_file(Path::new("test.ogg")));
        assert!(is_supported_audio_file(Path::new("test.webm")));
        assert!(is_supported_audio_file(Path::new("test.m4a")));
        assert!(!is_supported_audio_file(Path::new("test.txt")));
        assert!(!is_supported_audio_file(Path::new("test.unknown")));
        assert!(!is_supported_audio_file(Path::new("test")));
        assert!(!is_supported_audio_file(Path::new("")));
    }

    #[test]
    fn test_validate_audio_file() {
        let temp_dir = tempdir().unwrap();
        
        // Test with a valid file
        let file_path = temp_dir.path().join("test.mp3");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"dummy audio data").unwrap();
        
        assert!(validate_audio_file(&file_path, None).is_ok());
        
        // Test with a non-existent file
        let non_existent = temp_dir.path().join("non_existent.mp3");
        assert!(validate_audio_file(&non_existent, None).is_err());
        
        // Test with a directory
        assert!(validate_audio_file(temp_dir.path(), None).is_err());
        
        // Test with an unsupported file
        let unsupported_path = temp_dir.path().join("test.txt");
        std::fs::write(&unsupported_path, "text data").unwrap();
        assert!(validate_audio_file(&unsupported_path, None).is_err());
        
        // Test with a file that's too small
        let small_path = temp_dir.path().join("small.mp3");
        std::fs::write(&small_path, b"x").unwrap();
        assert!(validate_audio_file(&small_path, None).is_err());
        
        // Test with a custom max size
        let large_path = temp_dir.path().join("large.mp3");
        let large_data = vec![0u8; 2000]; // 2KB
        std::fs::write(&large_path, large_data).unwrap();
        assert!(validate_audio_file(&large_path, Some(1000)).is_err());
    }

    #[test]
    fn test_get_mime_type() {
        assert_eq!(get_mime_type(Path::new("test.mp3")), "audio/mpeg");
        assert_eq!(get_mime_type(Path::new("test.wav")), "audio/wav");
        assert_eq!(get_mime_type(Path::new("test.flac")), "audio/flac");
        assert_eq!(get_mime_type(Path::new("test.txt")), "text/plain");
        assert_eq!(get_mime_type(Path::new("test.unknown")), "application/octet-stream");
    }

    #[test]
    fn test_extract_audio_metadata() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.mp3");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"dummy audio data").unwrap();
        
        let metadata = extract_audio_metadata(&file_path).unwrap();
        assert_eq!(metadata.duration, Duration::from_secs(60));
        assert_eq!(metadata.sample_rate, 16000);
        assert_eq!(metadata.channels, 1);
        assert_eq!(metadata.bit_rate, Some(128000));
        assert_eq!(metadata.format, AudioFileType::Mp3);
        assert_eq!(metadata.file_size, 17); // Size of "dummy audio data"
        assert!(metadata.title.is_none());
        assert!(metadata.artist.is_none());
        assert!(metadata.album.is_none());
        assert!(metadata.date.is_none());
        assert!(metadata.genre.is_none());
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1023), "1023 B");
        assert_eq!(format_file_size(1024), "1.00 KB");
        assert_eq!(format_file_size(1536), "1.50 KB");
        assert_eq!(format_file_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_file_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_file_size(1024 * 1024 * 1024 * 1024), "1.00 TB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(0)), "00:00");
        assert_eq!(format_duration(Duration::from_secs(5)), "00:05");
        assert_eq!(format_duration(Duration::from_secs(59)), "00:59");
        assert_eq!(format_duration(Duration::from_secs(60)), "01:00");
        assert_eq!(format_duration(Duration::from_secs(61)), "01:01");
        assert_eq!(format_duration(Duration::from_secs(3599)), "59:59");
        assert_eq!(format_duration(Duration::from_secs(3600)), "01:00:00");
        assert_eq!(format_duration(Duration::from_secs(3601)), "01:00:01");
        assert_eq!(format_duration(Duration::from_secs(3661)), "01:01:01");
        assert_eq!(format_duration(Duration::from_secs(43261)), "12:01:01");
    }

    #[test]
    fn test_create_unique_filename() {
        let temp_dir = tempdir().unwrap();
        
        // Test with a non-existent file
        let unique_path = create_unique_filename(temp_dir.path(), "test.txt");
        assert_eq!(unique_path.file_name().unwrap().to_str().unwrap(), "test.txt");
        
        // Create the file
        std::fs::write(&unique_path, "test content").unwrap();
        
        // Test with an existing file
        let unique_path2 = create_unique_filename(temp_dir.path(), "test.txt");
        assert_eq!(unique_path2.file_name().unwrap().to_str().unwrap(), "test_1.txt");
        
        // Create the second file
        std::fs::write(&unique_path2, "test content 2").unwrap();
        
        // Test with another existing file
        let unique_path3 = create_unique_filename(temp_dir.path(), "test.txt");
        assert_eq!(unique_path3.file_name().unwrap().to_str().unwrap(), "test_2.txt");
        
        // Test with a file without extension
        let no_ext_path = temp_dir.path().join("noext");
        std::fs::write(&no_ext_path, "test content").unwrap();
        
        let unique_no_ext = create_unique_filename(temp_dir.path(), "noext");
        assert_eq!(unique_no_ext.file_name().unwrap().to_str().unwrap(), "noext_1");
    }

    #[test]
    fn test_ensure_directory_exists() {
        let temp_dir = tempdir().unwrap();
        
        // Test with a non-existent directory
        let new_dir = temp_dir.path().join("new_dir");
        assert!(!new_dir.exists());
        assert!(ensure_directory_exists(&new_dir).is_ok());
        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
        
        // Test with an existing directory
        assert!(ensure_directory_exists(&new_dir).is_ok());
        
        // Test with a file instead of a directory
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();
        assert!(ensure_directory_exists(&file_path).is_err());
        
        // Test with nested directories
        let nested_dir = temp_dir.path().join("nested").join("dir");
        assert!(!nested_dir.exists());
        assert!(ensure_directory_exists(&nested_dir).is_ok());
        assert!(nested_dir.exists());
        assert!(nested_dir.is_dir());
    }

    #[test]
    fn test_get_directory_size() {
        let temp_dir = tempdir().unwrap();
        
        // Test with an empty directory
        assert_eq!(get_directory_size(temp_dir.path()).unwrap(), 0);
        
        // Add some files
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, b"content1").unwrap();
        std::fs::write(&file2, b"content2").unwrap();
        
        let size = get_directory_size(temp_dir.path()).unwrap();
        assert_eq!(size, 16); // 8 bytes each
        
        // Add a subdirectory with files
        let sub_dir = temp_dir.path().join("sub_dir");
        std::fs::create_dir(&sub_dir).unwrap();
        let file3 = sub_dir.join("file3.txt");
        std::fs::write(&file3, b"content3").unwrap();
        
        let size = get_directory_size(temp_dir.path()).unwrap();
        assert_eq!(size, 24); // 8 + 8 + 8 bytes
        
        // Test with a non-existent directory
        let non_existent = temp_dir.path().join("non_existent");
        assert!(get_directory_size(&non_existent).is_err());
        
        // Test with a file instead of a directory
        assert!(get_directory_size(&file1).is_err());
    }

    #[test]
    fn test_cleanup_temp_files() {
        let temp_dir = tempdir().unwrap();
        
        // Create some files with different ages
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();
        
        // Set different modification times
        let now = std::time::SystemTime::now();
        let past = now - std::time::Duration::from_secs(3600); // 1 hour ago
        
        file::set_file_times(&file1, past, past).unwrap();
        
        // Clean up files older than 30 minutes
        let cleaned_count = cleanup_temp_files(temp_dir.path(), Duration::from_secs(1800)).unwrap();
        assert_eq!(cleaned_count, 1);
        assert!(!file1.exists());
        assert!(file2.exists());
        
        // Clean up all files
        let cleaned_count = cleanup_temp_files(temp_dir.path(), Duration::from_secs(0)).unwrap();
        assert_eq!(cleaned_count, 1);
        assert!(!file2.exists());
    }

    #[test]
    fn test_is_audio_file_by_content() {
        let temp_dir = tempdir().unwrap();
        
        // Test with a supported audio file
        let audio_path = temp_dir.path().join("test.mp3");
        std::fs::write(&audio_path, b"dummy audio data").unwrap();
        assert!(is_audio_file_by_content(&audio_path).unwrap());
        
        // Test with an unsupported file
        let text_path = temp_dir.path().join("test.txt");
        std::fs::write(&text_path, b"text data").unwrap();
        assert!(!is_audio_file_by_content(&text_path).unwrap());
        
        // Test with a non-existent file
        let non_existent = temp_dir.path().join("non_existent.mp3");
        assert!(is_audio_file_by_content(&non_existent).is_err());
    }

    #[test]
    fn test_get_audio_quality_category() {
        assert_eq!(get_audio_quality_category(None), "Unknown");
        assert_eq!(get_audio_quality_category(Some(32000)), "Low");
        assert_eq!(get_audio_quality_category(Some(64000)), "Low");
        assert_eq!(get_audio_quality_category(Some(96000)), "Medium");
        assert_eq!(get_audio_quality_category(Some(128000)), "Medium");
        assert_eq!(get_audio_quality_category(Some(192000)), "High");
        assert_eq!(get_audio_quality_category(Some(256000)), "High");
        assert_eq!(get_audio_quality_category(Some(320000)), "Very High");
        assert_eq!(get_audio_quality_category(Some(512000)), "Very High");
    }

    #[test]
    fn test_estimate_transcription_time() {
        let duration = Duration::from_secs(60);
        let estimated = estimate_transcription_time(duration);
        assert_eq!(estimated, Duration::from_secs(6)); // 10% of original
        
        let duration = Duration::from_secs(300); // 5 minutes
        let estimated = estimate_transcription_time(duration);
        assert_eq!(estimated, Duration::from_secs(30)); // 10% of original
        
        let duration = Duration::from_secs(0);
        let estimated = estimate_transcription_time(duration);
        assert_eq!(estimated, Duration::from_secs(0));
    }

    #[test]
    fn test_constants() {
        assert!(SUPPORTED_AUDIO_EXTENSIONS.contains(&"mp3"));
        assert!(SUPPORTED_AUDIO_EXTENSIONS.contains(&"wav"));
        assert!(SUPPORTED_AUDIO_EXTENSIONS.contains(&"flac"));
        assert!(SUPPORTED_AUDIO_EXTENSIONS.contains(&"aac"));
        assert!(SUPPORTED_AUDIO_EXTENSIONS.contains(&"ogg"));
        assert!(SUPPORTED_AUDIO_EXTENSIONS.contains(&"webm"));
        assert!(SUPPORTED_AUDIO_EXTENSIONS.contains(&"m4a"));
        assert!(!SUPPORTED_AUDIO_EXTENSIONS.contains(&"txt"));
        
        assert_eq!(DEFAULT_MAX_FILE_SIZE, 100 * 1024 * 1024); // 100MB
        assert_eq!(MIN_FILE_SIZE, 1024); // 1KB
    }

    #[test]
    fn test_edge_cases() {
        // Test with very large file size
        let large_size = 1024 * 1024 * 1024 * 1024; // 1TB
        assert_eq!(format_file_size(large_size), "1.00 TB");
        
        // Test with very long duration
        let long_duration = Duration::from_secs(86400); // 24 hours
        assert_eq!(format_duration(long_duration), "24:00:00");
        
        // Test with very short duration
        let short_duration = Duration::from_nanos(1);
        assert_eq!(format_duration(short_duration), "00:00");
        
        // Test with zero file size
        assert_eq!(format_file_size(0), "0 B");
        
        // Test with zero duration
        assert_eq!(format_duration(Duration::from_secs(0)), "00:00");
    }

    #[test]
    fn test_unicode_filenames() {
        let temp_dir = tempdir().unwrap();
        
        // Test with Unicode characters in filename
        let unicode_path = temp_dir.path().join("tëst_ñamé.mp3");
        std::fs::write(&unicode_path, b"dummy audio data").unwrap();
        
        assert!(is_supported_audio_file(&unicode_path));
        assert_eq!(detect_audio_file_type(&unicode_path), AudioFileType::Mp3);
        
        // Test with unique filename creation
        let unique_path = create_unique_filename(temp_dir.path(), "tëst_ñamé.mp3");
        assert_eq!(unique_path.file_name().unwrap().to_str().unwrap(), "tëst_ñamé_1.mp3");
    }

    #[test]
    fn test_complex_directory_structure() {
        let temp_dir = tempdir().unwrap();
        
        // Create a complex directory structure
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");
        let subdir1 = dir1.join("subdir1");
        
        ensure_directory_exists(&dir1).unwrap();
        ensure_directory_exists(&dir2).unwrap();
        ensure_directory_exists(&subdir1).unwrap();
        
        // Add files to different directories
        let file1 = dir1.join("file1.mp3");
        let file2 = dir2.join("file2.wav");
        let file3 = subdir1.join("file3.flac");
        
        std::fs::write(&file1, b"audio data 1").unwrap();
        std::fs::write(&file2, b"audio data 2").unwrap();
        std::fs::write(&file3, b"audio data 3").unwrap();
        
        // Test directory size calculation
        let total_size = get_directory_size(temp_dir.path()).unwrap();
        assert_eq!(total_size, 36); // 12 bytes each
        
        // Test cleanup with nested structure
        let cleaned_count = cleanup_temp_files(temp_dir.path(), Duration::from_secs(0)).unwrap();
        assert_eq!(cleaned_count, 3);
        
        assert!(!file1.exists());
        assert!(!file2.exists());
        assert!(!file3.exists());
    }
}