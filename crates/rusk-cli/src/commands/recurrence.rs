use anyhow::Result;
use chrono::Utc;
use comfy_table::Table;
use dialoguer::Confirm;
use owo_colors::OwoColorize;
use rusk_core::models::{NewSeriesException, ExceptionType, UpdateSeriesData};
use rusk_core::recurrence::RecurrenceManager;
use rusk_core::repository::Repository;

use crate::cli::{
    RecurrenceCommand, RecurrenceSubcommand, RecurrenceInfoCommand, RecurrencePreviewCommand,
    RecurrenceSkipCommand, RecurrenceMoveCommand, RecurrencePauseCommand, 
    RecurrenceResumeCommand, RecurrenceExceptionsCommand, RecurrenceDuplicateCommand,
    RecurrenceArchiveCommand, RecurrenceStatsCommand, RecurrenceBulkSkipCommand,
    RecurrenceRemoveExceptionsCommand, RecurrenceTimezonesCommand,
};
use crate::parser::parse_due_date;
use crate::timezone::format_timezone_display;
use crate::util::resolve_task_id;

pub async fn recurrence_command<R: Repository>(
    repository: &R,
    command: RecurrenceCommand,
) -> Result<()> {
    match command.command {
        RecurrenceSubcommand::Info(cmd) => info_command(repository, cmd).await,
        RecurrenceSubcommand::Preview(cmd) => preview_command(repository, cmd).await,
        RecurrenceSubcommand::Skip(cmd) => skip_command(repository, cmd).await,
        RecurrenceSubcommand::Move(cmd) => move_command(repository, cmd).await,
        RecurrenceSubcommand::Pause(cmd) => pause_command(repository, cmd).await,
        RecurrenceSubcommand::Resume(cmd) => resume_command(repository, cmd).await,
        RecurrenceSubcommand::Exceptions(cmd) => exceptions_command(repository, cmd).await,
        // Phase 5: Advanced Series Management
        RecurrenceSubcommand::Duplicate(cmd) => duplicate_command(repository, cmd).await,
        RecurrenceSubcommand::Archive(cmd) => archive_command(repository, cmd).await,
        RecurrenceSubcommand::Stats(cmd) => stats_command(repository, cmd).await,
        RecurrenceSubcommand::BulkSkip(cmd) => bulk_skip_command(repository, cmd).await,
        RecurrenceSubcommand::RemoveExceptions(cmd) => remove_exceptions_command(repository, cmd).await,
        RecurrenceSubcommand::Timezones(cmd) => timezones_command(repository, cmd).await,
    }
}

