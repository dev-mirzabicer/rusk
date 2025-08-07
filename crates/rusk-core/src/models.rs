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
    pub rrule: Option<String>,
    pub recurrence_template_id: Option<Uuid>,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
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
            rrule: None,
            recurrence_template_id: None,
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
    pub rrule: Option<String>,
    pub depends_on: Option<Uuid>,
    pub recurrence_template_id: Option<Uuid>,
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
    pub rrule: Option<Option<String>>,
    pub depends_on: Option<Option<Uuid>>,
    pub recurrence_template_id: Option<Option<Uuid>>,
}

#[derive(Debug)]
pub enum CompletionResult {
    Single(Task),
    Recurring { completed: Task, next: Task },
}