use anyhow::Result;
use rusk_core::repository::Repository;
use crate::cli::ListCommand;
use crate::views::table::{display_tasks, ViewTask};
use crate::config::Config;
use crate::query_parser;

pub async fn list_tasks(repo: &impl Repository, command: ListCommand, config: &Config) -> Result<()> {
    let query_str = if command.query.is_empty() && !config.default_filters.is_empty() {
        config.default_filters.join(" and ")
    } else {
        command.query
    };

    let query = query_parser::parse_query(&query_str)?;

    let tasks = repo.find_tasks_with_details(&query).await?;

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