//! Pinned model downloads shared by desktop and mobile services.

pub mod download;
pub mod fetch;
pub mod policy;
#[cfg(any(test, feature = "test-support"))]
pub mod test_server;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// User-requested, pinned model files from Hugging Face and its CDN.
    ModelDownload,
    /// An endpoint the user enables, with HTTP limited to loopback.
    Endpoint,
    /// The signed update manifest from GitHub and its CDN.
    UpdateCheck,
    /// A release file named by the verified update manifest.
    UpdateDownload,
}

impl Purpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelDownload => "model_download",
            Self::Endpoint => "endpoint",
            Self::UpdateCheck => "update_check",
            Self::UpdateDownload => "update_download",
        }
    }
}
