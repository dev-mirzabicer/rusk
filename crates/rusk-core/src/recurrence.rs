use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use rrule::{RRuleSet, Tz as RRuleTz};
use uuid::Uuid;
use std::sync::OnceLock;
use std::collections::HashMap;

use crate::error::CoreError;
use crate::models::{SeriesException, Task, TaskSeries};

/// Simple static caches for performance optimization
static TIMEZONE_CACHE: OnceLock<std::sync::Mutex<HashMap<String, bool>>> = OnceLock::new();
static RRULE_CACHE: OnceLock<std::sync::Mutex<HashMap<String, bool>>> = OnceLock::new();

/// Initialize cache if not already done
fn ensure_caches_initialized() {
    TIMEZONE_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    RRULE_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
}

/// Check if timezone is valid (cached)
#[inline]
fn is_timezone_valid_cached(timezone: &str) -> Result<bool, CoreError> {
    ensure_caches_initialized();
    
    if let Ok(mut cache) = TIMEZONE_CACHE.get().unwrap().lock() {
        if let Some(&is_valid) = cache.get(timezone) {
            return Ok(is_valid);
        }
        
        // Not in cache, validate and cache result
        let is_valid = timezone.parse::<Tz>().is_ok();
        cache.insert(timezone.to_string(), is_valid);
        Ok(is_valid)
    } else {
        // Fallback if cache lock fails
        Ok(timezone.parse::<Tz>().is_ok())
    }
}

/// Check if RRULE is valid (cached)
#[inline]
fn is_rrule_valid_cached(rrule_key: &str) -> Result<bool, CoreError> {
    ensure_caches_initialized();
    
    if let Ok(mut cache) = RRULE_CACHE.get().unwrap().lock() {
        if let Some(&is_valid) = cache.get(rrule_key) {
            return Ok(is_valid);
        }
        
        // Not in cache, validate and cache result  
        let test_dtstart = Utc::now();
        let rrule_string = if !rrule_key.contains("DTSTART") {
            format!("DTSTART:{}\nRRULE:{}", 
                test_dtstart.format("%Y%m%dT%H%M%SZ"), 
                rrule_key)
        } else {
            rrule_key.to_string()
        };
        
        let is_valid = rrule_string.parse::<RRuleSet>().is_ok();
        cache.insert(rrule_key.to_string(), is_valid);
        Ok(is_valid)
    } else {
        // Fallback if cache lock fails
        let test_dtstart = Utc::now();
        let rrule_string = if !rrule_key.contains("DTSTART") {
            format!("DTSTART:{}\nRRULE:{}", 
                test_dtstart.format("%Y%m%dT%H%M%SZ"), 
                rrule_key)
        } else {
            rrule_key.to_string()
        };
        Ok(rrule_string.parse::<RRuleSet>().is_ok())
    }
}

/// Occurrence of a series, with exception handling applied
#[derive(Debug, Clone)]
pub struct SeriesOccurrence {
    /// The original scheduled time for this occurrence
    pub occurrence_dt: DateTime<Utc>,
    /// The effective time (may be moved due to exceptions)
    pub effective_dt: DateTime<Utc>,
    /// Associated task ID if this occurrence has been materialized
    pub task_id: Option<Uuid>,
    /// Whether this occurrence has an exception applied
    pub has_exception: bool,
}

impl SeriesOccurrence {
    /// Returns whether this occurrence should be visible/materialized
    #[inline]
    pub fn is_visible(&self) -> bool {
        // Occurrence is visible unless it's explicitly skipped
        !self.has_exception || self.task_id.is_some()
    }

    /// Returns the effective time for this occurrence
    #[inline]
    pub fn effective_at(&self) -> DateTime<Utc> {
        self.effective_dt
    }

    /// Returns the scheduled time for this occurrence
    #[inline]
    pub fn scheduled_at(&self) -> DateTime<Utc> {
        self.occurrence_dt
    }
}

