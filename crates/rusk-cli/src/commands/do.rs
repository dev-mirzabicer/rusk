use anyhow::{anyhow, Result};
use rusk_core::error::CoreError;
use rusk_core::models::CompletionResult;
use rusk_core::repository::Repository;

use crate::cli::DoCommand;
use crate::util::resolve_task_id;

pub async fn do_task(repo: &impl Repository, command: DoCommand) -> Result<()> {
    let task_id = resolve_task_id(repo, &command.id).await?;
    let result = repo.complete_task(task_id).await;

    match result {
        Ok(CompletionResult::Single(task)) => {
            println!("Completed task: '{}'", task.name);
        }
        Ok(CompletionResult::Recurring { completed, next }) => {
            println!("Completed task: '{}'", completed.name);
            if let Some(due_at) = next.due_at {
                println!(
                    "Created recurring task '{}' for {}",
                    next.name,
                    due_at.to_rfc2822()
                );
            } else {
                println!("Created recurring task '{}'", next.name);
            }
        }
        Err(CoreError::TaskBlocked(deps)) => {
            return Err(anyhow!("Task is blocked by the following tasks: {}", deps))
        }
        Err(e) => return Err(e.into()),
    }

    Ok(())
}