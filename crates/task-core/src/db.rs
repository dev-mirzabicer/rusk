use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;
use crate::error::CoreError;

// Re-export the pool for use in other parts of the core crate
pub use sqlx::SqlitePool as DbPool;

/// Establishes a connection pool to the SQLite database and runs migrations.
///
/// # Arguments
///
/// * `db_path` - The path to the SQLite database file.
///
/// # Returns
///
/// A `Result` containing the `SqlitePool` or a `CoreError` if the connection fails
/// or migrations cannot be run.
pub async fn establish_connection(db_path: &str) -> Result<SqlitePool, CoreError> {
    // Create the database file and directory if they don't exist
    if let Some(parent) = Path::new(db_path).parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent)
                .await?;
        }
    }
    if !Path::new(db_path).exists() {
        tokio::fs::File::create(db_path)
            .await?;
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_path)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(pool)
}