async fn info_command<R: Repository>(
    repository: &R,
    command: RecurrenceInfoCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // First try to find task and its series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        // This is an instance task
        repository.find_series_by_id(series_id).await?
    } else {
        // This might be a template task
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    // Get template task
    let template_task = repository.find_task_by_id(series.template_task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Template task not found"))?;
    
    // Get exceptions
    let exceptions = repository.find_series_exceptions(series.id).await?;
    
    // Display series information
    println!("{}", "Series Information".blue().bold());
    println!("Series ID: {}", series.id.yellow());
    println!("Template Task: {} ({})", template_task.name.cyan(), template_task.id.yellow());
    println!("RRULE: {}", series.rrule.green());
    println!("Timezone: {}", series.timezone.magenta());
    println!("Active: {}", if series.active { "Yes".green().to_string() } else { "No".red().to_string() });
    println!("Created: {}", series.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
    
    if let Some(last_materialized) = series.last_materialized_until {
        println!("Materialized until: {}", last_materialized.format("%Y-%m-%d %H:%M:%S UTC"));
    }
    
    println!();
    
    if !exceptions.is_empty() {
        println!("{} ({} exceptions)", "Exceptions".yellow().bold(), exceptions.len());
        for exception in &exceptions {
            let type_str = match exception.exception_type {
                ExceptionType::Skip => "Skip".red().to_string(),
                ExceptionType::Override => "Override".yellow().to_string(),
                ExceptionType::Move => "Move".blue().to_string(),
            };
            println!("  {} {} - {}", 
                type_str,
                exception.occurrence_dt.format("%Y-%m-%d %H:%M"),
                exception.notes.as_deref().unwrap_or("No notes")
            );
        }
        println!();
    }
    
    // Show next few occurrences
    println!("{}", "Next 5 Occurrences".blue().bold());
    
    let recurrence_manager = RecurrenceManager::new(series.clone(), template_task.clone(), exceptions)?;
    let now = Utc::now();
    let next_occurrences = recurrence_manager.preview_occurrences(now, 5)?;
    
    if next_occurrences.is_empty() {
        println!("No upcoming occurrences (series may have ended)");
    } else {
        for (i, occurrence) in next_occurrences.iter().enumerate() {
            let formatted_time = format_timezone_display(occurrence.effective_at, &series.timezone)
                .unwrap_or_else(|_| occurrence.effective_at.format("%Y-%m-%d %H:%M:%S UTC").to_string());
            
            let status = if occurrence.has_exception {
                match occurrence.exception_type {
                    Some(ExceptionType::Skip) => " (SKIPPED)".red().to_string(),
                    Some(ExceptionType::Override) => " (OVERRIDDEN)".yellow().to_string(),
                    Some(ExceptionType::Move) => " (MOVED)".blue().to_string(),
                    None => "".to_string(),
                }
            } else {
                "".to_string()
            };
            
            println!("  {}. {}{}", i + 1, formatted_time, status);
        }
    }
    
    Ok(())
}

async fn preview_command<R: Repository>(
    repository: &R,
    command: RecurrencePreviewCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    // Get template task and exceptions
    let template_task = repository.find_task_by_id(series.template_task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Template task not found"))?;
    let exceptions = repository.find_series_exceptions(series.id).await?;
    
    // Show preview
    let recurrence_manager = RecurrenceManager::new(series.clone(), template_task.clone(), exceptions)?;
    let now = Utc::now();
    let occurrences = recurrence_manager.preview_occurrences(now, command.count)?;
    
    if occurrences.is_empty() {
        println!("No upcoming occurrences (series may have ended)");
        return Ok(());
    }
    
    println!("{} (next {} occurrences)", 
        "Series Preview".blue().bold(), 
        command.count
    );
    println!("Task: {}", template_task.name.cyan());
    println!();
    
    for (i, occurrence) in occurrences.iter().enumerate() {
        let formatted_time = format_timezone_display(occurrence.effective_at, &series.timezone)
            .unwrap_or_else(|_| occurrence.effective_at.format("%Y-%m-%d %H:%M:%S UTC").to_string());
        
        let status = if occurrence.has_exception {
            match occurrence.exception_type {
                Some(ExceptionType::Skip) => " (SKIPPED)".red().to_string(),
                Some(ExceptionType::Override) => " (OVERRIDDEN)".yellow().to_string(),
                Some(ExceptionType::Move) => " (MOVED)".blue().to_string(),
                None => "".to_string(),
            }
        } else {
            "".to_string()
        };
        
        println!("  {}. {}{}", i + 1, formatted_time, status);
    }
    
    Ok(())
}

async fn skip_command<R: Repository>(
    repository: &R,
    command: RecurrenceSkipCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    // Parse the date
    let skip_date = parse_due_date(&command.on, None)?;
    
    // Confirm action
    let confirmation = Confirm::new()
        .with_prompt(format!(
            "Skip occurrence on {}? This will hide this occurrence from your task list",
            skip_date.format("%Y-%m-%d %H:%M")
        ))
        .default(false)
        .interact()?;
    
    if !confirmation {
        println!("Skip cancelled.");
        return Ok(());
    }
    
    // Create skip exception
    let exception = NewSeriesException {
        series_id: series.id,
        occurrence_dt: skip_date,
        exception_type: ExceptionType::Skip,
        exception_task_id: None,
        notes: Some(format!("Skipped via CLI on {}", Utc::now().format("%Y-%m-%d"))),
    };
    
    repository.add_series_exception(exception).await?;
    
    println!("{} Occurrence on {} has been skipped", 
        "Success:".green().bold(),
        skip_date.format("%Y-%m-%d %H:%M")
    );
    
    Ok(())
}

async fn move_command<R: Repository>(
    repository: &R,
    command: RecurrenceMoveCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    // Parse dates
    let from_date = parse_due_date(&command.from, None)?;
    let to_date = parse_due_date(&command.to, None)?;
    
    // Get template task
    let template_task = repository.find_task_by_id(series.template_task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Template task not found"))?;
    
    // Confirm action
    let confirmation = Confirm::new()
        .with_prompt(format!(
            "Move occurrence from {} to {}?",
            from_date.format("%Y-%m-%d %H:%M"),
            to_date.format("%Y-%m-%d %H:%M")
        ))
        .default(false)
        .interact()?;
    
    if !confirmation {
        println!("Move cancelled.");
        return Ok(());
    }
    
    // Create a new task for the moved occurrence
    let moved_task_data = rusk_core::models::NewTaskData {
        name: template_task.name.clone(),
        description: template_task.description.clone(),
        due_at: Some(to_date),
        priority: Some(template_task.priority.clone()),
        project_id: template_task.project_id,
        tags: vec![], // We'll need to fetch and copy tags
        parent_id: template_task.parent_id,
        depends_on: None,
        rrule: None,
        series_id: None, // This is a standalone moved task
        timezone: None,
        project_name: None,
    };
    
    let moved_task = repository.add_task(moved_task_data).await?;
    
    // Create move exception
    let exception = NewSeriesException {
        series_id: series.id,
        occurrence_dt: from_date,
        exception_type: ExceptionType::Move,
        exception_task_id: Some(moved_task.id),
        notes: Some(format!("Moved to {} via CLI on {}", 
            to_date.format("%Y-%m-%d %H:%M"),
            Utc::now().format("%Y-%m-%d")
        )),
    };
    
    repository.add_series_exception(exception).await?;
    
    println!("{} Occurrence moved from {} to {} (Task ID: {})", 
        "Success:".green().bold(),
        from_date.format("%Y-%m-%d %H:%M"),
        to_date.format("%Y-%m-%d %H:%M"),
        moved_task.id.yellow()
    );
    
    Ok(())
}

async fn pause_command<R: Repository>(
    repository: &R,
    command: RecurrencePauseCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    if !series.active {
        println!("{} Series is already paused", "Info:".yellow().bold());
        return Ok(());
    }
    
    // Confirm action
    let confirmation = Confirm::new()
        .with_prompt("Pause this series? No new instances will be created until resumed")
        .default(false)
        .interact()?;
    
    if !confirmation {
        println!("Pause cancelled.");
        return Ok(());
    }
    
    // Update series to inactive
    let update_data = UpdateSeriesData {
        active: Some(false),
        ..Default::default()
    };
    
    repository.update_series(series.id, update_data).await?;
    
    println!("{} Series has been paused", "Success:".green().bold());
    
    Ok(())
}

async fn resume_command<R: Repository>(
    repository: &R,
    command: RecurrenceResumeCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    if series.active {
        println!("{} Series is already active", "Info:".yellow().bold());
        return Ok(());
    }
    
    // Confirm action
    let confirmation = Confirm::new()
        .with_prompt("Resume this series? New instances will be created automatically")
        .default(true)
        .interact()?;
    
    if !confirmation {
        println!("Resume cancelled.");
        return Ok(());
    }
    
    // Update series to active
    let update_data = UpdateSeriesData {
        active: Some(true),
        ..Default::default()
    };
    
    repository.update_series(series.id, update_data).await?;
    
    println!("{} Series has been resumed", "Success:".green().bold());
    
    Ok(())
}

async fn exceptions_command<R: Repository>(
    repository: &R,
    command: RecurrenceExceptionsCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    // Get exceptions
    let exceptions = repository.find_series_exceptions(series.id).await?;
    
    if exceptions.is_empty() {
        println!("No exceptions found for this series");
        return Ok(());
    }
    
    // Display exceptions in a table
    let mut table = Table::new();
    table
        .set_header(vec!["Type", "Date/Time", "Notes", "Created"])
        .load_preset(comfy_table::presets::UTF8_FULL);
    
    for exception in exceptions {
        let type_str = match exception.exception_type {
            ExceptionType::Skip => "Skip".to_string(),
            ExceptionType::Override => "Override".to_string(),
            ExceptionType::Move => "Move".to_string(),
        };
        
        let formatted_time = format_timezone_display(exception.occurrence_dt, &series.timezone)
            .unwrap_or_else(|_| exception.occurrence_dt.format("%Y-%m-%d %H:%M").to_string());
        
        table.add_row(vec![
            type_str,
            formatted_time,
            exception.notes.unwrap_or_else(|| "No notes".to_string()),
            exception.created_at.format("%Y-%m-%d").to_string(),
        ]);
    }
    
    println!("{}", table);
    
    Ok(())
}

// ========== Phase 5: Advanced Series Management Commands ==========

async fn duplicate_command<R: Repository>(
    repository: &R,
    command: RecurrenceDuplicateCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    // Confirm action
    let confirmation = Confirm::new()
        .with_prompt(format!("Duplicate series '{}' with new name '{}'?", task.name, command.name))
        .default(true)
        .interact()?;
    
    if !confirmation {
        println!("Duplication cancelled.");
        return Ok(());
    }
    
    // Duplicate the series
    let new_series = repository.duplicate_series(series.id, command.name.clone(), command.timezone).await?;
    
    println!("{} Series duplicated successfully", "Success:".green().bold());
    println!("Original Series ID: {}", series.id.yellow());
    println!("New Series ID: {}", new_series.id.yellow());
    println!("New Template Task: {}", command.name.cyan());
    
    Ok(())
}

async fn archive_command<R: Repository>(
    repository: &R,
    command: RecurrenceArchiveCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    if !command.force {
        // Check if all tasks are completed
        let stats = repository.get_series_statistics(series.id).await?;
        if stats.pending_occurrences > 0 {
            println!("{} Series has {} pending tasks", "Warning:".yellow().bold(), stats.pending_occurrences);
            println!("Use --force to archive anyway, or complete/cancel the pending tasks first.");
            return Ok(());
        }
    }
    
    // Confirm action
    let confirmation = Confirm::new()
        .with_prompt("Archive this series? It will be set to inactive and stop generating new instances")
        .default(false)
        .interact()?;
    
    if !confirmation {
        println!("Archive cancelled.");
        return Ok(());
    }
    
    // Archive the series
    repository.archive_completed_series(series.id).await?;
    
    println!("{} Series has been archived", "Success:".green().bold());
    
    Ok(())
}

async fn stats_command<R: Repository>(
    repository: &R,
    command: RecurrenceStatsCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    // Get statistics
    let stats = repository.get_series_statistics(series.id).await?;
    
    println!("{}", "Series Statistics".blue().bold());
    println!("Series ID: {}", stats.series_id.yellow());
    println!("Template Task: {}", task.name.cyan());
    println!();
    
    println!("{}", "Occurrence Statistics:".blue());
    println!("  Total Created: {}", stats.total_occurrences_created);
    println!("  Completed: {} ({:.1}%)", 
        stats.completed_occurrences,
        if stats.total_occurrences_created > 0 {
            (stats.completed_occurrences as f64 / stats.total_occurrences_created as f64) * 100.0
        } else { 0.0 }
    );
    println!("  Pending: {}", stats.pending_occurrences);
    println!("  Cancelled: {}", stats.cancelled_occurrences);
    println!();
    
    println!("{}", "Exception Statistics:".blue());
    println!("  Total Exceptions: {}", stats.total_exceptions);
    println!("  Skipped: {}", stats.skip_exceptions);
    println!("  Overridden: {}", stats.override_exceptions);
    println!("  Moved: {}", stats.move_exceptions);
    println!();
    
    println!("{}", "Timeline:".blue());
    if let Some(first) = stats.first_occurrence {
        println!("  First Occurrence: {}", first.format("%Y-%m-%d %H:%M UTC"));
    }
    if let Some(last) = stats.last_occurrence {
        println!("  Last Occurrence: {}", last.format("%Y-%m-%d %H:%M UTC"));
    }
    if let Some(next) = stats.next_occurrence {
        println!("  Next Occurrence: {}", next.format("%Y-%m-%d %H:%M UTC"));
    } else {
        println!("  Next Occurrence: None (series ended or paused)");
    }
    println!();
    
    println!("{}", "Health Score:".blue());
    let health_color = if stats.series_health_score >= 0.8 {
        "green"
    } else if stats.series_health_score >= 0.6 {
        "yellow"
    } else {
        "red"
    };
    println!("  {:.1}% {}", stats.series_health_score * 100.0, 
        match health_color {
            "green" => "Excellent".green().to_string(),
            "yellow" => "Good".yellow().to_string(),
            _ => "Needs Attention".red().to_string(),
        }
    );
    
    Ok(())
}

async fn bulk_skip_command<R: Repository>(
    repository: &R,
    command: RecurrenceBulkSkipCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    let mut dates_to_skip = Vec::new();
    
    if let (Some(from), Some(to)) = (&command.from, &command.to) {
        // Range-based skipping
        let from_date = parse_due_date(from, None)?;
        let to_date = parse_due_date(to, None)?;
        
        println!("Finding occurrences between {} and {}...", 
            from_date.format("%Y-%m-%d"), 
            to_date.format("%Y-%m-%d")
        );
        
        // Get template task and exceptions for RecurrenceManager
        let template_task = repository.find_task_by_id(series.template_task_id).await?
            .ok_or_else(|| anyhow::anyhow!("Template task not found"))?;
        let exceptions = repository.find_series_exceptions(series.id).await?;
        
        let recurrence_manager = RecurrenceManager::new(series.clone(), template_task, exceptions)?;
        let occurrences = recurrence_manager.generate_occurrences_between(from_date, to_date)?;
        
        for occurrence in occurrences {
            if occurrence.is_visible() {
                dates_to_skip.push(occurrence.scheduled_at);
            }
        }
    } else {
        // Individual dates
        for date_str in command.dates.split(',') {
            let date_str = date_str.trim();
            if !date_str.is_empty() {
                let date = parse_due_date(date_str, None)?;
                dates_to_skip.push(date);
            }
        }
    }
    
    if dates_to_skip.is_empty() {
        println!("No valid dates to skip found.");
        return Ok(());
    }
    
    // Confirm action
    let confirmation = Confirm::new()
        .with_prompt(format!("Skip {} occurrence(s)?", dates_to_skip.len()))
        .default(false)
        .interact()?;
    
    if !confirmation {
        println!("Bulk skip cancelled.");
        return Ok(());
    }
    
    // Create bulk exceptions
    let exceptions: Vec<NewSeriesException> = dates_to_skip.into_iter().map(|date| {
        NewSeriesException {
            series_id: series.id,
            occurrence_dt: date,
            exception_type: ExceptionType::Skip,
            exception_task_id: None,
            notes: Some(format!("Bulk skipped via CLI on {}", Utc::now().format("%Y-%m-%d"))),
        }
    }).collect();
    
    let created = repository.add_bulk_series_exceptions(exceptions).await?;
    
    println!("{} Successfully skipped {} occurrence(s)", 
        "Success:".green().bold(),
        created.len()
    );
    
    Ok(())
}

async fn remove_exceptions_command<R: Repository>(
    repository: &R,
    command: RecurrenceRemoveExceptionsCommand,
) -> Result<()> {
    let task_id = resolve_task_id(repository, &command.id).await?;
    
    // Find series
    let task = repository.find_task_by_id(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    
    let series = if let Some(series_id) = task.series_id {
        repository.find_series_by_id(series_id).await?
    } else {
        repository.find_series_by_template(task_id).await?
    };
    
    let series = series.ok_or_else(|| anyhow::anyhow!("No recurring series found for this task"))?;
    
    let mut dates_to_remove = Vec::new();
    
    if command.all {
        // Remove all exceptions
        let exceptions = repository.find_series_exceptions(series.id).await?;
        for exception in exceptions {
            dates_to_remove.push(exception.occurrence_dt);
        }
    } else if let Some(dates_str) = &command.dates {
        // Remove specific dates
        for date_str in dates_str.split(',') {
            let date_str = date_str.trim();
            if !date_str.is_empty() {
                let date = parse_due_date(date_str, None)?;
                dates_to_remove.push(date);
            }
        }
    } else {
        return Err(anyhow::anyhow!("Either --all or --dates must be specified"));
    }
    
    if dates_to_remove.is_empty() {
        println!("No exceptions to remove.");
        return Ok(());
    }
    
    // Confirm action
    let confirmation = Confirm::new()
        .with_prompt(format!("Remove {} exception(s)?", dates_to_remove.len()))
        .default(false)
        .interact()?;
    
    if !confirmation {
        println!("Remove exceptions cancelled.");
        return Ok(());
    }
    
    // Remove exceptions
    let removed_count = repository.remove_bulk_series_exceptions(series.id, dates_to_remove).await?;
    
    println!("{} Successfully removed {} exception(s)", 
        "Success:".green().bold(),
        removed_count
    );
    
    Ok(())
}

async fn timezones_command<R: Repository>(
    _repository: &R,
    command: RecurrenceTimezonesCommand,
) -> Result<()> {
    use crate::timezone::{get_common_timezones, get_all_timezones, suggest_timezone, get_timezone_info, timezone_observes_dst};
    
    let mut timezones = if command.common {
        get_common_timezones()
    } else {
        get_all_timezones()
    };
    
    // Filter by search pattern if provided
    if let Some(search) = &command.search {
        let search_lower = search.to_lowercase();
        timezones.retain(|tz| tz.to_lowercase().contains(&search_lower));
    }
    
    if timezones.is_empty() {
        println!("No timezones found matching the criteria.");
        if let Some(search) = &command.search {
            let suggestions = suggest_timezone(search);
            if !suggestions.is_empty() {
                println!("Did you mean one of these?");
                for suggestion in suggestions {
                    println!("  {}", suggestion.cyan());
                }
            }
        }
        return Ok(());
    }
    
    println!("{}", "Available Timezones".blue().bold());
    if command.common {
        println!("(Showing common timezones only. Use without --common for full list)");
    }
    println!();
    
    if command.detailed {
        // Show detailed information including current time and DST status
        println!("{:<30} {:<20} {:<8} {:<5} {}", 
            "Timezone".blue(), 
            "Current Time".blue(), 
            "Offset".blue(), 
            "DST".blue(),
            "Abbr".blue()
        );
        println!("{}", "─".repeat(75).blue());
        
        for tz in timezones {
            match get_timezone_info(tz) {
                Ok(info) => {
                    let dst_indicator = if info.observes_dst {
                        if timezone_observes_dst(tz).unwrap_or(false) {
                            "Yes".green().to_string()
                        } else {
                            "No".yellow().to_string()
                        }
                    } else {
                        "N/A".blue().to_string()
                    };
                    
                    println!("{:<30} {:<20} {:<8} {:<5} {}", 
                        info.name.cyan(),
                        info.current_time.white(),
                        info.offset.yellow(),
                        dst_indicator,
                        info.abbreviation.green()
                    );
                }
                Err(_) => println!("{:<30} {}", tz.cyan(), "Invalid timezone".red()),
            }
        }
    } else {
        // Simple list with basic info
        for tz in timezones {
            if let Ok(info) = get_timezone_info(tz) {
                println!("  {} ({})", tz.cyan(), info.abbreviation.green());
            } else {
                println!("  {}", tz.cyan());
            }
        }
    }
    
    println!();
    println!("{}", "Tip: Use --detailed for more information, --search <pattern> to filter".blue());
    
    Ok(())
}