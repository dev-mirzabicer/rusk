use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Project {
    #[serde(with = "uuid::serde::compact")]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Completed,
    Cancelled,
}

#[derive(Error, Debug, PartialEq)]
#[error("Invalid task status: {0}")]
pub struct ParseTaskStatusError(String);

impl FromStr for TaskStatus {
    type Err = ParseTaskStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(TaskStatus::Pending),
            "completed" => Ok(TaskStatus::Completed),
            "cancelled" => Ok(TaskStatus::Cancelled),
            _ => Err(ParseTaskStatusError(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum TaskPriority {
    None,
    Low,
    Medium,
    High,
}

#[derive(Error, Debug, PartialEq)]
#[error("Invalid task priority: {0}")]
pub struct ParseTaskPriorityError(String);

impl FromStr for TaskPriority {
    type Err = ParseTaskPriorityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(TaskPriority::None),
            "low" => Ok(TaskPriority::Low),
            "medium" => Ok(TaskPriority::Medium),
            "high" => Ok(TaskPriority::High),
            _ => Err(ParseTaskPriorityError(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
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
    /// Optional foreign key to task_series (for instances only)
    /// Template tasks: series_id = None, referenced by task_series.template_task_id
    /// Instance tasks: series_id points to their series, due_at set to occurrence time  
    /// Regular tasks: series_id = None (unchanged behavior)
    pub series_id: Option<Uuid>,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: Uuid::now_v7(),
            name: "".to_string(),
            description: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::None,
            due_at: None,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            project_id: None,
            parent_id: None,
            series_id: None,
        }
    }
}

/// Represents a filter for listing tasks.
#[derive(Debug, Clone)]
pub enum Filter {
    Status(TaskStatus),
    Tag(String),
    TagNot(String),
    Project(String),
    Priority(TaskPriority),
    DueDate(DueDate),
}

#[derive(Debug, Clone)]
pub enum DueDate {
    Today,
    Tomorrow,
    Overdue,
    Before(DateTime<Utc>),
    After(DateTime<Utc>),
}

#[derive(Debug, Clone, Default)]
pub struct NewTaskData {
    pub name: String,
    pub description: Option<String>,
    pub due_at: Option<DateTime<Utc>>,
    pub priority: Option<TaskPriority>,
    pub project_name: Option<String>, // Kept for CLI convenience
    pub project_id: Option<Uuid>,     // Used internally for transactions
    pub tags: Vec<String>,
    pub parent_id: Option<Uuid>,
    pub depends_on: Option<Uuid>,
    /// For creating recurring tasks: when present, a TaskSeries will be created
    /// with this RRULE and the task will become the template task
    pub rrule: Option<String>,
    /// For creating series instances: links the task to its series
    pub series_id: Option<Uuid>,
    /// Timezone for recurrence calculations (used with rrule)
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTaskData {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub due_at: Option<Option<DateTime<Utc>>>,
    pub priority: Option<TaskPriority>,
    pub status: Option<TaskStatus>,
    pub project_name: Option<Option<String>>,
    pub add_tags: Option<Vec<String>>,
    pub remove_tags: Option<Vec<String>>,
    pub parent_id: Option<Option<Uuid>>,
    pub depends_on: Option<Option<Uuid>>,
    /// For updating series information: modifies the associated TaskSeries
    /// Note: Series updates require EditScope to determine how to apply changes
    pub rrule: Option<Option<String>>,
    /// For updating series timezone
    pub timezone: Option<Option<String>>,
    /// For linking/unlinking tasks to/from series (advanced operations)
    pub series_id: Option<Option<Uuid>>,
}

#[derive(Debug)]
pub enum CompletionResult {
    Single(Task),
    Recurring { completed: Task, next: Task },
    /// Enhanced with series information for series-aware completion
    SeriesInstance { 
        completed: Task, 
        next: Option<Task>,
        series_id: Uuid,
        next_occurrence: Option<DateTime<Utc>>,
    },
}

// ============================================================================
// Series-Based Recurrence Models (Phase 1)
// ============================================================================

/// Represents a recurring task series with timezone-aware recurrence rules.
/// Central entity for managing recurrence patterns and metadata.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskSeries {
    /// Primary key, UUIDv7 for time-ordered performance
    #[serde(with = "uuid::serde::compact")]
    pub id: Uuid,
    /// Foreign key to template task (unique constraint)
    #[serde(with = "uuid::serde::compact")]
    pub template_task_id: Uuid,
    /// Canonical RFC 5545 recurrence rule with DTSTART
    pub rrule: String,
    /// Series start time in UTC
    pub dtstart: DateTime<Utc>,
    /// IANA timezone name (e.g., "America/New_York")
    pub timezone: String,
    /// Whether series is currently generating instances
    pub active: bool,
    /// Boundary for idempotent materialization
    pub last_materialized_until: Option<DateTime<Utc>>,
    /// Series creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modification timestamp
    pub updated_at: DateTime<Utc>,
}

impl Default for TaskSeries {
    fn default() -> Self {
        Self {
            id: Uuid::now_v7(),
            template_task_id: Uuid::now_v7(),
            rrule: String::new(),
            dtstart: Utc::now(),
            timezone: "UTC".to_string(),
            active: true,
            last_materialized_until: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// Types of exceptions that can be applied to series occurrences
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum ExceptionType {
    /// Hide this occurrence completely (exception_task_id = NULL)
    Skip,
    /// Replace with completely custom task (exception_task_id points to custom task)
    Override,
    /// Reschedule to different time (exception_task_id points to moved task)
    Move,
}

impl std::fmt::Display for ExceptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExceptionType::Skip => write!(f, "skip"),
            ExceptionType::Override => write!(f, "override"),
            ExceptionType::Move => write!(f, "move"),
        }
    }
}

impl FromStr for ExceptionType {
    type Err = ParseExceptionTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "skip" => Ok(ExceptionType::Skip),
            "override" => Ok(ExceptionType::Override),
            "move" => Ok(ExceptionType::Move),
            _ => Err(ParseExceptionTypeError(s.to_string())),
        }
    }
}

#[derive(Error, Debug, PartialEq)]
#[error("Invalid exception type: {0}")]
pub struct ParseExceptionTypeError(String);

/// Represents a deviation from the series pattern for a specific occurrence.
/// Used to skip, override, or move individual occurrences without affecting the series.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SeriesException {
    /// Foreign key to task_series
    #[serde(with = "uuid::serde::compact")]
    pub series_id: Uuid,
    /// Original scheduled occurrence time (UTC)
    pub occurrence_dt: DateTime<Utc>,
    /// Type of exception (skip|override|move)
    pub exception_type: ExceptionType,
    /// Reference to custom task (for override/move)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exception_task_id: Option<Uuid>,
    /// Optional explanation for the exception
    pub notes: Option<String>,
    /// Exception creation timestamp
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Data Transfer Objects (DTOs) for Series Operations
// ============================================================================

/// Data required to create a new recurring series
#[derive(Debug, Clone)]
pub struct NewSeriesData {
    /// Must exist and be non-recurring
    pub template_task_id: Uuid,
    /// Raw RRULE (will be normalized)
    pub rrule: String,
    /// Series start time
    pub dtstart: DateTime<Utc>,
    /// IANA timezone name
    pub timezone: String,
}

/// Data for modifying existing series
#[derive(Debug, Clone, Default)]
pub struct UpdateSeriesData {
    /// Update recurrence rule
    pub rrule: Option<String>,
    /// Change series start time
    pub dtstart: Option<DateTime<Utc>>,
    /// Change timezone
    pub timezone: Option<String>,
    /// Pause/resume series
    pub active: Option<bool>,
}

/// Data for creating series exceptions
#[derive(Debug, Clone)]
pub struct NewSeriesException {
    pub series_id: Uuid,
    /// Which occurrence to affect
    pub occurrence_dt: DateTime<Utc>,
    pub exception_type: ExceptionType,
    /// For override/move types
    pub exception_task_id: Option<Uuid>,
    pub notes: Option<String>,
}

/// Scope for task editing operations on recurring tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditScope {
    /// Affect only the selected occurrence
    ThisOccurrence,
    /// Update series starting from this occurrence
    ThisAndFuture,
    /// Modify entire series including past occurrences
    EntireSeries,
}

