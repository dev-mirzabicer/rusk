use clap::{Parser, Subcommand, ValueEnum};
use rusk_core::models::{TaskPriority, TaskStatus, EditScope};

/// Rusk - A feature-rich, high-quality, robust CLI task management tool
/// 
/// Rusk is a modern task management CLI with advanced features including:
/// • Powerful recurring task support with timezone awareness
/// • Natural language date parsing ("next friday", "in 2 weeks")
/// • Advanced filtering and query system
/// • Project organization and task dependencies
/// • Exception handling for recurring tasks (skip, move, override)
/// • Series management with bulk operations
/// 
/// Get started: rusk add "My first task" --due tomorrow
#[derive(Parser, Debug)]
#[command(
    author = "Rusk Development Team", 
    version = env!("CARGO_PKG_VERSION"),
    about = "A feature-rich, high-quality, robust CLI task management tool",
    long_about = "Rusk is a modern task management CLI with advanced features including recurring task support with timezone awareness, natural language date parsing, advanced filtering, project organization, and comprehensive series management."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Add a new task with optional recurrence, dependencies, and metadata
    #[command(visible_alias = "a")]
    Add(AddCommand),
    /// List and filter tasks with powerful query syntax
    #[command(visible_alias = "ls")]
    List(ListCommand),
    /// Delete a task permanently (use --force to skip confirmation)
    #[command(visible_alias = "rm")]
    Delete(DeleteCommand),
    /// Mark a task as completed (automatically handles recurring tasks)
    #[command(visible_alias = "done")]
    Do(DoCommand),
    /// Cancel a task and mark it as cancelled
    Cancel(CancelCommand),
    /// Edit task properties with scope-aware recurring task support
    #[command(visible_alias = "e")]
    Edit(EditCommand),
    /// Manage projects (add, list, delete)
    #[command(visible_alias = "proj")]
    Project(ProjectCommand),
    /// Manage recurring task series (info, preview, skip, move, etc.)
    #[command(visible_alias = "r")]
    Recur(RecurrenceCommand),
}

/// Add a new task with comprehensive options
/// 
/// Examples:
///   rusk add "Buy groceries" --due tomorrow --project Personal
///   rusk add "Daily standup" --every weekdays --at "9:00 AM" --project Work
///   rusk add "Team meeting" --every weekly --on mon --at "2:00 PM" --until "2025-12-31"
///   rusk add "Review code" --due "next friday" --depends-on abc123 --priority high
#[derive(Parser, Debug, Clone)]
pub struct AddCommand {
    /// Task name (required)
    /// 
    /// The primary identifier for your task. Can include emojis and special characters.
    pub name: String,
    
    /// Detailed description of the task
    #[clap(short = 'D', long, help = "Optional description providing additional context")]
    pub description: Option<String>,
    
    /// When the task is due (supports natural language)
    /// 
    /// Examples: "tomorrow", "next friday", "2025-08-20", "in 2 weeks"
    #[clap(short = 'd', long, help = "Due date (natural language or YYYY-MM-DD format)")]
    pub due: Option<String>,
    
    /// Project to associate with this task
    #[clap(short, long, help = "Project name (will be created if it doesn't exist)")]
    pub project: Option<String>,
    
    /// Tags for organization and filtering
    #[clap(short, long, num_args = 1.., help = "Tags for categorization (can specify multiple)")]
    pub tag: Vec<String>,
    
    /// Task dependency (blocks this task until dependency is completed)
    #[clap(long, help = "ID of task this depends on (partial IDs accepted)")]
    pub depends_on: Option<String>,
    
    /// Task priority level
    #[clap(long, value_enum, help = "Priority level (none, low, medium, high)")]
    pub priority: Option<TaskPriority>,
    
    /// Parent task for creating subtasks
    #[clap(long, help = "Parent task ID to create a subtask (partial IDs accepted)")]
    pub parent: Option<String>,
    
    /// Advanced: Raw RFC 5545 recurrence rule
    #[clap(long, conflicts_with_all = ["every", "on"], help = "Raw RRULE for complex patterns (e.g., 'FREQ=WEEKLY;BYDAY=MO,WE,FR')")]
    pub recurrence: Option<String>,
    
    /// Simple recurrence pattern
    #[clap(long, value_enum, help = "Human-friendly recurrence (daily, weekly, monthly, yearly, weekdays, weekends)")]
    pub every: Option<RecurrenceShortcut>,
    
    /// Specific days for weekly recurrence
    #[clap(long, help = "Days for weekly recurrence (e.g., 'mon,wed,fri' or 'weekdays')")]
    pub on: Option<String>,
    