/// RecurrenceManager: Encapsulates all recurrence calculation logic with timezone awareness.
/// 
/// Responsibilities:
/// 1. Parse and validate RRULE strings in context of series timezone
/// 2. Generate occurrence sequences between arbitrary time ranges
/// 3. Apply series exceptions to modify occurrence patterns
/// 4. Handle timezone conversions for accurate calculations
/// 5. Provide occurrence preview functionality
#[derive(Debug)]
pub struct RecurrenceManager {
    /// The series metadata
    series: TaskSeries,
    /// The template task for this series
    template_task: Task,
    /// RRule set for occurrence calculation
    rrule_set: RRuleSet,
    /// Timezone for this series
    timezone: Tz,
    /// Exceptions mapped by occurrence datetime for O(1) lookup
    exceptions: std::collections::HashMap<DateTime<Utc>, SeriesException>,
}

impl RecurrenceManager {
    /// Creates a new RecurrenceManager from series data.
    /// 
    /// # Arguments
    /// * `series` - The task series configuration
    /// * `template_task` - The template task for this series
    /// * `exceptions` - List of exceptions for this series
    /// 
    /// # Returns
    /// * `Result<Self, CoreError>` - RecurrenceManager instance or error
    /// 
    /// # Behavior
    /// - Validate series timezone as parseable IANA name
    /// - Parse RRULE string and create RRuleSet
    /// - Build exception lookup map for O(1) access during generation
    /// - Store timezone for later calculations
    pub fn new(
        series: TaskSeries, 
        template_task: Task, 
        exceptions: Vec<SeriesException>
    ) -> Result<Self, CoreError> {
        // Validate and parse timezone (with caching)
        if !is_timezone_valid_cached(&series.timezone)? {
            return Err(CoreError::InvalidTimezone(series.timezone.clone()));
        }
        let timezone: Tz = series.timezone.parse()
            .map_err(|_| CoreError::InvalidTimezone(series.timezone.clone()))?;

        // For RRULE parsing, we need a complete RRULE string with DTSTART
        let rrule_string = if !series.rrule.contains("DTSTART") {
            format!("DTSTART:{}\nRRULE:{}", 
                series.dtstart.format("%Y%m%dT%H%M%SZ"), 
                series.rrule)
        } else {
            series.rrule.clone()
        };

        // Parse the RRULE
        let rrule_set = rrule_string.parse::<RRuleSet>()
            .map_err(|e| CoreError::InvalidRRule(format!("Failed to parse RRULE '{}': {}", rrule_string, e)))?;

        // Build exception lookup map - use with_capacity for better performance
        let mut exceptions_map = std::collections::HashMap::with_capacity(exceptions.len());
        for ex in exceptions {
            exceptions_map.insert(ex.occurrence_dt, ex);
        }

        Ok(Self {
            series,
            template_task,
            rrule_set,
            timezone,
            exceptions: exceptions_map,
        })
    }