impl std::fmt::Display for EditScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditScope::ThisOccurrence => write!(f, "occurrence"),
            EditScope::ThisAndFuture => write!(f, "future"),
            EditScope::EntireSeries => write!(f, "series"),
        }
    }
}

impl FromStr for EditScope {
    type Err = ParseEditScopeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "occurrence" | "this" => Ok(EditScope::ThisOccurrence),
            "future" | "this_and_future" => Ok(EditScope::ThisAndFuture),
            "series" | "entire" | "all" => Ok(EditScope::EntireSeries),
            _ => Err(ParseEditScopeError(s.to_string())),
        }
    }
}

#[derive(Error, Debug, PartialEq)]
#[error("Invalid edit scope: {0}")]
pub struct ParseEditScopeError(String);

// ============================================================================
// Series Occurrence Models (Phase 2)
// ============================================================================

/// Represents a single occurrence within a recurring series.
/// Used by RecurrenceManager to return structured occurrence data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesOccurrence {
    /// The originally scheduled time for this occurrence (in UTC)
    pub scheduled_at: DateTime<Utc>,
    /// The effective time (may differ from scheduled_at if moved via exception)
    pub effective_at: DateTime<Utc>,
    /// Whether this occurrence is affected by an exception
    pub has_exception: bool,
    /// Type of exception if any
    pub exception_type: Option<ExceptionType>,
    /// Reference to custom task for override/move exceptions
    pub exception_task_id: Option<Uuid>,
}

