use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("Migration error")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("Task not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Task is blocked by: {0}")]
    TaskBlocked(String),

    #[error("Ambiguous short ID: {0}")]
    AmbiguousId(String),

    #[error("An unknown error has occurred.")]
    Unknown,
}