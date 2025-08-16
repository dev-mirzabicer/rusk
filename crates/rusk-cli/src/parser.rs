use anyhow::Result;
use chrono::{DateTime, Utc};
use chrono_english::{parse_date_string, Dialect};

pub fn parse_due_date(date_str: &str, _timezone: Option<&str>) -> Result<DateTime<Utc>> {
    // For now, we'll use the existing implementation
    // Future enhancement: use timezone parameter for localized parsing
    parse_date_string(date_str, Utc::now(), Dialect::Us)
        .map_err(|e| anyhow::anyhow!("Failed to parse due date '{}': {}", date_str, e))
}