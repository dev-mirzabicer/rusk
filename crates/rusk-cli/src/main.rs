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
mod timezone;
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
    use rusk_core::recurrence::{MaterializationConfig, MaterializationManager};
    
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
        cli::Commands::Recur(command) => {
            commands::recurrence::recurrence_command(&repository, command).await
        }
    };

    if let Err(e) = result {
        handle_error(e);
    }
}

fn handle_error(err: anyhow::Error) {
    let error_style = Style::new().red().bold();
    let tip_style = Style::new().cyan();
    let example_style = Style::new().green();
    let suggestion_style = Style::new().blue();
    let cause = err.source();

    if let Some(core_error) = cause.and_then(|e| e.downcast_ref::<CoreError>()) {
        match core_error {
            CoreError::NotFound(s) => {
                eprintln!("{} {}", "Error:".style(error_style), s);
                eprintln!("{} Use partial IDs (e.g., 'abc' instead of full UUID)", "Tip:".style(tip_style));
                eprintln!("{} Run 'rusk list' to see all available tasks", "Tip:".style(tip_style));
            }
            CoreError::TaskBlocked(s) => {
                eprintln!(
                    "{} Task is blocked by dependency: {}",
                    "Error:".style(error_style),
                    s.yellow()
                );
                eprintln!("{} Complete the blocking task first, or remove the dependency", "Tip:".style(tip_style));
                eprintln!("{} Remove dependency: rusk edit <task-id> --depends-on-clear", "Example:".style(example_style));
            }
            CoreError::AmbiguousId(tasks) => {
                eprintln!("{}", "Error: Multiple tasks match that ID prefix.".style(error_style));
                eprintln!("\n{} Which task did you mean?", "Please specify:".style(suggestion_style));
                for (id, name) in tasks {
                    eprintln!("  {} {}", id.yellow().bold(), name.bright_white());
                }
                eprintln!("\n{} Use more characters from the ID to be specific", "Tip:".style(tip_style));
            }
            CoreError::InvalidInput(s) => {
                eprintln!("{} Invalid input: {}", "Error:".style(error_style), s);
                eprintln!("{} Check your command syntax and arguments", "Tip:".style(tip_style));
                eprintln!("{} Use --help with any command for detailed usage information", "Tip:".style(tip_style));
                eprintln!("{} rusk add --help", "Example:".style(example_style));
            }
            CoreError::InvalidTimezone(s) => {
                eprintln!("{} Invalid timezone: {}", "Error:".style(error_style), s);
                eprintln!("{} Use standard IANA timezone names (not abbreviations)", "Tip:".style(tip_style));
                eprintln!("{} Browse available timezones: rusk recur timezones --search <region>", "Tip:".style(tip_style));
                eprintln!("{} rusk recur timezones --search america --common", "Example:".style(example_style));
            }
            CoreError::InvalidRRule(s) => {
                eprintln!("{} Invalid recurrence rule: {}", "Error:".style(error_style), s);
                eprintln!("{} Use human-friendly shortcuts instead of raw RRULE", "Tip:".style(tip_style));
                eprintln!("{} rusk add \"Daily task\" --every weekdays --at '9:00 AM'", "Example:".style(example_style));
                eprintln!("{} For complex patterns, validate RRULE syntax first", "Tip:".style(tip_style));
            }
            CoreError::SeriesNotFound(s) => {
                eprintln!("{} Recurring series not found: {}", "Error:".style(error_style), s);
                eprintln!("{} This task may not be part of a recurring series", "Tip:".style(tip_style));
                eprintln!("{} View recurring tasks: rusk list has:recurrence", "Tip:".style(tip_style));
                eprintln!("{} Get task details: rusk recur info <task-id>", "Example:".style(example_style));
            }
            CoreError::MaterializationError(s) => {
                eprintln!("{} Materialization error: {}", "Error:".style(error_style), s);
                eprintln!("{} This indicates an issue with recurring task generation", "Tip:".style(tip_style));
                eprintln!("{} Check the recurrence rule and timezone settings", "Tip:".style(tip_style));
                eprintln!("{} rusk recur info <series-id> to inspect configuration", "Example:".style(example_style));
            }
            CoreError::InvalidException(s) => {
                eprintln!("{} Invalid series exception: {}", "Error:".style(error_style), s);
                eprintln!("{} Ensure the occurrence date exists in the series schedule", "Tip:".style(tip_style));
                eprintln!("{} Preview upcoming occurrences: rusk recur preview <series-id>", "Tip:".style(tip_style));
                eprintln!("{} Use natural language dates: 'next friday', 'tomorrow'", "Example:".style(example_style));
            }
            CoreError::CircularDependency(task, depends_on) => {
                eprintln!(
                    "{} Circular dependency detected:",
                    "Error:".style(error_style)
                );
                eprintln!("  Task '{}' cannot depend on '{}'", task.yellow(), depends_on.yellow());
                eprintln!("{} Dependencies must form a directed acyclic graph (no cycles)", "Tip:".style(tip_style));
                eprintln!("{} Remove dependency: rusk edit {} --depends-on-clear", "Fix:".style(suggestion_style), task);
            }
            CoreError::Database(e) => {
                eprintln!("{} Database error: {}", "Error:".style(error_style), e);
                eprintln!("{} This may indicate database corruption or permission issues", "Tip:".style(tip_style));
                eprintln!("{} Check file permissions on rusk.db", "Tip:".style(tip_style));
                eprintln!("{} Consider backing up and reinitializing if problems persist", "Tip:".style(tip_style));
            }
            _ => {
                eprintln!("{} {}", "Error:".style(error_style), err);
                eprintln!("{} If this error persists, please report it as a bug", "Tip:".style(tip_style));
            }
        }
    } else {
        // Handle non-Core errors with helpful context
        let error_message = err.to_string();
        eprintln!("{} {}", "Error:".style(error_style), error_message);
        
        // Provide contextual help based on error message patterns
        if error_message.contains("permission") || error_message.contains("access") {
            eprintln!("{} Check file permissions on the database file", "Tip:".style(tip_style));
        } else if error_message.contains("network") || error_message.contains("connection") {
            eprintln!("{} Check your network connection", "Tip:".style(tip_style));
        } else if error_message.contains("parse") || error_message.contains("format") {
            eprintln!("{} Check the format of your input parameters", "Tip:".style(tip_style));
        } else {
            eprintln!("{} Use --help for command usage information", "Tip:".style(tip_style));
        }
    }
}