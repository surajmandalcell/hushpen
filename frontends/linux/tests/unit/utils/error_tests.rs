//! Unit tests for error handling utilities

use asrpro_gtk4::utils::{AppError, ErrorType, IntoAppError};
use std::io;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_creation() {
        // Test creating different types of errors
        let api_error = AppError::api("API request failed");
        assert!(matches!(api_error, AppError::Api { .. }));
        assert_eq!(api_error.to_string(), "API Error: API request failed");

        let file_error = AppError::file("File not found");
        assert!(matches!(file_error, AppError::File { .. }));
        assert_eq!(file_error.to_string(), "File Error: File not found");

        let ui_error = AppError::ui("Widget creation failed");
        assert!(matches!(ui_error, AppError::Ui { .. }));
        assert_eq!(ui_error.to_string(), "UI Error: Widget creation failed");

        let audio_error = AppError::audio("Audio device not found");
        assert!(matches!(audio_error, AppError::Audio { .. }));
        assert_eq!(audio_error.to_string(), "Audio Error: Audio device not found");

        let config_error = AppError::config("Invalid configuration");
        assert!(matches!(config_error, AppError::Config { .. }));
        assert_eq!(config_error.to_string(), "Config Error: Invalid configuration");

        let generic_error = AppError::generic("Something went wrong");
        assert!(matches!(generic_error, AppError::Generic { .. }));
        assert_eq!(generic_error.to_string(), "Application Error: Something went wrong");
    }

    #[test]
    fn test_app_error_with_source() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file.txt");
        let file_error = AppError::file_with_source("Failed to read file", io_error);
        
        assert!(matches!(file_error, AppError::File { .. }));
        assert!(file_error.to_string().contains("Failed to read file"));
    }

    #[test]
    fn test_user_message() {
        let api_error = AppError::api("Connection timeout");
        assert_eq!(api_error.user_message(), "Network error: Connection timeout");

        let file_error = AppError::file("Permission denied");
        assert_eq!(file_error.user_message(), "File error: Permission denied");

        let ui_error = AppError::ui("Invalid input");
        assert_eq!(ui_error.user_message(), "Interface error: Invalid input");

        let audio_error = AppError::audio("Device busy");
        assert_eq!(audio_error.user_message(), "Audio error: Device busy");

        let config_error = AppError::config("Missing setting");
        assert_eq!(config_error.user_message(), "Configuration error: Missing setting");

        let generic_error = AppError::generic("Unexpected error");
        assert_eq!(generic_error.user_message(), "Error: Unexpected error");
    }

    #[test]
    fn test_short_message() {
        let api_error = AppError::api("Connection timeout");
        assert_eq!(api_error.short_message(), "Network request failed");

        let file_error = AppError::file("Permission denied");
        assert_eq!(file_error.short_message(), "File operation failed");

        let ui_error = AppError::ui("Invalid input");
        assert_eq!(ui_error.short_message(), "Interface operation failed");

        let audio_error = AppError::audio("Device busy");
        assert_eq!(audio_error.short_message(), "Audio operation failed");

        let config_error = AppError::config("Missing setting");
        assert_eq!(config_error.short_message(), "Configuration error");

        let generic_error = AppError::generic("Unexpected error");
        assert_eq!(generic_error.short_message(), "Operation failed");
    }

    #[test]
    fn test_into_app_error() {
        // Test converting a Result to AppResult
        let ok_result: Result<String, io::Error> = Ok("success".to_string());
        let app_result = ok_result.into_app_error(ErrorType::File);
        assert!(app_result.is_ok());
        assert_eq!(app_result.unwrap(), "success");

        let err_result: Result<String, io::Error> = Err(io::Error::new(io::ErrorKind::NotFound, "file.txt"));
        let app_result = err_result.into_app_error(ErrorType::File);
        assert!(app_result.is_err());
        
        match app_result.unwrap_err() {
            AppError::File { message, .. } => {
                assert!(message.contains("file.txt"));
            },
            _ => panic!("Expected File error"),
        }
    }

    #[test]
    fn test_http_error_message() {
        use asrpro_gtk4::utils::error::helpers;
        use reqwest::StatusCode;

        assert_eq!(
            helpers::http_error_message(StatusCode::BAD_REQUEST),
            "Bad request. Please check your input and try again."
        );
        assert_eq!(
            helpers::http_error_message(StatusCode::UNAUTHORIZED),
            "Authentication failed. Please check your credentials."
        );
        assert_eq!(
            helpers::http_error_message(StatusCode::FORBIDDEN),
            "Access denied. You don't have permission to perform this action."
        );
        assert_eq!(
            helpers::http_error_message(StatusCode::NOT_FOUND),
            "The requested resource was not found."
        );
        assert_eq!(
            helpers::http_error_message(StatusCode::TOO_MANY_REQUESTS),
            "Too many requests. Please wait a moment and try again."
        );
        assert_eq!(
            helpers::http_error_message(StatusCode::INTERNAL_SERVER_ERROR),
            "Server error. Please try again later."
        );
        assert_eq!(
            helpers::http_error_message(StatusCode::BAD_GATEWAY),
            "Service temporarily unavailable. Please try again later."
        );
        assert_eq!(
            helpers::http_error_message(StatusCode::SERVICE_UNAVAILABLE),
            "Service unavailable. Please try again later."
        );
        assert_eq!(
            helpers::http_error_message(StatusCode::IM_A_TEAPOT),
            "Request failed with status code: 418"
        );
    }

    #[test]
    fn test_file_io_error_message() {
        use asrpro_gtk4::utils::error::helpers;

        let not_found_error = io::Error::new(io::ErrorKind::NotFound, "file.txt");
        assert_eq!(
            helpers::file_io_error_message(&not_found_error),
            "The specified file was not found."
        );

        let permission_error = io::Error::new(io::ErrorKind::PermissionDenied, "file.txt");
        assert_eq!(
            helpers::file_io_error_message(&permission_error),
            "Permission denied. Please check file permissions."
        );

        let exists_error = io::Error::new(io::ErrorKind::AlreadyExists, "file.txt");
        assert_eq!(
            helpers::file_io_error_message(&exists_error),
            "The file already exists."
        );

        let invalid_input_error = io::Error::new(io::ErrorKind::InvalidInput, "file.txt");
        assert_eq!(
            helpers::file_io_error_message(&invalid_input_error),
            "Invalid file path or format."
        );

        let timeout_error = io::Error::new(io::ErrorKind::TimedOut, "file.txt");
        assert_eq!(
            helpers::file_io_error_message(&timeout_error),
            "File operation timed out."
        );

        let write_zero_error = io::Error::new(io::ErrorKind::WriteZero, "file.txt");
        assert_eq!(
            helpers::file_io_error_message(&write_zero_error),
            "No data could be written to the file."
        );

        let interrupted_error = io::Error::new(io::ErrorKind::Interrupted, "file.txt");
        assert_eq!(
            helpers::file_io_error_message(&interrupted_error),
            "File operation was interrupted."
        );

        let unexpected_eof_error = io::Error::new(io::ErrorKind::UnexpectedEof, "file.txt");
        assert_eq!(
            helpers::file_io_error_message(&unexpected_eof_error),
            "Unexpected end of file."
        );

        let other_error = io::Error::new(io::ErrorKind::Other, "file.txt");
        assert!(helpers::file_io_error_message(&other_error).contains("File error:"));
    }

    #[test]
    fn test_audio_error_message() {
        use asrpro_gtk4::utils::error::helpers;

        assert_eq!(
            helpers::audio_error_message("No audio device found"),
            "Audio device not found or not available. Please check your audio settings."
        );

        assert_eq!(
            helpers::audio_error_message("Unsupported audio format"),
            "Unsupported audio format. Please use a supported audio format."
        );

        assert_eq!(
            helpers::audio_error_message("Permission denied to access audio device"),
            "Permission denied to access audio device. Please check audio permissions."
        );

        assert_eq!(
            helpers::audio_error_message("Some other audio error"),
            "Audio error: Some other audio error"
        );
    }

    #[test]
    fn test_error_debug_format() {
        let error = AppError::api("Test error");
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Api"));
        assert!(debug_str.contains("Test error"));
    }

    #[test]
    fn test_error_clone() {
        let error1 = AppError::file("Test file error");
        let error2 = error1.clone();
        
        assert_eq!(error1.to_string(), error2.to_string());
        assert!(matches!(error1, AppError::File { .. }));
        assert!(matches!(error2, AppError::File { .. }));
    }

    #[test]
    fn test_error_source() {
        let error = AppError::generic("Test error");
        assert!(error.source().is_none());
    }

    #[test]
    fn test_complex_error_scenarios() {
        // Test creating an error with a complex source
        let source_error = std::fmt::Error;
        let app_error = AppError::ui_with_source("Failed to format UI", source_error);
        
        assert!(matches!(app_error, AppError::Ui { .. }));
        assert!(app_error.to_string().contains("Failed to format UI"));
        
        // Test user-friendly messages for complex scenarios
        assert_eq!(app_error.user_message(), "Interface error: Failed to format UI");
        assert_eq!(app_error.short_message(), "Interface operation failed");
    }

    #[test]
    fn test_error_chain() {
        // Test error conversion chain
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let file_result: Result<(), io::Error> = Err(io_error);
        let app_result = file_result.into_app_error(ErrorType::File);
        
        assert!(app_result.is_err());
        let app_error = app_result.unwrap_err();
        
        assert!(matches!(app_error, AppError::File { .. }));
        assert!(app_error.to_string().contains("access denied"));
        assert_eq!(app_error.user_message(), "File error: access denied");
    }
}