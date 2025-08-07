use anyhow::{anyhow, Result};
use task_core::repository::Repository;
use uuid::Uuid;

pub async fn resolve_task_id(repo: &impl Repository, short_id: &str) -> Result<Uuid> {
    let tasks = repo.find_tasks_by_short_id_prefix(short_id).await?;
    if tasks.len() == 1 {
        Ok(tasks[0].id)
    } else if tasks.is_empty() {
        Err(anyhow!("No task found with ID prefix '{}'", short_id))
    } else {
        let task_ids: Vec<String> = tasks.iter().map(|t| t.id.to_string()).collect();
        Err(anyhow!(
            "Ambiguous task ID prefix '{}'. Found multiple tasks: {}",
            short_id,
            task_ids.join(", ")
        ))
    }
}