    /// Time of day for recurring tasks
    #[clap(long, help = "Time for recurrence (e.g., '9:00 AM', '14:30', '5pm')")]
    pub at: Option<String>,
    
    /// End date for finite recurrence
    #[clap(long, help = "Stop recurring after this date (e.g., '2025-12-31', 'next year')")]
    pub until: Option<String>,
    
    /// Maximum occurrences for finite recurrence
    #[clap(long, help = "Stop after this many occurrences (alternative to --until)")]
    pub count: Option<u32>,
    
    /// Timezone for recurrence calculations
    #[clap(long, help = "IANA timezone (e.g., 'America/New_York', 'Europe/London'). Run 'rusk recur timezones' to list options")]
    pub timezone: Option<String>,
}

/// Edit an existing task with scope-aware recurring task support
/// 
/// For recurring tasks, you'll be prompted to choose scope:
///   - This occurrence only: Modify just this instance
///   - This and future: Update the series from this point forward
///   - Entire series: Modify all past and future occurrences
/// 
/// Examples:
///   rusk edit abc123 --name "Updated task name"
///   rusk edit def456 --due "next week" --scope occurrence
///   rusk edit ghi789 --recurrence-clear  # Convert recurring to one-time
#[derive(Parser, Debug, Clone)]
pub struct EditCommand {
    /// Task ID to edit (partial IDs accepted)
    pub id: String,
    
    /// Skip interactive scope prompting (use default or specified scope)
    #[arg(long, help = "Force scope without interactive prompting (useful for scripts)")]
    pub force_scope: bool,

    /// Update task name
    #[arg(long, help = "New name for the task")]
    pub name: Option<String>,

    /// Update task description
    #[arg(long, help = "New description for the task")]
    pub description: Option<String>,
    /// Clear the task description
    #[arg(long, conflicts_with = "description", help = "Remove the description entirely")]
    pub description_clear: bool,

    /// Update due date
    #[arg(long, help = "New due date (natural language or YYYY-MM-DD)")]
    pub due: Option<String>,
    /// Clear the due date
    #[arg(long, conflicts_with = "due", help = "Remove the due date")]
    pub due_clear: bool,

    /// Update task dependency
    #[arg(long, help = "New dependency task ID (partial IDs accepted)")]
    pub depends_on: Option<String>,
    /// Clear task dependency
    #[arg(long, conflicts_with = "depends_on", help = "Remove dependency relationship")]
    pub depends_on_clear: bool,

    /// Update task priority
    #[arg(long, value_enum, help = "New priority level (none, low, medium, high)")]
    pub priority: Option<TaskPriority>,

    /// Update parent task
    #[arg(long, help = "New parent task ID to move this task under (partial IDs accepted)")]
    pub parent: Option<String>,
    /// Clear parent relationship
    #[arg(long, conflicts_with = "parent", help = "Remove parent relationship (move to top level)")]
    pub parent_clear: bool,

    /// How to apply changes to recurring tasks
    #[arg(long, value_enum, help = "Scope for recurring tasks: occurrence (this only), future (this and future), series (entire series)")]
    pub scope: Option<EditScope>,
    
    /// Update recurrence rule
    #[arg(long, help = "New RRULE for recurring tasks (raw RFC 5545 format)")]
    pub recurrence: Option<String>,
    /// Remove recurrence
    #[arg(long, conflicts_with = "recurrence", help = "Convert recurring task to one-time task")]
    pub recurrence_clear: bool,
    
    /// Update series timezone
    #[arg(long, help = "New timezone for recurring series (IANA format, e.g., 'America/New_York')")]
    pub timezone: Option<String>,

    /// Update task status
    #[arg(long, value_enum, help = "New status (pending, completed, cancelled)")]
    pub status: Option<TaskStatus>,

    /// Update project assignment
    #[arg(long, help = "New project name (will be created if it doesn't exist)")]
    pub project: Option<String>,
    /// Clear project assignment
    #[arg(long, conflicts_with = "project", help = "Remove from project")]
    pub project_clear: bool,

    /// Add new tags
    #[arg(long, num_args = 1.., help = "Tags to add (can specify multiple)")]
    pub add_tag: Vec<String>,

    /// Remove existing tags
    #[arg(long, num_args = 1.., help = "Tags to remove (can specify multiple)")]
    pub remove_tag: Vec<String>,
}

