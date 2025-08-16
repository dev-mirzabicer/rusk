use anyhow::Result;
use chrono::{DateTime, Utc, Timelike};
use rusk_core::models::NewTaskData;
use rusk_core::repository::Repository;
use crate::cli::{AddCommand, RecurrenceShortcut};
use crate::parser::parse_due_date;
use crate::timezone::{detect_system_timezone, normalize_timezone_input};
use uuid::Uuid;

pub async fn add_task(repo: &impl Repository, command: AddCommand) -> Result<()> {
    let due_at = command.due.as_ref().map(|d| parse_due_date(d, None)).transpose()?;
    let depends_on = command
        .depends_on
        .as_ref()
        .map(|d| d.parse::<Uuid>())
        .transpose()?;

    // Handle recurrence options
    let (rrule, timezone) = if command.recurrence.is_some() || command.every.is_some() {
        let timezone = if let Some(ref tz) = command.timezone {
            normalize_timezone_input(&tz)?
        } else {
            detect_system_timezone()
        };

        let rrule = if let Some(raw_rrule) = command.recurrence {
            // Use raw RRULE
            raw_rrule
        } else if let Some(shortcut) = command.every {
            // Generate RRULE from shortcut
            generate_rrule_from_shortcut(shortcut, &command, due_at.unwrap_or_else(Utc::now))?
        } else {
            return Err(anyhow::anyhow!("Either --recurrence or --every must be provided for recurring tasks"));
        };

        (Some(rrule), Some(timezone))
    } else {
        (None, command.timezone.map(|tz| normalize_timezone_input(&tz)).transpose()?)
    };

    let new_task_data = NewTaskData {
        name: command.name,
        description: command.description,
        due_at,
        priority: command.priority,
        project_name: command.project,
        project_id: None,
        tags: command.tag,
        parent_id: command.parent.as_ref().map(|p| p.parse()).transpose()?,
        rrule,
        depends_on,
        series_id: None,
        timezone,
    };

    let is_recurring = new_task_data.rrule.is_some();
    let added_task = repo.add_task(new_task_data).await?;
    
    // Enhanced success feedback with colors and helpful information
    use owo_colors::{OwoColorize, Style};
    let success_style = Style::new().green().bold();
    let info_style = Style::new().blue();
    let subtle_style = Style::new().bright_black();
    
    if is_recurring {
        println!(
            "{} Created recurring task: {}", 
            "✓".style(success_style), 
            added_task.name.bright_white().bold()
        );
        println!(
            "  {} Task ID: {}", 
            "→".style(info_style), 
            added_task.id.to_string().yellow()
        );
        println!(
            "  {} Recurring series automatically created and activated", 
            "→".style(info_style)
        );
        
        // Show next steps for recurring tasks
        println!(
            "\n{} Next steps:", 
            "💡".style(subtle_style)
        );
        println!(
            "   {} Preview upcoming: rusk recur preview {}", 
            "•".style(subtle_style), 
            added_task.id.to_string().yellow()
        );
        println!(
            "   {} View series info: rusk recur info {}", 
            "•".style(subtle_style), 
            added_task.id.to_string().yellow()
        );
        println!(
            "   {} List all recurring: rusk list has:recurrence", 
            "•".style(subtle_style)
        );
    } else {
        println!(
            "{} Created task: {}", 
            "✓".style(success_style), 
            added_task.name.bright_white().bold()
        );
        println!(
            "  {} Task ID: {}", 
            "→".style(info_style), 
            added_task.id.to_string().yellow()
        );
        
        // Show helpful next steps for regular tasks
        if added_task.due_at.is_some() {
            println!(
                "  {} Due: {}", 
                "→".style(info_style), 
                added_task.due_at.unwrap().format("%Y-%m-%d %H:%M").to_string().cyan()
            );
        }
        
        println!(
            "\n{} Quick actions:", 
            "💡".style(subtle_style)
        );
        println!(
            "   {} Mark complete: rusk do {}", 
            "•".style(subtle_style), 
            added_task.id.to_string().yellow()
        );
        println!(
            "   {} Edit task: rusk edit {}", 
            "•".style(subtle_style), 
            added_task.id.to_string().yellow()
        );
    }

    Ok(())
}

