use rusk_core::error::CoreError;
use chrono::{DateTime, Utc, Datelike, Offset};
use chrono_tz::Tz;
use iana_time_zone;
use std::str::FromStr;

/// Validate IANA timezone name
pub fn validate_timezone(timezone: &str) -> Result<(), CoreError> {
    Tz::from_str(timezone)
        .map(|_| ())
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))
}

/// Detect system timezone
pub fn detect_system_timezone() -> String {
    // Try multiple detection methods
    if let Ok(tz) = std::env::var("TZ") {
        if !tz.is_empty() && validate_timezone(&tz).is_ok() {
            return tz;
        }
    }
    
    // Use iana-time-zone crate
    if let Ok(tz) = iana_time_zone::get_timezone() {
        if validate_timezone(&tz).is_ok() {
            return tz;
        }
    }
    
    // Fallback to UTC
    "UTC".to_string()
}

/// Get common timezones for user selection
pub fn get_common_timezones() -> Vec<&'static str> {
    vec![
        "UTC",
        "America/New_York",
        "America/Chicago",
        "America/Denver", 
        "America/Los_Angeles",
        "America/Toronto",
        "America/Vancouver",
        "Europe/London",
        "Europe/Paris",
        "Europe/Berlin",
        "Europe/Rome",
        "Europe/Madrid",
        "Europe/Amsterdam",
        "Europe/Stockholm",
        "Asia/Tokyo",
        "Asia/Seoul",
        "Asia/Shanghai",
        "Asia/Hong_Kong",
        "Asia/Singapore",
        "Asia/Bangkok",
        "Asia/Mumbai",
        "Asia/Dubai",
        "Australia/Sydney",
        "Australia/Melbourne",
        "Pacific/Auckland",
    ]
}

/// Suggest similar timezone for invalid input
pub fn suggest_timezone(invalid: &str) -> Vec<&'static str> {
    let invalid_lower = invalid.to_lowercase();
    let common = get_common_timezones();
    
    // Simple fuzzy matching
    let mut matches: Vec<_> = common.into_iter()
        .filter(|tz| {
            let tz_lower = tz.to_lowercase();
            tz_lower.contains(&invalid_lower) || 
            invalid_lower.contains(&tz_lower) ||
            // Check city names
            tz.split('/').any(|part| part.to_lowercase().contains(&invalid_lower))
        })
        .collect();
    
    // Limit to top 5 suggestions
    matches.truncate(5);
    matches
}

/// Format timezone for display (with abbreviation if possible)
pub fn format_timezone_display(datetime: DateTime<Utc>, timezone: &str) -> Result<String, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    let local_dt = datetime.with_timezone(&tz);
    
    // Format with timezone abbreviation
    Ok(format!("{} ({})", 
        local_dt.format("%Y-%m-%d %H:%M:%S"),
        local_dt.format("%Z")
    ))
}

/// Get timezone offset string for display (e.g., "-05:00")
pub fn get_timezone_offset(timezone: &str, at_time: DateTime<Utc>) -> Result<String, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    let local_dt = at_time.with_timezone(&tz);
    Ok(format!("{}", local_dt.format("%z")))
}

/// Get timezone abbreviation (e.g., "EST", "EDT")
pub fn get_timezone_abbreviation(timezone: &str, at_time: DateTime<Utc>) -> Result<String, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    let local_dt = at_time.with_timezone(&tz);
    Ok(format!("{}", local_dt.format("%Z")))
}

/// Convert user-friendly timezone input to IANA name
pub fn normalize_timezone_input(input: &str) -> Result<String, CoreError> {
    // First try direct parsing
    if validate_timezone(input).is_ok() {
        return Ok(input.to_string());
    }
    
    // Try common abbreviations and mappings
    let normalized = match input.to_lowercase().as_str() {
        "est" | "eastern" => "America/New_York",
        "cst" | "central" => "America/Chicago", 
        "mst" | "mountain" => "America/Denver",
        "pst" | "pacific" => "America/Los_Angeles",
        "gmt" | "utc" => "UTC",
        "bst" | "london" => "Europe/London",
        "cet" | "paris" => "Europe/Paris",
        "jst" | "tokyo" => "Asia/Tokyo",
        _ => {
            // Try suggestions
            let suggestions = suggest_timezone(input);
            if suggestions.is_empty() {
                return Err(CoreError::InvalidTimezone(format!(
                    "Unknown timezone '{}'. Use standard IANA names like 'America/New_York'", 
                    input
                )));
            } else {
                return Err(CoreError::InvalidTimezone(format!(
                    "Unknown timezone '{}'. Did you mean: {}?", 
                    input,
                    suggestions.join(", ")
                )));
            }
        }
    };
    
    validate_timezone(normalized)?;
    Ok(normalized.to_string())
}