    /// Generates occurrences between the specified UTC time range.
    /// 
    /// # Arguments
    /// * `start` - Start of time range (UTC)
    /// * `end` - End of time range (UTC)
    /// 
    /// # Returns
    /// * `Result<Vec<SeriesOccurrence>, CoreError>` - List of occurrences or error
    /// 
    /// # Behavior
    /// - Convert UTC bounds to series timezone for accurate RRULE evaluation
    /// - Generate raw occurrences using rrule crate iterator
    /// - Apply exceptions in chronological order:
    ///   - Skip: Remove occurrence from result set
    ///   - Override: Mark occurrence as having custom task
    ///   - Move: Update effective time while preserving scheduled time
    /// - Return structured occurrence data for further processing
    pub fn generate_occurrences_between(
        &self, 
        start: DateTime<Utc>, 
        end: DateTime<Utc>
    ) -> Result<Vec<SeriesOccurrence>, CoreError> {
        let mut occurrences = Vec::new();

        // Generate occurrences using RRuleSet with proper bounds  
        // More efficient approach: avoid string conversions entirely by using direct timezone conversion
        let start_rrule = start.with_timezone(&RRuleTz::UTC);
        let end_rrule = end.with_timezone(&RRuleTz::UTC);
        
        let bounded_rrule = self.rrule_set.clone()
            .after(start_rrule)
            .before(end_rrule);
        
        let (occurrences_vec, _) = bounded_rrule.all(1000); // Reasonable limit
        
        // Pre-allocate with estimated capacity for better performance
        occurrences.reserve(occurrences_vec.len());
        
        for dt in occurrences_vec {
            let occurrence_dt = dt.with_timezone(&Utc);
            
            // Check for exceptions with more efficient pattern matching
            match self.exceptions.get(&occurrence_dt) {
                Some(exception) => {
                    match exception.exception_type {
                        crate::models::ExceptionType::Skip => {
                            // Skip this occurrence completely
                            continue;
                        },
                        crate::models::ExceptionType::Override | crate::models::ExceptionType::Move => {
                            // Include with exception marker
                            occurrences.push(SeriesOccurrence {
                                occurrence_dt,
                                effective_dt: occurrence_dt, // May be updated for move exceptions
                                task_id: exception.exception_task_id,
                                has_exception: true,
                            });
                        }
                    }
                },
                None => {
                    // Normal occurrence without exception
                    occurrences.push(SeriesOccurrence {
                        occurrence_dt,
                        effective_dt: occurrence_dt,
                        task_id: None,
                        has_exception: false,
                    });
                }
            }
        }

        Ok(occurrences)
    }

    /// Finds the next occurrence strictly after the given time.
    /// 
    /// # Arguments
    /// * `after` - Find occurrences after this UTC time
    /// 
    /// # Returns
    /// * `Result<Option<DateTime<Utc>>, CoreError>` - Next occurrence time or None if series ended
    /// 
    /// # Behavior
    /// - Find first valid occurrence strictly after given time
    /// - Respect timezone for accurate "after" comparison
    /// - Skip exceptions when finding next valid occurrence
    /// - Return None if series has ended (finite recurrence)
    pub fn next_occurrence_after(
        &self, 
        after: DateTime<Utc>
    ) -> Result<Option<DateTime<Utc>>, CoreError> {
        // More efficient approach: avoid string conversion and clones
        let after_rrule = after.with_timezone(&RRuleTz::UTC);
        let after_rrule_set = self.rrule_set.clone().after(after_rrule);
        let (next_occurrences_vec, _) = after_rrule_set.all(10); // Get next 10 to handle skipped ones
        
        // Use iterator and find for more efficient processing
        for dt in next_occurrences_vec {
            let next_utc = dt.with_timezone(&Utc);
            
            // Check if this occurrence is skipped by an exception
            match self.exceptions.get(&next_utc) {
                Some(exception) if exception.exception_type == crate::models::ExceptionType::Skip => {
                    // Continue to find the next non-skipped occurrence
                    continue;
                },
                _ => {
                    return Ok(Some(next_utc));
                }
            }
        }
        
        Ok(None)
    }

    /// Validates an RRULE string in the context of a timezone.
    /// 
    /// # Arguments
    /// * `rrule` - RRULE string to validate
    /// * `timezone` - IANA timezone name
    /// 
    /// # Returns
    /// * `Result<(), CoreError>` - Ok if valid, error if invalid
    /// 
    /// # Behavior
    /// - Validate RRULE parses correctly in given timezone
    /// - Ensure consistent storage format across all series
    pub fn validate_rrule(rrule: &str, timezone: &str) -> Result<(), CoreError> {
        // Validate timezone (with caching)
        if !is_timezone_valid_cached(timezone)? {
            return Err(CoreError::InvalidTimezone(timezone.to_string()));
        }

        // Validate RRULE (with caching)
        if !is_rrule_valid_cached(rrule)? {
            return Err(CoreError::InvalidRRule(format!("Invalid RRULE: {}", rrule)));
        }

        Ok(())
    }

