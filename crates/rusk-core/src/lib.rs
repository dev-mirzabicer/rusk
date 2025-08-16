//! # Rusk Core Library
//!
//! A comprehensive task management library with advanced series-based recurrence,
//! timezone support, and sophisticated materialization strategies.
//!
//! ## Features
//!
//! - **Series-Based Recurrence**: Industry-standard approach to recurring tasks
//!   with support for exceptions (skip, move, override)
//! - **Timezone Awareness**: Full IANA timezone support with DST handling
//! - **Intelligent Materialization**: Just-in-time task instance creation with
//!   configurable lookahead windows
//! - **Advanced Filtering**: Powerful query system with logical operators
//! - **Type Safety**: Compile-time checked SQL queries with sqlx
//! - **Performance Optimized**: Efficient algorithms with memory-conscious design
//!
//! ## Core Modules
//!
//! - [`db`]: Database connection and migration management
//! - [`models`]: Core data structures and transfer objects
//! - [`repository`]: Data access layer with Repository pattern
//! - [`recurrence`]: Recurrence calculation and materialization engines
//! - [`timezone`]: Timezone utilities and validation
//! - [`error`]: Comprehensive error types with context
//! - [`query`]: Advanced filtering and query parsing
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use rusk_core::{
//!     db, models::NewTaskData, repository::{Repository, SqliteRepository},
//!     recurrence::{MaterializationManager, MaterializationConfig}
//! };
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize database
//!     let pool = db::establish_connection("tasks.db").await?;
//!     
//!     // Create repository with materialization
//!     let config = MaterializationConfig::default();
//!     let manager = MaterializationManager::new(config);
//!     let repo = SqliteRepository::new(pool, manager);
//!     
//!     // Add a recurring task
//!     let task_data = NewTaskData {
//!         name: "Daily standup".to_string(),
//!         rrule: Some("FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR".to_string()),
//!         timezone: Some("America/New_York".to_string()),
//!         ..Default::default()
//!     };
//!     
//!     let task = repo.add_task(task_data).await?;
//!     println!("Created task: {}", task.name);
//!     
//!     Ok(())
//! }
//! ```

pub mod db;
pub mod error;
pub mod models;
pub mod query;
pub mod repository;
pub mod recurrence;
pub mod timezone;