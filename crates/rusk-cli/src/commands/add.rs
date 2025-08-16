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
    
    if is_recurring {
        println!("Added recurring task with ID: {} (series created)", added_task.id);
    } else {
        println!("Added task with ID: {}", added_task.id);
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

/// Parse time string like "9:00 AM", "14:30", "9pm"
fn parse_time_string(time_str: &str) -> Result<chrono::NaiveTime> {
    use chrono::NaiveTime;
    
    // Try various time formats
    let formats = [
        "%H:%M",           // 14:30
        "%I:%M %p",        // 9:00 AM
        "%I%p",            // 9AM
        "%I %p",           // 9 AM
        "%H",              // 14
    ];
    
    for format in &formats {
        if let Ok(time) = NaiveTime::parse_from_str(time_str, format) {
            return Ok(time);
        }
    }
    
    Err(anyhow::anyhow!("Invalid time format: '{}'. Use formats like '9:00 AM', '14:30', '9pm'", time_str))
}

/// Parse days string like "mon,tue,wed" or "monday,tuesday"
fn parse_days_string(days_str: &str) -> Result<Vec<String>> {
    let mut rrule_days = Vec::new();
    
    for day in days_str.split(',') {
        let day = day.trim().to_lowercase();
        let rrule_day = match day.as_str() {
            "mon" | "monday" => "MO",
            "tue" | "tuesday" => "TU", 
            "wed" | "wednesday" => "WE",
            "thu" | "thursday" => "TH",
            "fri" | "friday" => "FR",
            "sat" | "saturday" => "SA",
            "sun" | "sunday" => "SU",
            _ => return Err(anyhow::anyhow!("Invalid day: '{}'. Use mon,tue,wed,thu,fri,sat,sun", day)),
        };
        rrule_days.push(rrule_day.to_string());
    }
    
    if rrule_days.is_empty() {
        return Err(anyhow::anyhow!("No valid days specified"));
    }
    
    Ok(rrule_days)
}