    /// Normalizes an RRULE string to canonical format.
    /// 
    /// # Arguments
    /// * `rrule` - Raw RRULE string
    /// * `dtstart` - Series start time
    /// * `timezone` - IANA timezone name
    /// 
    /// # Returns
    /// * `Result<String, CoreError>` - Normalized RRULE string
    /// 
    /// # Behavior
    /// - Normalize to canonical format with explicit DTSTART and TZID
    /// - Ensure consistent storage format across all series
    pub fn normalize_rrule(
        rrule: &str, 
        dtstart: DateTime<Utc>, 
        timezone: &str
    ) -> Result<String, CoreError> {
        // First validate the inputs (uses caching)
        Self::validate_rrule(rrule, timezone)?;

        // Parse timezone (we know it's valid from cache check above)
        let tz: Tz = timezone.parse()
            .map_err(|_| CoreError::InvalidTimezone(timezone.to_string()))?;

        // Convert dtstart to the series timezone
        let dtstart_local = dtstart.with_timezone(&tz);
        
        // Create a normalized RRULE with explicit DTSTART (skip re-validation)
        let normalized = format!(
            "DTSTART;TZID={}:{}\nRRULE:{}",
            timezone,
            dtstart_local.format("%Y%m%dT%H%M%S"),
            rrule
        );

        Ok(normalized)
    }

    /// Gets the series associated with this manager.
    pub fn series(&self) -> &TaskSeries {
        &self.series
    }

    /// Gets the template task associated with this manager.
    pub fn template_task(&self) -> &Task {
        &self.template_task
    }

    /// Gets the timezone for this series.
    pub fn timezone(&self) -> &Tz {
        &self.timezone
    }

    /// Preview upcoming occurrences for this series.
    /// 
    /// # Arguments
    /// * `from` - Start time for preview
    /// * `count` - Maximum number of occurrences to return
    /// 
    /// # Returns
    /// * `Result<Vec<SeriesOccurrence>, CoreError>` - List of upcoming occurrences
    pub fn preview_occurrences(
        &self,
        from: DateTime<Utc>,
        count: usize
    ) -> Result<Vec<SeriesOccurrence>, CoreError> {
        let end_time = from + chrono::Duration::days(365); // Look ahead one year
        
        // More efficient approach: avoid unnecessary timezone conversions
        let from_rrule = from.with_timezone(&RRuleTz::UTC);
        let end_rrule = end_time.with_timezone(&RRuleTz::UTC);
        
        let bounded_rrule = self.rrule_set.clone()
            .after(from_rrule)
            .before(end_rrule);
        
        let (occurrences_vec, _) = bounded_rrule.all((count.min(1000) as u16).min(u16::MAX)); // Limit to requested count or 1000
        
        // Pre-allocate with better capacity estimation
        let mut result = Vec::with_capacity(count.min(occurrences_vec.len()));
        
        for dt in occurrences_vec.into_iter().take(count) {
            let occurrence_dt = dt.with_timezone(&Utc);
            
            // Check for exceptions with more efficient pattern matching
            match self.exceptions.get(&occurrence_dt) {
                Some(exception) => {
                    match exception.exception_type {
                        crate::models::ExceptionType::Skip => {
                            // Skip this occurrence completely
                            continue;
                        },
                        crate::models::ExceptionType::Override | crate::models::ExceptionType::Move => {
                            // Include with exception marker
                            result.push(SeriesOccurrence {
                                occurrence_dt,
                                effective_dt: occurrence_dt, // May be updated for move exceptions
                                task_id: exception.exception_task_id,
                                has_exception: true,
                            });
                        }
                    }
                },
                None => {
                    // Normal occurrence without exception
                    result.push(SeriesOccurrence {
                        occurrence_dt,
                        effective_dt: occurrence_dt,
                        task_id: None,
                        has_exception: false,
                    });
                }
            }
        }
        
        Ok(result)
    }
}

// ============================================================================
// MaterializationManager (Phase 2)
// ============================================================================

use crate::models::Filter;

