use serde::Deserialize;
use figment::{Figment, providers::{Format, Toml, Env}};
use chrono_tz::Tz;
use std::str::FromStr;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub default_filters: Vec<String>,
    #[serde(default)]
    pub recurrence: MaterializationConfig,
}

/// Configuration for series materialization and recurrence handling
#[derive(Deserialize, Debug)]
pub struct MaterializationConfig {
    /// User's default timezone (IANA format)
    pub default_timezone: String,
    /// Default materialization window in days
    pub lookahead_days: u32,
    /// Always maintain N future instances
    pub min_upcoming_instances: u32,
    /// Limit for batch materialization operations
    pub max_batch_size: u32,
    /// Whether to materialize missed past occurrences
    pub enable_catchup: bool,
    /// Include near-past in materialization windows (days)
    pub materialization_grace_days: u32,
}

impl Default for MaterializationConfig {
    fn default() -> Self {
        Self {
            default_timezone: detect_system_timezone(),
            lookahead_days: 30,
            min_upcoming_instances: 1,
            max_batch_size: 100,
            enable_catchup: false,
            materialization_grace_days: 3,
        }
    }
}

impl Config {
    pub fn new() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("RUSK_"))
            .extract()
    }
}

// ============================================================================
// Timezone Utilities (Phase 1)
// ============================================================================

/// Validates that a timezone string is a valid IANA timezone name
pub fn validate_timezone(timezone: &str) -> Result<Tz, String> {
    Tz::from_str(timezone)
        .map_err(|_| format!("Invalid timezone: '{}'. Use IANA timezone names like 'America/New_York'", timezone))
}

/// Detects the system timezone, falling back to UTC if detection fails
pub fn detect_system_timezone() -> String {
    // Try to detect system timezone using various methods
    
    // Method 1: Check TZ environment variable
    if let Ok(tz) = std::env::var("TZ") {
        if validate_timezone(&tz).is_ok() {
            return tz;
        }
    }
    
    // Method 2: Try to read from /etc/timezone (Linux)
    #[cfg(target_os = "linux")]
    {
        if let Ok(tz) = std::fs::read_to_string("/etc/timezone") {
            let tz = tz.trim();
            if validate_timezone(tz).is_ok() {
                return tz.to_string();
            }
        }
    }
    
    // Method 3: Try to read from system configuration (macOS)
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("readlink")
            .arg("/etc/localtime")
            .output()
        {
            if let Ok(path) = String::from_utf8(output.stdout) {
                // Extract timezone from path like /usr/share/zoneinfo/America/New_York
                if let Some(tz) = path.strip_prefix("/usr/share/zoneinfo/") {
                    let tz = tz.trim();
                    if validate_timezone(tz).is_ok() {
                        return tz.to_string();
                    }
                }
            }
        }
    }
    
    // Method 4: Try using chrono's system timezone detection
    if let Ok(local_tz) = iana_time_zone::get_timezone() {
        if validate_timezone(&local_tz).is_ok() {
            return local_tz;
        }
    }
    
    // Fallback to UTC
    "UTC".to_string()
}

/// Gets a list of common/popular timezones for user selection
pub fn get_common_timezones() -> Vec<&'static str> {
    vec![
        "UTC",
        "America/New_York",
        "America/Chicago", 
        "America/Denver",
        "America/Los_Angeles",
        "America/Sao_Paulo",
        "Europe/London",
        "Europe/Paris",
        "Europe/Berlin",
        "Europe/Rome",
        "Europe/Madrid",
        "Asia/Tokyo",
        "Asia/Shanghai",
        "Asia/Kolkata",
        "Asia/Dubai",
        "Australia/Sydney",
        "Australia/Melbourne",
        "Pacific/Auckland",
    ]
}

/// Suggests similar timezone names when validation fails
pub fn suggest_timezone(invalid_tz: &str) -> Vec<String> {
    let common = get_common_timezones();
    let lower_invalid = invalid_tz.to_lowercase();
    
    let mut suggestions = Vec::new();
    
    // Find timezones that contain parts of the invalid timezone
    for &tz in &common {
        let lower_tz = tz.to_lowercase();
        if lower_tz.contains(&lower_invalid) || lower_invalid.contains(&lower_tz) {
            suggestions.push(tz.to_string());
        }
    }
    
    // If no partial matches, return some common ones
    if suggestions.is_empty() {
        suggestions.extend(common.iter().take(5).map(|s| s.to_string()));
    }
    
    suggestions
}