use anyhow::Result;
use dialoguer::Select;
use owo_colors::OwoColorize;
use rusk_core::models::{UpdateTaskData, EditScope};
use rusk_core::repository::{Repository, TaskRepository};

use crate::cli::EditCommand;
use crate::parser::parse_due_date;
use crate::timezone::normalize_timezone_input;
use crate::util::resolve_task_id;

pub async fn edit_task(repo: &(impl Repository + Sync), command: EditCommand) -> Result<()> {
    let task_id = resolve_task_id(repo, &command.id).await?;

    // Check if this task is part of a series and determine scope
    let task = repo.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let scope = if task.series_id.is_some() {
        // This is a recurring task, need scope
        if let Some(scope) = command.scope {
            scope
        } else if command.force_scope {
            EditScope::ThisOccurrence // Default for forced scope
        } else {
            // Interactive scope selection
            let scope_options = vec![
                format!("This occurrence only ({})", 
                    task.due_at.map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_else(|| "No due date".to_string())),
                "This and future occurrences".to_string(),
                "Entire series".to_string(),
            ];
            
            println!("{}", "This task is part of a recurring series.".yellow());
            let selection = Select::new()
                .with_prompt("How would you like to apply your changes?")
                .items(&scope_options)
                .default(0)
                .interact()?;
            
            match selection {
                0 => EditScope::ThisOccurrence,
                1 => EditScope::ThisAndFuture,
                2 => EditScope::EntireSeries,
                _ => unreachable!(),
            }
        }
    } else {
        EditScope::ThisOccurrence // Not a recurring task
    };

    let description = if command.description_clear {
        Some(None)
    } else {
        command.description.map(Some)
    };

    let due_at = if command.due_clear {
        Some(None)
    } else if let Some(due_str) = command.due {
        Some(Some(parse_due_date(&due_str, None)?))
    } else {
        None
    };

    let parent_id = if command.parent_clear {
        Some(None)
    } else if let Some(parent_str) = command.parent {
        Some(Some(resolve_task_id(repo, &parent_str).await?))
    } else {
        None
    };

    let depends_on = if command.depends_on_clear {
        Some(None)
    } else if let Some(depends_on_str) = command.depends_on {
        Some(Some(resolve_task_id(repo, &depends_on_str).await?))
    } else {
        None
    };

    let project_name = if command.project_clear {
        Some(None)
    } else {
        command.project.map(Some)
    };

    let rrule = if command.recurrence_clear {
        Some(None)
    } else {
        command.recurrence.map(Some)
    };

    let timezone = command.timezone.map(|tz| normalize_timezone_input(&tz).map(Some)).transpose()?;

    let update_data = UpdateTaskData {
        name: command.name,
        description,
        due_at,
        priority: command.priority,
        status: command.status,
        project_name,
        add_tags: if command.add_tag.is_empty() {
            None
        } else {
            Some(command.add_tag)
        },
        remove_tags: if command.remove_tag.is_empty() {
            None
        } else {
            Some(command.remove_tag)
        },
        parent_id,
        rrule,
        depends_on,
        timezone,
        series_id: None, // Not user-editable for now
    };

    let updated_task = repo.update_task(task_id, update_data, Some(scope)).await?;

    match scope {
        EditScope::ThisOccurrence => println!("Updated task with ID: {}", updated_task.id),
        EditScope::ThisAndFuture => println!("Updated series and future occurrences (template task ID: {})", updated_task.id),
        EditScope::EntireSeries => println!("Updated entire series (template task ID: {})", updated_task.id),
    }

    Ok(())
}