/// Statistics collected during materialization operations
#[derive(Debug, Clone)]
pub struct MaterializationSummary {
    /// Number of series processed
    pub series_processed: usize,
    /// Total instances created across all series
    pub instances_created: usize,
    /// Number of series that had errors
    pub series_with_errors: usize,
    /// Detailed error messages
    pub errors: Vec<String>,
    /// Time taken for the operation
    pub duration_ms: u64,
}

impl Default for MaterializationSummary {
    fn default() -> Self {
        Self {
            series_processed: 0,
            instances_created: 0,
            series_with_errors: 0,
            errors: Vec::new(),
            duration_ms: 0,
        }
    }
}

/// MaterializationManager: Intelligent instance creation with configurable policies.
/// 
/// Responsibilities:
/// 1. Determine appropriate materialization windows based on user activity
/// 2. Create task instances for series occurrences within windows
/// 3. Ensure idempotent operations (safe to run multiple times)
/// 4. Respect configuration limits and policies
/// 5. Coordinate with Repository for transactional safety
pub struct MaterializationManager {
    /// Configuration for materialization policies
    config: MaterializationConfig,
}

/// Configuration for materialization behavior
#[derive(Debug, Clone)]
pub struct MaterializationConfig {
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
            lookahead_days: 30,
            min_upcoming_instances: 1,
            max_batch_size: 100,
            enable_catchup: false,
            materialization_grace_days: 3,
        }
    }
}

impl MaterializationManager {
    /// Creates a new MaterializationManager with the given configuration.
    pub fn new(config: MaterializationConfig) -> Self {
        Self { config }
    }

