use anyhow::Result;
use chrono::{DateTime, Utc};
use chrono_english::{parse_date_string, Dialect};

pub fn parse_due_date(date_str: &str) -> Result<DateTime<Utc>> {
    parse_date_string(date_str, Utc::now(), Dialect::Us)
        .map_err(|e| anyhow::anyhow!("Failed to parse due date '{}': {}", date_str, e))
}