/// Get all available timezones (IANA database)
pub fn get_all_timezones() -> Vec<&'static str> {
    // This would ideally load from chrono_tz's database
    // For now, return an extended list of common timezones
    vec![
        "UTC",
        "Africa/Cairo",
        "Africa/Johannesburg",
        "Africa/Lagos",
        "America/Anchorage",
        "America/Argentina/Buenos_Aires",
        "America/Chicago",
        "America/Denver",
        "America/Lima",
        "America/Los_Angeles",
        "America/Mexico_City",
        "America/New_York",
        "America/Sao_Paulo",
        "America/Toronto",
        "America/Vancouver",
        "Asia/Bangkok",
        "Asia/Calcutta",
        "Asia/Dubai",
        "Asia/Hong_Kong",
        "Asia/Jakarta",
        "Asia/Manila",
        "Asia/Seoul",
        "Asia/Shanghai",
        "Asia/Singapore",
        "Asia/Tokyo",
        "Australia/Melbourne",
        "Australia/Perth",
        "Australia/Sydney",
        "Europe/Amsterdam",
        "Europe/Berlin",
        "Europe/London",
        "Europe/Madrid",
        "Europe/Paris",
        "Europe/Rome",
        "Europe/Stockholm",
        "Pacific/Auckland",
        "Pacific/Honolulu",
    ]
}

/// Check if timezone observes DST
pub fn timezone_observes_dst(timezone: &str) -> Result<bool, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    // Check if the timezone has different offsets in summer vs winter
    let now = Utc::now();
    let jan_date = now.with_month(1).unwrap().with_day(15).unwrap();
    let jul_date = now.with_month(7).unwrap().with_day(15).unwrap();
    
    let jan_local = jan_date.with_timezone(&tz);
    let jul_local = jul_date.with_timezone(&tz);
    
    Ok(jan_local.offset().fix() != jul_local.offset().fix())
}

/// Format datetime according to user preferences
pub fn format_with_user_preferences(
    datetime: DateTime<Utc>,
    timezone: &str,
    use_24h: bool,
    show_timezone: bool,
    show_seconds: bool,
) -> Result<String, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    let local_dt = datetime.with_timezone(&tz);
    
    let mut format_str = String::new();
    format_str.push_str("%Y-%m-%d ");
    
    if use_24h {
        if show_seconds {
            format_str.push_str("%H:%M:%S");
        } else {
            format_str.push_str("%H:%M");
        }
    } else {
        if show_seconds {
            format_str.push_str("%I:%M:%S %p");
        } else {
            format_str.push_str("%I:%M %p");
        }
    }
    
    if show_timezone {
        format_str.push_str(" %Z");
    }
    
    Ok(local_dt.format(&format_str).to_string())
}

/// Get timezone information for display
pub fn get_timezone_info(timezone: &str) -> Result<TimezoneInfo, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    let now = Utc::now();
    let local_dt = now.with_timezone(&tz);
    
    let observes_dst = timezone_observes_dst(timezone)?;
    let offset = format!("{}", local_dt.format("%z"));
    let abbreviation = format!("{}", local_dt.format("%Z"));
    let current_time = format!("{}", local_dt.format("%Y-%m-%d %H:%M:%S"));
    
    Ok(TimezoneInfo {
        name: timezone.to_string(),
        offset,
        abbreviation,
        current_time,
        observes_dst,
    })
}

/// Timezone information structure
#[derive(Debug, Clone)]
pub struct TimezoneInfo {
    pub name: String,
    pub offset: String,
    pub abbreviation: String,
    pub current_time: String,
    pub observes_dst: bool,
}