/// Mark a task as completed
/// 
/// For recurring tasks, this will automatically generate the next occurrence
/// based on the recurrence rule. Completed tasks remain in the database for
/// tracking and can be viewed with appropriate filters.
/// 
/// Examples:
///   rusk do abc123
///   rusk done def456  # Using alias
#[derive(Parser, Debug, Clone)]
pub struct DoCommand {
    /// Task ID to mark as completed (partial IDs accepted)
    pub id: String,
}

/// Cancel a task and mark it as cancelled
/// 
/// Cancelled tasks are distinguished from completed tasks and won't
/// generate new occurrences for recurring series.
/// 
/// Examples:
///   rusk cancel abc123
#[derive(Parser, Debug, Clone)]
pub struct CancelCommand {
    /// Task ID to cancel (partial IDs accepted)
    pub id: String,
}

/// Delete a task permanently
/// 
/// WARNING: This permanently removes the task from the database.
/// For recurring tasks, you'll be prompted about series deletion.
/// Use --force to skip confirmation prompts (useful for scripts).
/// 
/// Examples:
///   rusk delete abc123        # With confirmation
///   rusk rm def456 --force    # Skip confirmation
#[derive(Parser, Debug, Clone)]
pub struct DeleteCommand {
    /// Task ID to delete permanently (partial IDs accepted)
    pub id: String,
    /// Skip confirmation prompt (useful for automation)
    #[clap(short, long, help = "Delete without confirmation prompt")]
    pub force: bool,
}

/// List and filter tasks with powerful query syntax
/// 
/// Supports advanced filtering with logical operators:
///   - Basic: status:pending project:Work tag:urgent
///   - Logical: status:pending and (project:Work or tag:urgent)
///   - Dates: due:today due:before:friday overdue
///   - Negation: not status:completed
///   - Series: has:recurrence no:recurrence
/// 
/// Examples:
///   rusk list                           # Default view (pending tasks)
///   rusk list due:today                 # Tasks due today
///   rusk list project:Work and tag:urgent
///   rusk list "due:before:friday and not status:completed"
///   rusk ls overdue                     # Using alias
#[derive(Parser, Debug, Clone)]
pub struct ListCommand {
    /// Filter query using logical operators and field filters
    /// 
    /// Available fields: status, project, tag, due, priority, has, no
    /// Operators: and, or, not, parentheses for grouping
    /// Date filters: today, tomorrow, overdue, before:DATE, after:DATE
    #[clap(default_value = "", help = "Filter expression (empty shows default view)")]
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
/// 
/// These shortcuts make it easy to create common recurring patterns
/// without writing complex RRULE strings.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceShortcut {
    /// Every day at the same time
    Daily,
    /// Every week on the same day of the week
    Weekly,
    /// Every month on the same date (handles month-end intelligently)
    Monthly,
    /// Every year on the same date
    Yearly,
    /// Monday through Friday (work days)
    Weekdays,
    /// Saturday and Sunday (weekend days)
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
    /// Show comprehensive information about a recurring series
    #[command(visible_alias = "i")]
    Info(RecurrenceInfoCommand),
    /// Preview upcoming occurrences for planning
    #[command(visible_alias = "p")]
    Preview(RecurrencePreviewCommand),
    /// Skip a specific occurrence without affecting the series
    #[command(visible_alias = "s")]
    Skip(RecurrenceSkipCommand),
    /// Move an occurrence to a different date/time
    #[command(visible_alias = "m")]
    Move(RecurrenceMoveCommand),
    /// Pause series to stop generating new instances
    Pause(RecurrencePauseCommand),
    /// Resume a paused series
    Resume(RecurrenceResumeCommand),
    /// List all exceptions (skips, moves, overrides) for a series
    #[command(visible_alias = "ex")]
    Exceptions(RecurrenceExceptionsCommand),
    /// Create a copy of an existing series with modifications
    #[command(visible_alias = "dup")]
    Duplicate(RecurrenceDuplicateCommand),
    /// Archive a completed series (mark as inactive)
    #[command(visible_alias = "arch")]
    Archive(RecurrenceArchiveCommand),
    /// Show detailed statistics and health metrics for a series
    Stats(RecurrenceStatsCommand),
    /// Skip multiple occurrences in a date range or list
    #[command(name = "bulk-skip")]
    BulkSkip(RecurrenceBulkSkipCommand),
    /// Remove specific exceptions to restore original schedule
    #[command(name = "remove-exceptions")]
    RemoveExceptions(RecurrenceRemoveExceptionsCommand),
    /// Browse and search available timezones
    #[command(visible_alias = "tz")]
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