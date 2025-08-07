use crate::parser::parse_due_date;
use anyhow::{anyhow, Result};
use task_core::models::{DueDate, Filter, TaskPriority, TaskStatus};

pub fn parse_filters(filters: Vec<String>) -> Result<Vec<Filter>> {
    let mut parsed_filters = Vec::new();

    if filters.is_empty() {
        // Default filters
        parsed_filters.push(Filter::DueDate(DueDate::Today));
        parsed_filters.push(Filter::Status(TaskStatus::Pending));
        return Ok(parsed_filters);
    }

    for filter_str in filters {
        let parts: Vec<&str> = filter_str.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid filter format: {}", filter_str));
        }

        let key = parts[0];
        let value = parts[1].to_string();

        match key {
            "status" => {
                let status: TaskStatus = serde_plain::from_str(&value)
                    .map_err(|_| anyhow!("Invalid status value: {}", value))?;
                parsed_filters.push(Filter::Status(status));
            }
            "tag" => parsed_filters.push(Filter::Tag(value)),
            "tag!" => parsed_filters.push(Filter::TagNot(value)),
            "project" => parsed_filters.push(Filter::Project(value)),
            "priority" => {
                let priority: TaskPriority = serde_plain::from_str(&value)
                    .map_err(|_| anyhow!("Invalid priority value: {}", value))?;
                parsed_filters.push(Filter::Priority(priority));
            }
            "due" => {
                let due_parts: Vec<&str> = value.splitn(2, ':').collect();
                let due_date = match due_parts[0] {
                    "today" => DueDate::Today,
                    "tomorrow" => DueDate::Tomorrow,
                    "overdue" => DueDate::Overdue,
                    "before" => {
                        if due_parts.len() != 2 {
                            return Err(anyhow!("Invalid due date format for 'before': {}", value));
                        }
                        DueDate::Before(parse_due_date(due_parts[1])?)
                    }
                    "after" => {
                        if due_parts.len() != 2 {
                            return Err(anyhow!("Invalid due date format for 'after': {}", value));
                        }
                        DueDate::After(parse_due_date(due_parts[1])?)
                    }
                    _ => return Err(anyhow!("Unknown due date filter: {}", value)),
                };
                parsed_filters.push(Filter::DueDate(due_date));
            }
            _ => return Err(anyhow!("Unknown filter key: {}", key)),
        }
    }

    Ok(parsed_filters)
}