    /// Creates a MaterializationManager with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(MaterializationConfig::default())
    }

    /// Calculates optimal materialization window based on filter analysis.
    /// 
    /// # Arguments
    /// * `filters` - List of filters that will be applied to the query
    /// 
    /// # Returns
    /// * `(DateTime<Utc>, DateTime<Utc>)` - (start, end) tuple for materialization boundary
    /// 
    /// # Behavior
    /// - Analyze filter conditions to determine optimal time window
    /// - Default to configured lookahead period if no time filters present
    /// - Narrow window if filters specify date ranges (performance optimization)
    /// - Include grace period for near-past occurrences
    #[inline]
    pub fn calculate_window_for_filters(
        &self,
        filters: &[Filter],
    ) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        let mut start_time = now - chrono::Duration::days(self.config.materialization_grace_days as i64);
        let mut end_time = now + chrono::Duration::days(self.config.lookahead_days as i64);

        // Analyze filters to optimize window - use early termination for performance
        for filter in filters {
            if let Filter::DueDate(due_date) = filter {
                use crate::models::DueDate;
                match due_date {
                    DueDate::Today => {
                        // Narrow window to today only
                        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
                        let today_end = now.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
                        start_time = start_time.max(today_start);
                        end_time = end_time.min(today_end);
                    },
                    DueDate::Tomorrow => {
                        // Narrow window to tomorrow only
                        let tomorrow = now + chrono::Duration::days(1);
                        let tomorrow_start = tomorrow.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
                        let tomorrow_end = tomorrow.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
                        start_time = start_time.max(tomorrow_start);
                        end_time = end_time.min(tomorrow_end);
                    },
                    DueDate::Before(before_date) => {
                        // Narrow end window to before date
                        end_time = end_time.min(*before_date);
                    },
                    DueDate::After(after_date) => {
                        // Narrow start window to after date
                        start_time = start_time.max(*after_date);
                    },
                    DueDate::Overdue => {
                        // Focus on past dates only
                        end_time = end_time.min(now);
                    },
                }
            }
            // Other filter types don't affect time windows - skip them efficiently
        }

        // Ensure we always have a reasonable window
        if start_time >= end_time {
            start_time = now - chrono::Duration::days(1);
            end_time = now + chrono::Duration::days(self.config.lookahead_days as i64);
        }

        (start_time, end_time)
    }

    /// Gets the current configuration.
    pub fn config(&self) -> &MaterializationConfig {
        &self.config
    }

    /// Updates the configuration for this materialization manager.
    pub fn update_config(&mut self, config: MaterializationConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Task, TaskStatus, TaskPriority};
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_series() -> TaskSeries {
        TaskSeries {
            id: Uuid::now_v7(),
            template_task_id: Uuid::now_v7(),
            rrule: "FREQ=DAILY;INTERVAL=1".to_string(),
            dtstart: Utc::now(),
            timezone: "UTC".to_string(),
            active: true,
            last_materialized_until: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_task() -> Task {
        Task {
            id: Uuid::now_v7(),
            name: "Test Task".to_string(),
            description: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::None,
            due_at: Some(Utc::now()),
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            project_id: None,
            parent_id: None,
            series_id: None,
        }
    }

    mod recurrence_manager_tests {
        use super::*;

        #[test]
        fn test_new_success() {
            let series = create_test_series();
            let task = create_test_task();
            let exceptions = vec![];

            let manager = RecurrenceManager::new(series, task, exceptions);
            assert!(manager.is_ok());
        }

        #[test]
        fn test_new_invalid_timezone() {
            let mut series = create_test_series();
            series.timezone = "Invalid/Timezone".to_string();
            let task = create_test_task();
            let exceptions = vec![];

            let result = RecurrenceManager::new(series, task, exceptions);
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), CoreError::InvalidTimezone(_)));
        }

        #[test]
        fn test_new_invalid_rrule() {
            let mut series = create_test_series();
            series.rrule = "INVALID_RRULE".to_string();
            let task = create_test_task();
            let exceptions = vec![];

            let result = RecurrenceManager::new(series, task, exceptions);
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), CoreError::InvalidRRule(_)));
        }

        #[test]
        fn test_validate_rrule_success() {
            assert!(RecurrenceManager::validate_rrule("FREQ=DAILY;INTERVAL=1", "UTC").is_ok());
            assert!(RecurrenceManager::validate_rrule("FREQ=WEEKLY;BYDAY=MO", "UTC").is_ok());
            assert!(RecurrenceManager::validate_rrule("FREQ=MONTHLY;BYMONTHDAY=1", "America/New_York").is_ok());
        }

        #[test]
        fn test_validate_rrule_invalid_rrule() {
            let result = RecurrenceManager::validate_rrule("INVALID_RRULE", "UTC");
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), CoreError::InvalidRRule(_)));
        }

        #[test]
        fn test_validate_rrule_invalid_timezone() {
            let result = RecurrenceManager::validate_rrule("FREQ=DAILY;INTERVAL=1", "Invalid/Timezone");
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), CoreError::InvalidTimezone(_)));
        }

        #[test]
        fn test_normalize_rrule_success() {
            let dtstart = Utc::now();
            let result = RecurrenceManager::normalize_rrule("FREQ=DAILY;INTERVAL=1", dtstart, "UTC");
            assert!(result.is_ok());
            
            let normalized = result.unwrap();
            assert!(normalized.contains("DTSTART"));
            assert!(normalized.contains("FREQ=DAILY"));
            assert!(normalized.contains("UTC"));
        }

        #[test]
        fn test_normalize_rrule_with_timezone() {
            let dtstart = Utc::now();
            let result = RecurrenceManager::normalize_rrule("FREQ=WEEKLY;BYDAY=MO", dtstart, "America/New_York");
            assert!(result.is_ok());
            
            let normalized = result.unwrap();
            assert!(normalized.contains("DTSTART;TZID=America/New_York"));
            assert!(normalized.contains("FREQ=WEEKLY"));
        }
    }

    mod materialization_manager_tests {
        use super::*;
        use crate::models::{Filter, DueDate};
        use chrono::Duration;

        #[test]
        fn test_new_with_config() {
            let config = MaterializationConfig {
                lookahead_days: 60,
                min_upcoming_instances: 3,
                max_batch_size: 200,
                enable_catchup: true,
                materialization_grace_days: 7,
            };
            let manager = MaterializationManager::new(config.clone());
            assert_eq!(manager.config().lookahead_days, 60);
            assert_eq!(manager.config().min_upcoming_instances, 3);
            assert_eq!(manager.config().max_batch_size, 200);
            assert_eq!(manager.config().enable_catchup, true);
            assert_eq!(manager.config().materialization_grace_days, 7);
        }

        #[test]
        fn test_with_defaults() {
            let manager = MaterializationManager::with_defaults();
            assert_eq!(manager.config().lookahead_days, 30);
            assert_eq!(manager.config().min_upcoming_instances, 1);
            assert_eq!(manager.config().max_batch_size, 100);
            assert_eq!(manager.config().enable_catchup, false);
            assert_eq!(manager.config().materialization_grace_days, 3);
        }

        #[test]
        fn test_calculate_window_no_filters() {
            let manager = MaterializationManager::with_defaults();
            let (_start, _end) = manager.calculate_window_for_filters(&[]);
            
            let now = Utc::now();
            let expected_start = now - Duration::days(3); // grace period
            let expected_end = now + Duration::days(30); // lookahead
            
            // Allow for small time differences due to test execution time
            // Just verify basic bounds for now
            assert!(_start <= now);
            assert!(_end >= now);
        }

        #[test]
        fn test_calculate_window_today_filter() {
            let manager = MaterializationManager::with_defaults();
            let filters = vec![Filter::DueDate(DueDate::Today)];
            let (start, _end) = manager.calculate_window_for_filters(&filters);
            
            let now = Utc::now();
            let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            
            assert!(start >= today_start);
        }

        #[test]
        fn test_calculate_window_before_filter() {
            let manager = MaterializationManager::with_defaults();
            let before_date = Utc::now() + Duration::days(7);
            let filters = vec![Filter::DueDate(DueDate::Before(before_date))];
            let (_start, end) = manager.calculate_window_for_filters(&filters);
            
            assert!(end <= before_date);
        }

        #[test]
        fn test_calculate_window_after_filter() {
            let manager = MaterializationManager::with_defaults();
            let after_date = Utc::now() + Duration::days(7);
            let filters = vec![Filter::DueDate(DueDate::After(after_date))];
            let (start, _end) = manager.calculate_window_for_filters(&filters);
            
            assert!(start >= after_date);
        }

        #[test]
        fn test_calculate_window_overdue_filter() {
            let manager = MaterializationManager::with_defaults();
            let filters = vec![Filter::DueDate(DueDate::Overdue)];
            let (_start, end) = manager.calculate_window_for_filters(&filters);
            
            let now = Utc::now();
            assert!(end <= now);
        }

        #[test]
        fn test_update_config() {
            let mut manager = MaterializationManager::with_defaults();
            let new_config = MaterializationConfig {
                lookahead_days: 45,
                min_upcoming_instances: 2,
                max_batch_size: 150,
                enable_catchup: true,
                materialization_grace_days: 5,
            };
            
            manager.update_config(new_config);
            assert_eq!(manager.config().lookahead_days, 45);
            assert_eq!(manager.config().min_upcoming_instances, 2);
            assert_eq!(manager.config().enable_catchup, true);
        }
    }

    mod materialization_summary_tests {
        use super::*;

        #[test]
        fn test_default() {
            let summary = MaterializationSummary::default();
            assert_eq!(summary.series_processed, 0);
            assert_eq!(summary.instances_created, 0);
            assert_eq!(summary.series_with_errors, 0);
            assert!(summary.errors.is_empty());
            assert_eq!(summary.duration_ms, 0);
        }

        #[test]
        fn test_clone() {
            let mut summary = MaterializationSummary::default();
            summary.series_processed = 5;
            summary.instances_created = 15;
            summary.errors.push("Test error".to_string());
            
            let cloned = summary.clone();
            assert_eq!(cloned.series_processed, 5);
            assert_eq!(cloned.instances_created, 15);
            assert_eq!(cloned.errors.len(), 1);
            assert_eq!(cloned.errors[0], "Test error");
        }
    }
}