impl SeriesOccurrence {
    /// Create a normal occurrence without exceptions
    pub fn normal(scheduled_at: DateTime<Utc>) -> Self {
        Self {
            scheduled_at,
            effective_at: scheduled_at,
            has_exception: false,
            exception_type: None,
            exception_task_id: None,
        }
    }

    /// Create a skipped occurrence
    pub fn skipped(scheduled_at: DateTime<Utc>) -> Self {
        Self {
            scheduled_at,
            effective_at: scheduled_at, // Not used for skipped occurrences
            has_exception: true,
            exception_type: Some(ExceptionType::Skip),
            exception_task_id: None,
        }
    }

    /// Create an override occurrence with custom task
    pub fn override_with(scheduled_at: DateTime<Utc>, exception_task_id: Uuid) -> Self {
        Self {
            scheduled_at,
            effective_at: scheduled_at,
            has_exception: true,
            exception_type: Some(ExceptionType::Override),
            exception_task_id: Some(exception_task_id),
        }
    }

    /// Create a moved occurrence with new time and custom task
    pub fn moved(scheduled_at: DateTime<Utc>, effective_at: DateTime<Utc>, exception_task_id: Uuid) -> Self {
        Self {
            scheduled_at,
            effective_at,
            has_exception: true,
            exception_type: Some(ExceptionType::Move),
            exception_task_id: Some(exception_task_id),
        }
    }

    /// Returns true if this occurrence should be visible to the user
    pub fn is_visible(&self) -> bool {
        !matches!(self.exception_type, Some(ExceptionType::Skip))
    }
}

/// Configuration for materialization behavior - core version
/// This is separate from the CLI config to allow for type differences
#[derive(Debug, Clone)]
pub struct MaterializationConfig {
    /// Default materialization window in days
    pub lookahead_days: i64,
    /// Always maintain N future instances
    pub min_upcoming_instances: usize,
    /// Limit for batch operations
    pub max_batch_size: usize,
    /// Whether to materialize missed past occurrences
    pub enable_catchup: bool,
    /// Include near-past in windows (days)
    pub materialization_grace_days: i64,
}

impl Default for MaterializationConfig {
    fn default() -> Self {
        Self {
            lookahead_days: 30,
            min_upcoming_instances: 1,
            max_batch_size: 100,
            enable_catchup: false,
            materialization_grace_days: 3,
        }
    }
}