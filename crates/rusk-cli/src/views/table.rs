use chrono::{DateTime, Utc};
use chrono_humanize::Humanize;
use comfy_table::{Attribute, Cell, Color, Row, Table};
use rusk_core::models::{TaskPriority, TaskStatus};
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
    pub series_id: Option<Uuid>,
    pub is_template: bool,
    pub has_exceptions: bool,
    pub timezone: Option<String>,
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
        
        // Build name with visual indicators
        let mut display_name = String::new();
        display_name.push_str(&indentation);
        
        // Add series indicator
        if task.series_id.is_some() {
            display_name.push('↻'); // Recurring symbol
            display_name.push(' ');
        }
        
        display_name.push_str(&task.name);
        
        // Add template badge
        if task.is_template {
            display_name.push_str(" (Template)");
        }
        
        // Add exception indicator
        if task.has_exceptions {
            display_name.push_str(" ⚠");
        }
        
        let mut name_cell = Cell::new(display_name);

        // Style based on status and priority
        match task.status {
            TaskStatus::Completed | TaskStatus::Cancelled => {
                name_cell = name_cell
                    .add_attribute(Attribute::CrossedOut)
                    .fg(Color::DarkGrey);
            }
            TaskStatus::Pending => {
                name_cell = match task.priority {
                    TaskPriority::High => name_cell.fg(Color::Red).add_attribute(Attribute::Bold),
                    TaskPriority::Medium => name_cell.fg(Color::Yellow),
                    TaskPriority::Low => name_cell.fg(Color::Green),
                    TaskPriority::None => name_cell,
                };
            }
        };
        row.add_cell(name_cell);

        let mut status_cell = Cell::new(format!("{:?}", task.status));
        status_cell = match task.status {
            TaskStatus::Completed => status_cell.fg(Color::Green),
            TaskStatus::Cancelled => status_cell.fg(Color::DarkGrey),
            TaskStatus::Pending => status_cell,
        };
        row.add_cell(status_cell);

        let due_date_cell = if let Some(due_at) = task.due_at {
            let now = Utc::now();
            let today = now.date_naive();
            let due_date = due_at.date_naive();

            let mut due_text = due_at.humanize();
            
            // Add timezone abbreviation for recurring tasks
            if task.series_id.is_some() && task.timezone.is_some() {
                // For now, just show the timezone name
                // In a full implementation, we'd convert to local time and show abbreviation
                due_text = format!("{}", due_text);
            }
            
            if task.status == TaskStatus::Pending {
                if due_at < now {
                    Cell::new(due_text).fg(Color::Red) // Overdue
                } else if due_date == today {
                    Cell::new(due_text).fg(Color::Yellow) // Due today
                } else {
                    Cell::new(due_text)
                }
            } else {
                Cell::new(due_text)
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