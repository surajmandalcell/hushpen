use std::path::PathBuf;

use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub enum FileStatus {
    Pending,
    Uploading,
    Processing,
    Completed,
    Failed(String),
}

impl FileStatus {
    pub fn label(&self) -> String {
        match self {
            FileStatus::Pending => "Pending".into(),
            FileStatus::Uploading => "Uploading".into(),
            FileStatus::Processing => "Processing".into(),
            FileStatus::Completed => "Completed".into(),
            FileStatus::Failed(reason) => format!("Failed: {reason}"),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, FileStatus::Completed | FileStatus::Failed(_))
    }
}

#[derive(Clone, Debug)]
pub struct FileRecord {
    pub id: Uuid,
    pub path: PathBuf,
    pub display_name: String,
    pub status: FileStatus,
    pub progress: f64,
    pub task_id: Option<Uuid>,
    pub result_text: Option<String>,
}

impl FileRecord {
    pub fn new(path: PathBuf) -> Self {
        let display_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());

        Self {
            id: Uuid::new_v4(),
            path,
            display_name,
            status: FileStatus::Pending,
            progress: 0.0,
            task_id: None,
            result_text: None,
        }
    }
}
