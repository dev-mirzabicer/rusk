use clap::{Parser, Subcommand};
use rusk_core::models::{TaskPriority, TaskStatus};

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
    /// The recurrence of the task
    #[clap(long)]
    pub recurrence: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct EditCommand {
    /// The ID of the task to edit
    pub id: String,

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

    #[arg(long)]
    pub recurrence: Option<String>,
    #[arg(long, conflicts_with = "recurrence")]
    pub recurrence_clear: bool,

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
    /// Filters to apply to the task list (e.g., "status:pending", "due:today")
    pub filters: Vec<String>,
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