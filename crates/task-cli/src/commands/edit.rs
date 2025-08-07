use anyhow::Result;
use async_trait::async_trait;
use std::future::Future;
use task_core::models::UpdateTaskData;
use task_core::repository::Repository;

use crate::cli::EditCommand;
use crate::parser::parse_due_date;
use crate::util::resolve_task_id;

#[async_trait]
trait Noneable<T> {
    async fn to_noneable<F, Fut>(self, f: F) -> Result<Option<Option<T>>>
    where
        F: Fn(String) -> Fut + Send + Sync,
        Fut: Future<Output = Result<T>> + Send;
}

#[async_trait]
impl<T: Send> Noneable<T> for Option<String> {
    async fn to_noneable<F, Fut>(self, f: F) -> Result<Option<Option<T>>>
    where
        F: Fn(String) -> Fut + Send + Sync,
        Fut: Future<Output = Result<T>> + Send,
    {
        match self {
            Some(s) if s.to_lowercase() == "none" => Ok(Some(None)),
            Some(s) => Ok(Some(Some(f(s).await?))),
            None => Ok(None),
        }
    }
}

pub async fn edit_task(repo: &(impl Repository + Sync), command: EditCommand) -> Result<()> {
    let task_id = resolve_task_id(repo, &command.id).await?;

    let due_at = command
        .due
        .to_noneable(|s| async move { parse_due_date(&s) })
        .await?;

    let parent_id = command
        .parent
        .to_noneable(|s| async move { resolve_task_id(repo, &s).await })
        .await?;

    let depends_on = command
        .depends_on
        .to_noneable(|s| async move { resolve_task_id(repo, &s).await })
        .await?;

    let project_name = command
        .project
        .to_noneable(|s| async move { Ok(s) })
        .await?;

    let description = command
        .description
        .to_noneable(|s| async move { Ok(s) })
        .await?;

    let rrule = command
        .recurrence
        .to_noneable(|s| async move { Ok(s) })
        .await?;

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
        recurrence_template_id: None, // Not user-editable for now
    };

    let updated_task = repo.update_task(task_id, update_data).await?;

    println!("Updated task with ID: {}", updated_task.id);

    Ok(())
}