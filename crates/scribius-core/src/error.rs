use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScribiusError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Data error: {0}")]
    Data(String),
}

pub type Result<T> = std::result::Result<T, ScribiusError>;
