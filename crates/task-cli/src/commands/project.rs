use anyhow::Result;
use task_core::repository::Repository;
use crate::cli::{ProjectCommand, ProjectSubcommand, AddProjectCommand, DeleteProjectCommand};

use crate::views::table::{display_projects, ViewProject};

pub async fn project_command(repo: &impl Repository, command: ProjectCommand) -> Result<()> {
    match command.command {
        ProjectSubcommand::Add(add_command) => add_project(repo, add_command).await,
        ProjectSubcommand::List => list_projects(repo).await,
        ProjectSubcommand::Delete(delete_command) => delete_project(repo, delete_command).await,
    }
}

async fn add_project(repo: &impl Repository, command: AddProjectCommand) -> Result<()> {
    let project = repo.add_project(command.name, command.description).await?;
    println!("Added project: {}", project.name);
    Ok(())
}

async fn list_projects(repo: &impl Repository) -> Result<()> {
    let projects = repo.find_projects().await?;
    let view_projects: Vec<ViewProject> = projects
        .into_iter()
        .map(|p| ViewProject {
            id: p.id,
            name: p.name,
            description: p.description,
            created_at: p.created_at,
        })
        .collect();
    display_projects(&view_projects);
    Ok(())
}

async fn delete_project(repo: &impl Repository, command: DeleteProjectCommand) -> Result<()> {
    repo.delete_project(command.name).await?;
    println!("Project deleted.");
    Ok(())
}