use clap::{Parser, Subcommand, ValueEnum};
use rusk_core::models::{TaskPriority, TaskStatus, EditScope};

/// A feature-rich, high-quality, robust CLI rusk management tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Add a new task
    Add(AddCommand),
    /// List tasks
    List(ListCommand),
    /// Delete a task
    Delete(DeleteCommand),
    /// Mark a task as completed
    Do(DoCommand),
    /// Cancel a task
    Cancel(CancelCommand),
    /// Edit a task
    Edit(EditCommand),
    /// Manage projects
    Project(ProjectCommand),
    /// Manage recurring task series
    Recur(RecurrenceCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct AddCommand {
    /// The name of the task
    pub name: String,
    /// The description of the task
    #[clap(short, long)]
    pub description: Option<String>,
    /// The due date of the task
    #[clap(short, long)]
    pub due: Option<String>,
    /// The project of the task
    #[clap(short, long)]
    pub project: Option<String>,
    /// Tags to add to the task
    #[clap(short, long, num_args = 1..)]
    pub tag: Vec<String>,
    /// The ID of a task that this task depends on
    #[clap(long)]
    pub depends_on: Option<String>,
    /// The priority of the task
    #[clap(long, value_enum)]
    pub priority: Option<TaskPriority>,
    /// The parent task ID
    #[clap(long)]
    pub parent: Option<String>,
    /// The recurrence of the task (raw RRULE)
    #[clap(long, conflicts_with_all = ["every", "on"], help = "Raw RFC 5545 recurrence rule")]
    pub recurrence: Option<String>,
    /// Human-friendly recurrence frequency
    #[clap(long, value_enum, help = "Human-friendly frequency (daily, weekly, monthly, etc.)")]
    pub every: Option<RecurrenceShortcut>,
    /// Days of week for weekly recurrence
    #[clap(long, help = "Days of week (mon,tue,wed,thu,fri,sat,sun)")]
    pub on: Option<String>,
    /// Time of day for recurrence
    #[clap(long, help = "Time of day for recurrence (e.g., '9:00 AM', '14:30')")]
    pub at: Option<String>,
    /// End date for recurrence
    #[clap(long, help = "End date for recurrence (e.g., '2025-12-31')")]
    pub until: Option<String>,
    /// Maximum number of occurrences
    #[clap(long, help = "Maximum number of occurrences")]
    pub count: Option<u32>,
    /// Timezone for recurrence
    #[clap(long, help = "Timezone for recurrence (IANA format, e.g., 'America/New_York')")]
    pub timezone: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct EditCommand {
    /// The ID of the task to edit
    pub id: String,
    
    /// Force scope without prompting (for scripting)
    #[arg(long, help = "Force scope without interactive prompting")]
    pub force_scope: bool,

    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub description: Option<String>,
    #[arg(long, conflicts_with = "description")]
    pub description_clear: bool,

    #[arg(long)]
    pub due: Option<String>,
    #[arg(long, conflicts_with = "due")]
    pub due_clear: bool,

    #[arg(long)]
    pub depends_on: Option<String>,
    #[arg(long, conflicts_with = "depends_on")]
    pub depends_on_clear: bool,

    #[arg(long, value_enum)]
    pub priority: Option<TaskPriority>,

    #[arg(long)]
    pub parent: Option<String>,
    #[arg(long, conflicts_with = "parent")]
    pub parent_clear: bool,

    /// How to apply changes (ask|occurrence|future|series)
    #[arg(long, value_enum, help = "How to apply changes to recurring tasks")]
    pub scope: Option<EditScope>,
    
    #[arg(long, help = "Update recurrence rule (raw RRULE)")]
    pub recurrence: Option<String>,
    #[arg(long, conflicts_with = "recurrence", help = "Remove recurrence (convert to one-time task)")]
    pub recurrence_clear: bool,
    
    /// Update series timezone
    #[arg(long, help = "Update series timezone (IANA format)")]
    pub timezone: Option<String>,

    #[arg(long, value_enum)]
    pub status: Option<TaskStatus>,

    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, conflicts_with = "project")]
    pub project_clear: bool,

    /// Add tags to the task
    #[arg(long, num_args = 1..)]
    pub add_tag: Vec<String>,

    /// Remove tags from the task
    #[arg(long, num_args = 1..)]
    pub remove_tag: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct DoCommand {
    /// The ID of the task to mark as completed
    pub id: String,
}

#[derive(Parser, Debug, Clone)]
pub struct CancelCommand {
    /// The ID of the task to cancel
    pub id: String,
}

#[derive(Parser, Debug, Clone)]
pub struct DeleteCommand {
    /// The ID of the task to delete
    pub id: String,
    /// Force deletion without confirmation
    #[clap(short, long)]
    pub force: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct ListCommand {
    /// A filter query string (e.g., "status:pending and (project:Work or tag:urgent)")
    #[clap(default_value = "")]
    pub query: String,
}

#[derive(Parser, Debug, Clone)]
pub struct ProjectCommand {
    #[command(subcommand)]
    pub command: ProjectSubcommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProjectSubcommand {
    /// Add a new project
    Add(AddProjectCommand),
    /// List projects
    List,
    /// Delete a project
    Delete(DeleteProjectCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct AddProjectCommand {
    /// The name of the project
    pub name: String,

    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct DeleteProjectCommand {
    /// The name of the project to delete
    pub name: String,
}

/// Human-friendly recurrence patterns
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceShortcut {
    /// Every day
    Daily,
    /// Every week (same day)
    Weekly,
    /// Every month (same date)
    Monthly,
    /// Every year (same date)
    Yearly,
    /// Monday to Friday
    Weekdays,
    /// Saturday and Sunday
    Weekends,
}

impl std::fmt::Display for RecurrenceShortcut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecurrenceShortcut::Daily => write!(f, "daily"),
            RecurrenceShortcut::Weekly => write!(f, "weekly"),
            RecurrenceShortcut::Monthly => write!(f, "monthly"),
            RecurrenceShortcut::Yearly => write!(f, "yearly"),
            RecurrenceShortcut::Weekdays => write!(f, "weekdays"),
            RecurrenceShortcut::Weekends => write!(f, "weekends"),
        }
    }
}

impl RecurrenceShortcut {
    /// Convert shortcut to RRULE pattern
    pub fn to_rrule(&self, _dtstart: chrono::DateTime<chrono::Utc>) -> String {
        match self {
            RecurrenceShortcut::Daily => "FREQ=DAILY".to_string(),
            RecurrenceShortcut::Weekly => "FREQ=WEEKLY".to_string(),
            RecurrenceShortcut::Monthly => "FREQ=MONTHLY".to_string(),
            RecurrenceShortcut::Yearly => "FREQ=YEARLY".to_string(),
            RecurrenceShortcut::Weekdays => "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR".to_string(),
            RecurrenceShortcut::Weekends => "FREQ=WEEKLY;BYDAY=SA,SU".to_string(),
        }
    }
}

/// Recurrence management commands
#[derive(Parser, Debug, Clone)]
pub struct RecurrenceCommand {
    #[command(subcommand)]
    pub command: RecurrenceSubcommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RecurrenceSubcommand {
    /// Show series information and upcoming occurrences
    Info(RecurrenceInfoCommand),
    /// Show next N occurrences for series
    Preview(RecurrencePreviewCommand),
    /// Skip specific occurrence
    Skip(RecurrenceSkipCommand),
    /// Move occurrence to different time
    Move(RecurrenceMoveCommand),
    /// Pause series (stop generating new instances)
    Pause(RecurrencePauseCommand),
    /// Resume paused series
    Resume(RecurrenceResumeCommand),
    /// List all exceptions for series
    Exceptions(RecurrenceExceptionsCommand),
    /// Duplicate an existing series with a new name
    Duplicate(RecurrenceDuplicateCommand),
    /// Archive a completed series
    Archive(RecurrenceArchiveCommand),
    /// Get detailed statistics for a series
    Stats(RecurrenceStatsCommand),
    /// Bulk skip multiple occurrences
    BulkSkip(RecurrenceBulkSkipCommand),
    /// Remove exceptions from a series
    RemoveExceptions(RecurrenceRemoveExceptionsCommand),
    /// List available timezones
    Timezones(RecurrenceTimezonesCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceInfoCommand {
    /// Series ID or template task ID
    pub id: String,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrencePreviewCommand {
    /// Series ID or template task ID
    pub id: String,
    /// Number of occurrences to show
    #[clap(long, short, default_value = "10")]
    pub count: usize,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceSkipCommand {
    /// Series ID or template task ID
    pub id: String,
    /// Date to skip (e.g., '2025-08-20', 'next friday')
    #[clap(long)]
    pub on: String,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceMoveCommand {
    /// Series ID or template task ID
    pub id: String,
    /// Original date/time to move
    #[clap(long)]
    pub from: String,
    /// New date/time
    #[clap(long)]
    pub to: String,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrencePauseCommand {
    /// Series ID or template task ID
    pub id: String,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceResumeCommand {
    /// Series ID or template task ID
    pub id: String,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceExceptionsCommand {
    /// Series ID or template task ID
    pub id: String,
}

// ========== Phase 5: Advanced Series Management Commands ==========

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceDuplicateCommand {
    /// Series ID or template task ID to duplicate
    pub id: String,
    /// New name for the duplicated series
    pub name: String,
    /// Timezone for the new series (optional)
    #[clap(long)]
    pub timezone: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceArchiveCommand {
    /// Series ID or template task ID to archive
    pub id: String,
    /// Force archival even if not all tasks are completed
    #[clap(long, short)]
    pub force: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceStatsCommand {
    /// Series ID or template task ID
    pub id: String,
    /// Show detailed breakdown
    #[clap(long)]
    pub detailed: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceBulkSkipCommand {
    /// Series ID or template task ID
    pub id: String,
    /// Dates to skip (comma-separated, e.g., '2025-08-20,2025-08-27')
    #[clap(long)]
    pub dates: String,
    /// Skip all occurrences in a date range
    #[clap(long)]
    pub from: Option<String>,
    /// End date for range skip
    #[clap(long)]
    pub to: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceRemoveExceptionsCommand {
    /// Series ID or template task ID
    pub id: String,
    /// Specific dates to remove exceptions for (comma-separated)
    #[clap(long)]
    pub dates: Option<String>,
    /// Remove all exceptions for the series
    #[clap(long)]
    pub all: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct RecurrenceTimezonesCommand {
    /// Search pattern for timezone names
    #[clap(long)]
    pub search: Option<String>,
    /// Show common timezones only
    #[clap(long)]
    pub common: bool,
    /// Show timezone details including DST info
    #[clap(long)]
    pub detailed: bool,
}