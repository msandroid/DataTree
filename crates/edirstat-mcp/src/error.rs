use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Snapshot(#[from] edirstat_core::EdirstatError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl ApiError {
    #[must_use]
    pub fn msg(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }
}
