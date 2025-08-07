use anyhow::Result;
use task_core::repository::Repository;
use uuid::Uuid;

pub async fn delete_task(repo: &impl Repository, task_id: Uuid) -> Result<()> {
    repo.delete_task(task_id).await?;
    println!("Task deleted successfully.");
    Ok(())
}