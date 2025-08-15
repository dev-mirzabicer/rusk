use clap::Parser;
use dialoguer::Confirm;
use owo_colors::{OwoColorize, Style};
use rusk_core::db;
use rusk_core::error::CoreError;
use rusk_core::repository::{Repository, SqliteRepository};
use util::resolve_task_id;

mod cli;
mod commands;
mod config;
mod parser;
mod query_parser;
mod util;
mod views;

const DATABASE_URL: &str = "rusk.db";

#[tokio::main]
async fn main() {
    let config = config::Config::new().unwrap_or_else(|_| config::Config { 
        default_filters: vec![], 
        recurrence: config::MaterializationConfig::default(),
    });
    let db_pool = match db::establish_connection(DATABASE_URL).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    };
    use rusk_core::models::MaterializationConfig;
    use rusk_core::recurrence::MaterializationManager;
    
    let materialization_manager = MaterializationManager::new(MaterializationConfig::default());
    let repository = SqliteRepository::new(db_pool, materialization_manager);

    let cli = cli::Cli::parse();

    let result = match cli.command {
        cli::Commands::Add(command) => commands::add::add_task(&repository, command).await,
        cli::Commands::List(command) => {
            commands::list::list_tasks(&repository, command, &config).await
        }
        cli::Commands::Delete(command) => {
            let task_id = match resolve_task_id(&repository, &command.id).await {
                Ok(id) => id,
                Err(e) => {
                    handle_error(e.into());
                    return;
                }
            };
            let task = match repository.find_task_by_id(task_id).await {
                Ok(Some(t)) => t,
                Ok(None) => {
                    let error_style = Style::new().red().bold();
                    eprintln!("{} Task with ID '{}' not found.", "Error:".style(error_style), task_id);
                    return;
                }
                Err(e) => {
                    handle_error(e.into());
                    return;
                }
            };

            if !command.force {
                let confirmation = Confirm::new()
                    .with_prompt(format!(
                        "Are you sure you want to delete task '{}'?",
                        task.name
                    ))
                    .default(false)
                    .interact()
                    .unwrap_or(false);

                if !confirmation {
                    println!("Deletion cancelled.");
                    return;
                }
            }
            commands::delete::delete_task(&repository, task_id).await
        }
        cli::Commands::Do(command) => commands::r#do::do_task(&repository, command).await,
        cli::Commands::Cancel(command) => {
            commands::cancel::cancel_task(&repository, command).await
        }
        cli::Commands::Edit(command) => commands::edit::edit_task(&repository, command).await,
        cli::Commands::Project(command) => {
            commands::project::project_command(&repository, command).await
        }
    };

    if let Err(e) = result {
        handle_error(e);
    }
}

fn handle_error(err: anyhow::Error) {
    let error_style = Style::new().red().bold();
    let cause = err.source();

    if let Some(core_error) = cause.and_then(|e| e.downcast_ref::<CoreError>()) {
        match core_error {
            CoreError::NotFound(s) => {
                eprintln!("{} {}", "Error:".style(error_style), s);
            }
            CoreError::TaskBlocked(s) => {
                eprintln!(
                    "{} Task is blocked by: {}",
                    "Error:".style(error_style),
                    s.yellow()
                );
            }
            CoreError::AmbiguousId(tasks) => {
                eprintln!("{}", "Error: Ambiguous ID.".style(error_style));
                eprintln!("Did you mean one of these?");
                for (id, name) in tasks {
                    eprintln!("  {} ({})", id.yellow(), name);
                }
            }
            CoreError::InvalidInput(s) => {
                eprintln!("{} Invalid input: {}", "Error:".style(error_style), s);
            }
            CoreError::CircularDependency(task, depends_on) => {
                eprintln!(
                    "{} Circular dependency detected: Task '{}' cannot depend on '{}'",
                    "Error:".style(error_style),
                    task.yellow(),
                    depends_on.yellow()
                );
            }
            _ => eprintln!("{} {}", "Error:".style(error_style), err),
        }
    } else {
        eprintln!("{} {}", "Error:".style(error_style), err);
    }
}