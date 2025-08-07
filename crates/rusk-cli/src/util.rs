use anyhow::{anyhow, Result};
use rusk_core::error::CoreError;
use rusk_core::repository::Repository;
use uuid::Uuid;

pub async fn resolve_task_id(repo: &impl Repository, short_id: &str) -> Result<Uuid> {
    if short_id.len() < 2 {
        return Err(anyhow!(CoreError::InvalidInput(
            "Short ID must be at least 2 characters long.".to_string()
        )));
    }
    let tasks = repo.find_tasks_by_short_id_prefix(short_id).await?;
    if tasks.len() == 1 {
        Ok(tasks[0].id)
    } else if tasks.is_empty() {
        Err(anyhow!(CoreError::NotFound(format!(
            "No task found with ID prefix '{}'",
            short_id
        ))))
    } else {
        let task_info: Vec<(String, String)> = tasks
            .into_iter()
            .map(|t| (t.id.to_string(), t.name))
            .collect();
        Err(anyhow!(CoreError::AmbiguousId(task_info)))
    }
}