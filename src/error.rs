//! Error handling for watertight-meshes-rs.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, WatertightError>;

#[derive(Error, Debug)]
pub enum WatertightError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization/parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Mesh format error: {0}")]
    FormatError(String),

    #[error("Unsupported mesh extension: {0}")]
    UnsupportedFormat(String),

    #[error("Invalid mesh topology: {0}")]
    TopologyError(String),

    #[error("Environment error: {0}")]
    EnvironmentError(String),

    #[error("Model download error: {0}")]
    DownloadError(String),

    #[error("Workflow error: {0}")]
    WorkflowError(String),
}
