use anyhow::Result;
use rusk_core::models::NewTaskData;
use rusk_core::repository::Repository;
use crate::cli::AddCommand;
use crate::parser::parse_due_date;
use uuid::Uuid;

pub async fn add_task(repo: &impl Repository, command: AddCommand) -> Result<()> {
    let due_at = command.due.map(|d| parse_due_date(&d)).transpose()?;
    let depends_on = command
        .depends_on
        .map(|d| d.parse::<Uuid>())
        .transpose()?;

    let new_task_data = NewTaskData {
        name: command.name,
        description: command.description,
        due_at,
        priority: command.priority,
        project_name: command.project,
        project_id: None,
        tags: command.tag,
        parent_id: command.parent.map(|p| p.parse()).transpose()?,
        rrule: command.recurrence,
        depends_on,
        recurrence_template_id: None,
    };

    let added_task = repo.add_task(new_task_data).await?;

    println!("Added task with ID: {}", added_task.id);

    Ok(())
}