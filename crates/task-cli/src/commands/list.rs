use anyhow::Result;
use task_core::repository::Repository;
use crate::cli::ListCommand;
use crate::views::table::{display_tasks, ViewTask};
use crate::config::Config;

pub async fn list_tasks(repo: &impl Repository, command: ListCommand, config: &Config) -> Result<()> {
    let filters = if command.filters.is_empty() {
        crate::filter::parse_filters(config.default_filters.clone())?
    } else {
        crate::filter::parse_filters(command.filters)?
    };

    let tasks = repo.find_tasks_with_details(&filters).await?;

    let view_tasks: Vec<ViewTask> = tasks
        .into_iter()
        .map(|t| {
            let tags = t.tags.map_or_else(Vec::new, |s| s.split(',').map(String::from).collect());
            ViewTask {
                id: t.id,
                name: t.name,
                status: t.status,
                priority: t.priority,
                due_at: t.due_at,
                project_name: t.project_name,
                tags,
                depth: t.depth as usize,
            }
        })
        .collect();

    display_tasks(&view_tasks);

    Ok(())
}