/// Generate RRULE from recurrence shortcut with additional options
fn generate_rrule_from_shortcut(
    shortcut: RecurrenceShortcut,
    command: &AddCommand,
    base_time: DateTime<Utc>,
) -> Result<String> {
    let mut rrule = shortcut.to_rrule(base_time);
    
    // Add time specification
    if let Some(time_str) = &command.at {
        // Parse time (could be "9:00 AM", "14:30", etc.)
        let time = parse_time_string(time_str)?;
        rrule = format!("{};BYHOUR={};BYMINUTE={}", rrule, time.hour(), time.minute());
    }
    
    // Add day specification for weekly patterns
    if shortcut == RecurrenceShortcut::Weekly && command.on.is_some() {
        let days = parse_days_string(command.on.as_ref().unwrap())?;
        rrule = format!("{};BYDAY={}", rrule, days.join(","));
    }
    
    // Add end date
    if let Some(until_str) = &command.until {
        let until_date = parse_due_date(until_str, None)?;
        rrule = format!("{};UNTIL={}", rrule, until_date.format("%Y%m%dT%H%M%SZ"));
    }
    
    // Add count limit
    if let Some(count) = command.count {
        rrule = format!("{};COUNT={}", rrule, count);
    }
    
    Ok(rrule)
}

/// Parse time string like "9:00 AM", "14:30", "9pm", "noon", "midnight"
fn parse_time_string(time_str: &str) -> Result<chrono::NaiveTime> {
    use chrono::NaiveTime;
    
    let input = time_str.trim().to_lowercase();
    
    // Handle special times first
    match input.as_str() {
        "noon" | "12pm" | "12:00pm" => return Ok(NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
        "midnight" | "12am" | "12:00am" => return Ok(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        _ => {}
    }
    
    // Try various time formats with improved parsing
    let formats = [
        "%H:%M:%S",        // 14:30:00
        "%H:%M",           // 14:30
        "%I:%M:%S %p",     // 9:00:00 AM
        "%I:%M %p",        // 9:00 AM
        "%I%p",            // 9AM, 9PM
        "%I %p",           // 9 AM, 9 PM
        "%H",              // 14 (hour only)
    ];
    
    // Try original input first
    for format in &formats {
        if let Ok(time) = NaiveTime::parse_from_str(time_str, format) {
            return Ok(time);
        }
    }
    
    // Try with normalized input (lowercase)
    for format in &formats {
        if let Ok(time) = NaiveTime::parse_from_str(&input, format) {
            return Ok(time);
        }
    }
    
    // Enhanced error message with examples
    Err(anyhow::anyhow!(
        "Invalid time format: '{}'\n\nSupported formats:\n  • 24-hour: '14:30', '09:00'\n  • 12-hour: '2:30 PM', '9:00 AM'\n  • Compact: '2pm', '9am'\n  • Special: 'noon', 'midnight'", 
        time_str
    ))
}

/// Parse days string like "mon,tue,wed", "monday,tuesday", or "weekdays"
fn parse_days_string(days_str: &str) -> Result<Vec<String>> {
    let input = days_str.trim().to_lowercase();
    
    // Handle special day groups
    match input.as_str() {
        "weekdays" | "workdays" => {
            return Ok(vec!["MO".to_string(), "TU".to_string(), "WE".to_string(), "TH".to_string(), "FR".to_string()]);
        },
        "weekends" => {
            return Ok(vec!["SA".to_string(), "SU".to_string()]);
        },
        "daily" | "everyday" => {
            return Ok(vec!["MO".to_string(), "TU".to_string(), "WE".to_string(), "TH".to_string(), "FR".to_string(), "SA".to_string(), "SU".to_string()]);
        },
        _ => {}
    }
    
    let mut rrule_days = Vec::new();
    let mut invalid_days = Vec::new();
    
    for day in input.split(',') {
        let day = day.trim();
        if day.is_empty() {
            continue;
        }
        
        let rrule_day = match day {
            "mon" | "monday" | "m" => "MO",
            "tue" | "tuesday" | "tu" => "TU", 
            "wed" | "wednesday" | "w" => "WE",
            "thu" | "thursday" | "th" => "TH",
            "fri" | "friday" | "f" => "FR",
            "sat" | "saturday" | "sa" => "SA",
            "sun" | "sunday" | "su" => "SU",
            _ => {
                invalid_days.push(day.to_string());
                continue;
            }
        };
        
        if !rrule_days.contains(&rrule_day.to_string()) {
            rrule_days.push(rrule_day.to_string());
        }
    }
    
    if !invalid_days.is_empty() {
        return Err(anyhow::anyhow!(
            "Invalid day(s): {}\n\nSupported formats:\n  • Full names: 'monday,tuesday,wednesday'\n  • Short names: 'mon,tue,wed'\n  • Single letters: 'm,tu,w,th,f,sa,su'\n  • Groups: 'weekdays', 'weekends', 'daily'", 
            invalid_days.join(", ")
        ));
    }
    
    if rrule_days.is_empty() {
        return Err(anyhow::anyhow!(
            "No valid days specified in: '{}'\n\nExamples:\n  • mon,wed,fri\n  • weekdays\n  • monday,wednesday,friday", 
            days_str
        ));
    }
    
    Ok(rrule_days)
}