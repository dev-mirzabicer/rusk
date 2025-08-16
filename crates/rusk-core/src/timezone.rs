use crate::error::CoreError;
use chrono::{DateTime, Utc, Datelike, TimeZone, Offset};
use chrono_tz::Tz;
use std::str::FromStr;

/// Validate IANA timezone name
pub fn validate_timezone(timezone: &str) -> Result<(), CoreError> {
    Tz::from_str(timezone)
        .map(|_| ())
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))
}

/// Check if DST is currently active for a timezone
pub fn is_dst_active(timezone: &str, at_time: DateTime<Utc>) -> Result<bool, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    let local_dt = at_time.with_timezone(&tz);
    
    // Check if the timezone is currently observing DST
    // This is a simple heuristic - compare current offset with January offset
    let jan_date = at_time.date_naive().with_day(1).unwrap();
    let jan_datetime = jan_date.and_hms_opt(12, 0, 0).unwrap();
    let jan_local = tz.from_utc_datetime(&jan_datetime);
    let jan_offset = jan_local.offset();
    let current_offset = local_dt.offset();
    
    Ok(current_offset.fix() != jan_offset.fix())
}

/// Handle DST transitions correctly for recurring events
pub fn handle_dst_transition(
    original_time: DateTime<Utc>,
    timezone: &str,
    target_local_time: chrono::NaiveTime,
) -> Result<DateTime<Utc>, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    let original_local = original_time.with_timezone(&tz);
    let target_date = original_local.date_naive();
    
    // Try to create the target datetime in the local timezone
    let target_naive = target_date.and_time(target_local_time);
    
    // Handle ambiguous times during DST transitions
    match tz.from_local_datetime(&target_naive).earliest() {
        Some(local_dt) => Ok(local_dt.with_timezone(&Utc)),
        None => {
            // Time doesn't exist (spring forward) - move to next valid time
            let next_time = target_local_time.overflowing_add_signed(chrono::Duration::hours(1)).0;
            let next_naive = target_date.and_time(next_time);
            match tz.from_local_datetime(&next_naive).earliest() {
                Some(local_dt) => Ok(local_dt.with_timezone(&Utc)),
                None => Ok(original_time), // Fallback to original time
            }
        }
    }
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

/// Format datetime with timezone-aware display
pub fn format_with_timezone(
    datetime: DateTime<Utc>,
    timezone: &str,
    format: &str,
) -> Result<String, CoreError> {
    let tz: Tz = timezone.parse()
        .map_err(|_| CoreError::InvalidTimezone(format!("Invalid timezone: {}", timezone)))?;
    
    let local_dt = datetime.with_timezone(&tz);
    Ok(local_dt.format(format).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_timezone() {
        assert!(validate_timezone("UTC").is_ok());
        assert!(validate_timezone("America/New_York").is_ok());
        assert!(validate_timezone("Invalid/Timezone").is_err());
    }

    #[test]
    fn test_timezone_offset() {
        let utc_time = Utc::now();
        assert!(get_timezone_offset("UTC", utc_time).is_ok());
        assert!(get_timezone_offset("America/New_York", utc_time).is_ok());
    }

    #[test]
    fn test_timezone_abbreviation() {
        let utc_time = Utc::now();
        let utc_abbr = get_timezone_abbreviation("UTC", utc_time).unwrap();
        assert_eq!(utc_abbr, "UTC");
    }
}