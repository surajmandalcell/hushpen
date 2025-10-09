use std::fmt;

/// Application error types for different categories
#[derive(Debug, Clone)]
pub enum AppError {
    /// API-related errors (HTTP requests, responses, etc.)
    Api { message: String, source: Option<String> },
    
    /// File-related errors (reading, writing, format issues)
    File { message: String, source: Option<String> },
    
    /// UI-related errors (widget creation, layout, etc.)
    Ui { message: String, source: Option<String> },
    
    /// Audio-related errors (processing, format, device issues)
    Audio { message: String, source: Option<String> },
    
    /// Configuration-related errors (parsing, validation, etc.)
    Config { message: String, source: Option<String> },
    
    /// Generic application errors
    Generic { message: String, source: Option<String> },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api { message, .. } => write!(f, "API Error: {}", message),
            Self::File { message, .. } => write!(f, "File Error: {}", message),
            Self::Ui { message, .. } => write!(f, "UI Error: {}", message),
            Self::Audio { message, .. } => write!(f, "Audio Error: {}", message),
            Self::Config { message, .. } => write!(f, "Config Error: {}", message),
            Self::Generic { message, .. } => write!(f, "Application Error: {}", message),
        }
    }
}

impl AppError {
    /// Create a new API error
    pub fn api<S: Into<String>>(message: S) -> Self {
        Self::Api {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new API error with a source
    pub fn api_with_source<S: Into<String>, E: std::fmt::Display>(message: S, source: E) -> Self {
        Self::Api {
            message: message.into(),
            source: Some(source.to_string()),
        }
    }
    
    /// Create a new file error
    pub fn file<S: Into<String>>(message: S) -> Self {
        Self::File {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new file error with a source
    pub fn file_with_source<S: Into<String>, E: std::fmt::Display>(message: S, source: E) -> Self {
        Self::File {
            message: message.into(),
            source: Some(source.to_string()),
        }
    }
    
    /// Create a new UI error
    pub fn ui<S: Into<String>>(message: S) -> Self {
        Self::Ui {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new UI error with a source
    pub fn ui_with_source<S: Into<String>, E: std::fmt::Display>(message: S, source: E) -> Self {
        Self::Ui {
            message: message.into(),
            source: Some(source.to_string()),
        }
    }
    
    /// Create a new audio error
    pub fn audio<S: Into<String>>(message: S) -> Self {
        Self::Audio {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new audio error with a source
    pub fn audio_with_source<S: Into<String>, E: std::fmt::Display>(message: S, source: E) -> Self {
        Self::Audio {
            message: message.into(),
            source: Some(source.to_string()),
        }
    }
    
    /// Create a new config error
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new config error with a source
    pub fn config_with_source<S: Into<String>, E: std::fmt::Display>(message: S, source: E) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(source.to_string()),
        }
    }
    
    /// Create a new generic error
    pub fn generic<S: Into<String>>(message: S) -> Self {
        Self::Generic {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new generic error with a source
    pub fn generic_with_source<S: Into<String>, E: std::fmt::Display>(message: S, source: E) -> Self {
        Self::Generic {
            message: message.into(),
            source: Some(source.to_string()),
        }
    }
    
    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Self::Api { message, .. } => {
                format!("Network error: {}", message)
            },
            Self::File { message, .. } => {
                format!("File error: {}", message)
            },
            Self::Ui { message, .. } => {
                format!("Interface error: {}", message)
            },
            Self::Audio { message, .. } => {
                format!("Audio error: {}", message)
            },
            Self::Config { message, .. } => {
                format!("Configuration error: {}", message)
            },
            Self::Generic { message, .. } => {
                format!("Error: {}", message)
            },
        }
    }
    
    /// Get a short error message for UI notifications
    pub fn short_message(&self) -> String {
        match self {
            Self::Api { .. } => "Network request failed".to_string(),
            Self::File { .. } => "File operation failed".to_string(),
            Self::Ui { .. } => "Interface operation failed".to_string(),
            Self::Audio { .. } => "Audio operation failed".to_string(),
            Self::Config { .. } => "Configuration error".to_string(),
            Self::Generic { .. } => "Operation failed".to_string(),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// Result type alias for application operations
pub type AppResult<T> = Result<T, AppError>;

/// Trait for converting common error types to AppError
pub trait IntoAppError<T> {
    /// Convert the result to an AppResult
    fn into_app_error(self, error_type: ErrorType) -> AppResult<T>;
}

/// Error type categories for conversion
#[derive(Debug, Clone, Copy)]
pub enum ErrorType {
    Api,
    File,
    Ui,
    Audio,
    Config,
    Generic,
}

// Implement conversions for common error types
impl<T, E> IntoAppError<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_app_error(self, error_type: ErrorType) -> AppResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => {
                let message = error.to_string();
                match error_type {
                    ErrorType::Api => Err(AppError::api_with_source(message, error)),
                    ErrorType::File => Err(AppError::file_with_source(message, error)),
                    ErrorType::Ui => Err(AppError::ui_with_source(message, error)),
                    ErrorType::Audio => Err(AppError::audio_with_source(message, error)),
                    ErrorType::Config => Err(AppError::config_with_source(message, error)),
                    ErrorType::Generic => Err(AppError::generic_with_source(message, error)),
                }
            }
        }
    }
}

/// Helper functions for creating user-friendly error messages
pub mod helpers {
    use super::*;
    
    /// Create a user-friendly message for HTTP errors
    pub fn http_error_message(status: reqwest::StatusCode) -> String {
        match status.as_u16() {
            400 => "Bad request. Please check your input and try again.".to_string(),
            401 => "Authentication failed. Please check your credentials.".to_string(),
            403 => "Access denied. You don't have permission to perform this action.".to_string(),
            404 => "The requested resource was not found.".to_string(),
            429 => "Too many requests. Please wait a moment and try again.".to_string(),
            500 => "Server error. Please try again later.".to_string(),
            502 => "Service temporarily unavailable. Please try again later.".to_string(),
            503 => "Service unavailable. Please try again later.".to_string(),
            _ => format!("Request failed with status code: {}", status),
        }
    }
    
    /// Create a user-friendly message for file I/O errors
    pub fn file_io_error_message(error: &std::io::Error) -> String {
        match error.kind() {
            std::io::ErrorKind::NotFound => "The specified file was not found.".to_string(),
            std::io::ErrorKind::PermissionDenied => "Permission denied. Please check file permissions.".to_string(),
            std::io::ErrorKind::AlreadyExists => "The file already exists.".to_string(),
            std::io::ErrorKind::InvalidInput => "Invalid file path or format.".to_string(),
            std::io::ErrorKind::TimedOut => "File operation timed out.".to_string(),
            std::io::ErrorKind::WriteZero => "No data could be written to the file.".to_string(),
            std::io::ErrorKind::Interrupted => "File operation was interrupted.".to_string(),
            std::io::ErrorKind::UnexpectedEof => "Unexpected end of file.".to_string(),
            _ => format!("File error: {}", error),
        }
    }
    
    /// Create a user-friendly message for audio errors
    pub fn audio_error_message(error: &str) -> String {
        if error.contains("device") {
            "Audio device not found or not available. Please check your audio settings.".to_string()
        } else if error.contains("format") {
            "Unsupported audio format. Please use a supported audio format.".to_string()
        } else if error.contains("permission") {
            "Permission denied to access audio device. Please check audio permissions.".to_string()
        } else {
            format!("Audio error: {}", error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let api_error = AppError::api("Test API error");
        assert!(matches!(api_error, AppError::Api { .. }));
        assert_eq!(api_error.to_string(), "API Error: Test API error");
    }
    
    #[test]
    fn test_user_message() {
        let file_error = AppError::file("File not found");
        assert_eq!(file_error.user_message(), "File error: File not found");
    }
    
    #[test]
    fn test_short_message() {
        let ui_error = AppError::ui("Widget creation failed");
        assert_eq!(ui_error.short_message(), "Interface operation failed");
    }
    
    #[test]
    fn test_http_error_message() {
        let msg = helpers::http_error_message(reqwest::StatusCode::NOT_FOUND);
        assert!(msg.contains("not found"));
    }
    
    #[test]
    fn test_file_io_error_message() {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "test.txt");
        let msg = helpers::file_io_error_message(&error);
        assert!(msg.contains("not found"));
    }
}