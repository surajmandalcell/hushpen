//! Utility modules for the ASRPro application
//!
//! This module contains various utility functions and helpers used throughout the application.

pub mod error;
pub mod file_utils;

// Re-export commonly used types
pub use error::{AppError, AppResult, ErrorType, IntoAppError};