use chrono::{DateTime, Utc};
use chrono_humanize::Humanize;
use comfy_table::{Cell, Color, Row, Table};
use task_core::models::{TaskPriority, TaskStatus};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ViewTask {
    pub id: Uuid,
    pub name: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub due_at: Option<DateTime<Utc>>,
    pub project_name: Option<String>,
    pub tags: Vec<String>,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct ViewProject {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub fn display_tasks(tasks: &[ViewTask]) {
    if tasks.is_empty() {
        println!("No tasks found.");
        return;
    }

    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "Status", "Due Date", "Project", "Tags"]);

    for task in tasks {
        let mut row = Row::new();
        row.add_cell(Cell::new(&task.id.to_string()[..7]));

        let indentation = "  ".repeat(task.depth);
        let name_cell = Cell::new(format!("{}{}", indentation, &task.name));

        let name_cell = match task.status {
            TaskStatus::Completed | TaskStatus::Cancelled => name_cell.fg(Color::DarkGrey),
            TaskStatus::Pending => match task.priority {
                TaskPriority::High => name_cell.fg(Color::Red),
                TaskPriority::Medium => name_cell.fg(Color::Yellow),
                TaskPriority::Low => name_cell.fg(Color::Green),
                TaskPriority::None => name_cell,
            },
        };
        row.add_cell(name_cell);

        let status_cell = Cell::new(format!("{:?}", task.status));
        let status_cell = match task.status {
            TaskStatus::Completed => status_cell.fg(Color::Green),
            TaskStatus::Cancelled => status_cell.fg(Color::DarkGrey),
            TaskStatus::Pending => status_cell,
        };
        row.add_cell(status_cell);

        let due_date_cell = if let Some(due_at) = task.due_at {
            let humanized_due_at = due_at.humanize();
            if due_at < Utc::now() && task.status == TaskStatus::Pending {
                Cell::new(humanized_due_at).fg(Color::Red)
            } else {
                Cell::new(humanized_due_at)
            }
        } else {
            Cell::new("None")
        };
        row.add_cell(due_date_cell);

        row.add_cell(Cell::new(task.project_name.as_deref().unwrap_or("None")));
        row.add_cell(Cell::new(if task.tags.is_empty() {
            "None".to_string()
        } else {
            task.tags.join(", ")
        }));
        table.add_row(row);
    }

    println!("{table}");
}

pub fn display_projects(projects: &[ViewProject]) {
    if projects.is_empty() {
        println!("No projects found.");
        return;
    }

    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "Description", "Created At"]);

    for project in projects {
        let mut row = Row::new();
        row.add_cell(Cell::new(&project.id.to_string()));
        row.add_cell(Cell::new(&project.name));
        row.add_cell(Cell::new(project.description.as_deref().unwrap_or("None")));
        row.add_cell(Cell::new(project.created_at.humanize()));
        table.add_row(row);
    }

    println!("{table}");
}