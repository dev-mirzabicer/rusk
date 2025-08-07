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

    #[error("Ambiguous short ID. Did you mean one of these?")]
    AmbiguousId(Vec<(String, String)>), // Vec of (ID, Name)

    #[error("Circular dependency detected: Task '{0}' cannot depend on '{1}'.")]
    CircularDependency(String, String),

    #[error("An unknown error has occurred.")]
    Unknown,
}