use anyhow::Result;
use task_core::repository::Repository;

use crate::cli::CancelCommand;
use crate::util::resolve_task_id;

pub async fn cancel_task(repo: &impl Repository, command: CancelCommand) -> Result<()> {
    let task_id = resolve_task_id(repo, &command.id).await?;
    let task = repo.cancel_task(task_id).await?;
    println!("Cancelled task: {}", task.name);
    Ok(())
}