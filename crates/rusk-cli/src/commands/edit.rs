use anyhow::Result;
use rusk_core::models::UpdateTaskData;
use rusk_core::repository::Repository;

use crate::cli::EditCommand;
use crate::parser::parse_due_date;
use crate::util::resolve_task_id;

pub async fn edit_task(repo: &(impl Repository + Sync), command: EditCommand) -> Result<()> {
    let task_id = resolve_task_id(repo, &command.id).await?;

    let description = if command.description_clear {
        Some(None)
    } else {
        command.description.map(Some)
    };

    let due_at = if command.due_clear {
        Some(None)
    } else if let Some(due_str) = command.due {
        Some(Some(parse_due_date(&due_str)?))
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
        timezone: None, // TODO: Get from config or command  
        series_id: None, // Not user-editable for now
    };

    let updated_task = repo.update_task(task_id, update_data, None).await?;

    println!("Updated task with ID: {}", updated_task.id);

    Ok(())
}