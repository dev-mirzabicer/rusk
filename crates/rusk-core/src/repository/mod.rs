use crate::db::DbPool;
use crate::error::CoreError;
use crate::models::{
    CompletionResult, NewTaskData, Project, Task, TaskPriority, TaskStatus,
    UpdateTaskData, TaskSeries, SeriesException, NewSeriesData, UpdateSeriesData, 
    NewSeriesException, EditScope, SeriesStatistics,
};
use crate::query::Query;
use crate::recurrence::MaterializationManager;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

// Re-export domain modules
pub mod tasks;
pub mod projects;
pub mod series;
pub mod materialization;
pub mod exceptions;
pub mod query_builder;

// Traits are defined in this module and implemented in respective domain modules

// Core types needed across domains
#[derive(Debug, Clone, FromRow)]
pub struct TaskQueryResult {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub due_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub project_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub series_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub depth: i32,
    pub tags: Option<String>,
}

/// Domain-specific trait for task operations
#[async_trait]
pub trait TaskRepository {
    async fn add_task(&self, data: NewTaskData) -> Result<Task, CoreError>;
    async fn find_task_by_id(&self, id: Uuid) -> Result<Option<Task>, CoreError>;
    async fn find_tasks_by_short_id_prefix(&self, short_id: &str) -> Result<Vec<Task>, CoreError>;
    async fn find_tasks_with_details(&self, query: &Query) -> Result<Vec<TaskQueryResult>, CoreError>;
    async fn delete_task(&self, id: Uuid) -> Result<(), CoreError>;
    async fn complete_task(&self, id: Uuid) -> Result<CompletionResult, CoreError>;
    async fn cancel_task(&self, id: Uuid) -> Result<Task, CoreError>;
    async fn update_task(&self, id: Uuid, data: UpdateTaskData, scope: Option<EditScope>) -> Result<Task, CoreError>;
}

/// Domain-specific trait for project operations
#[async_trait]
pub trait ProjectRepository {
    async fn add_project(&self, name: String, description: Option<String>) -> Result<Project, CoreError>;
    async fn find_project_by_id(&self, id: Uuid) -> Result<Option<Project>, CoreError>;
    async fn find_project_by_name(&self, name: &str) -> Result<Option<Project>, CoreError>;
    async fn find_projects(&self) -> Result<Vec<Project>, CoreError>;
    async fn delete_project(&self, name: String) -> Result<(), CoreError>;
}

/// Domain-specific trait for series operations
#[async_trait]
pub trait SeriesRepository {
    async fn create_series(&self, data: NewSeriesData) -> Result<TaskSeries, CoreError>;
    async fn find_series_by_id(&self, id: Uuid) -> Result<Option<TaskSeries>, CoreError>;
    async fn find_series_by_template(&self, template_id: Uuid) -> Result<Option<TaskSeries>, CoreError>;
    async fn update_series(&self, id: Uuid, data: UpdateSeriesData) -> Result<TaskSeries, CoreError>;
    async fn delete_series(&self, id: Uuid) -> Result<(), CoreError>;
    async fn find_active_series(&self) -> Result<Vec<TaskSeries>, CoreError>;
    async fn duplicate_series(&self, series_id: Uuid, new_name: String, new_timezone: Option<String>) -> Result<TaskSeries, CoreError>;
    async fn archive_completed_series(&self, series_id: Uuid) -> Result<(), CoreError>;
    async fn bulk_update_series(&self, updates: Vec<(Uuid, UpdateSeriesData)>) -> Result<Vec<TaskSeries>, CoreError>;
    async fn find_series_by_pattern(&self, pattern: &str) -> Result<Vec<TaskSeries>, CoreError>;
    async fn get_series_statistics(&self, series_id: Uuid) -> Result<SeriesStatistics, CoreError>;
}

/// Domain-specific trait for materialization operations
#[async_trait]
pub trait MaterializationRepository {
    async fn refresh_series_materialization(&self, window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> Result<(), CoreError>;
    async fn find_materialized_tasks_for_series(&self, series_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Task>, CoreError>;
    async fn update_series_materialization_boundary(&self, series_id: Uuid, boundary: DateTime<Utc>) -> Result<(), CoreError>;
}

/// Domain-specific trait for exception operations
#[async_trait]
pub trait ExceptionRepository {
    async fn add_series_exception(&self, exception: NewSeriesException) -> Result<SeriesException, CoreError>;
    async fn find_series_exceptions(&self, series_id: Uuid) -> Result<Vec<SeriesException>, CoreError>;
    async fn remove_series_exception(&self, series_id: Uuid, occurrence_dt: DateTime<Utc>) -> Result<(), CoreError>;
    async fn add_bulk_series_exceptions(&self, exceptions: Vec<NewSeriesException>) -> Result<Vec<SeriesException>, CoreError>;
    async fn remove_bulk_series_exceptions(&self, series_id: Uuid, occurrence_dts: Vec<DateTime<Utc>>) -> Result<usize, CoreError>;
    async fn validate_exception_conflicts(&self, series_id: Uuid, new_exception: &NewSeriesException) -> Result<Vec<SeriesException>, CoreError>;
    async fn override_occurrence_with_task(&self, series_id: Uuid, occurrence_dt: DateTime<Utc>, override_task_data: NewTaskData) -> Result<Task, CoreError>;
    async fn move_occurrence_with_validation(&self, series_id: Uuid, from_dt: DateTime<Utc>, to_dt: DateTime<Utc>, timezone: &str) -> Result<Task, CoreError>;
}

/// Main repository trait that composes all domain traits
#[async_trait]
pub trait Repository: 
    TaskRepository + 
    ProjectRepository + 
    SeriesRepository + 
    MaterializationRepository + 
    ExceptionRepository 
{
    // This trait automatically composes all domain-specific repositories
    // Individual domain operations are defined in their respective traits
}

/// SQLite implementation of the repository pattern
pub struct SqliteRepository {
    pool: DbPool,
    materialization_manager: MaterializationManager,
}

impl SqliteRepository {
    pub fn new(pool: DbPool, materialization_manager: MaterializationManager) -> Self {
        Self { pool, materialization_manager }
    }
    
    /// Get a reference to the database pool for internal use across modules
    pub(crate) fn pool(&self) -> &DbPool {
        &self.pool
    }
    
    /// Get a reference to the materialization manager for internal use
    pub(crate) fn materialization_manager(&self) -> &MaterializationManager {
        &self.materialization_manager
    }
}

// The main Repository trait implementation will automatically be available
// when all domain trait implementations are defined
impl Repository for SqliteRepository {}