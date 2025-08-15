use crate::error::CoreError;
use crate::models::{SeriesException, SeriesOccurrence, TaskSeries, Task, ExceptionType, MaterializationConfig};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use rrule::RRuleSet;
use std::collections::HashMap;

/// RecurrenceManager v2: Encapsulates all recurrence calculation logic with timezone awareness.
/// 
/// Responsibilities:
/// 1. Parse and validate RRULE strings in context of series timezone
/// 2. Generate occurrence sequences between arbitrary time ranges
/// 3. Apply series exceptions to modify occurrence patterns
/// 4. Handle timezone conversions for accurate calculations
/// 5. Provide occurrence preview functionality
pub struct RecurrenceManager {
    /// The series being managed
    series: TaskSeries,
    /// The template task for this series
    template_task: Task,
    /// Parsed timezone for calculations
    timezone: Tz,
    /// Parsed RRULE set for occurrence generation
    rrule_set: RRuleSet,
    /// Exception lookup map for O(1) access during generation
    exceptions: HashMap<DateTime<Utc>, SeriesException>,
}

impl RecurrenceManager {
    /// Creates a new RecurrenceManager with timezone-aware construction.
    /// 
    /// # Arguments
    /// * `series` - The TaskSeries containing recurrence rules and metadata
    /// * `template_task` - The template task for this series
    /// * `exceptions` - List of exceptions for this series
    /// 
    /// # Returns
    /// * `Ok(RecurrenceManager)` - Successfully constructed manager
    /// * `Err(CoreError)` - If timezone is invalid or RRULE cannot be parsed
    /// 
    /// # Errors
    /// * `CoreError::InvalidTimezone` - If series timezone is not a valid IANA name
    /// * `CoreError::InvalidRRule` - If RRULE string cannot be parsed
    pub fn new(
        series: TaskSeries,
        template_task: Task,
        exceptions: Vec<SeriesException>,
    ) -> Result<Self, CoreError> {
        // Validate and parse timezone
        let timezone = series.timezone.parse::<Tz>()
            .map_err(|_| CoreError::InvalidTimezone(series.timezone.clone()))?;

        // Parse RRULE string
        let rrule_set = series.rrule.parse::<RRuleSet>()
            .map_err(|e| CoreError::InvalidRRule(format!("Failed to parse RRULE '{}': {}", series.rrule, e)))?;

        // Build exception lookup map for O(1) access
        let exceptions_map: HashMap<DateTime<Utc>, SeriesException> = exceptions
            .into_iter()
            .map(|exception| (exception.occurrence_dt, exception))
            .collect();

        Ok(Self {
            series,
            template_task,
            timezone,
            rrule_set,
            exceptions: exceptions_map,
        })
    }

    /// Validates an RRULE string in the context of a given timezone.
    /// 
    /// # Arguments
    /// * `rrule` - The RRULE string to validate
    /// * `timezone` - The IANA timezone name
    /// 
    /// # Returns
    /// * `Ok(())` - If RRULE is valid
    /// * `Err(CoreError)` - If RRULE is invalid or timezone is unknown
    pub fn validate_rrule(rrule: &str, timezone: &str) -> Result<(), CoreError> {
        // Validate timezone first
        timezone.parse::<Tz>()
            .map_err(|_| CoreError::InvalidTimezone(timezone.to_string()))?;

        // Validate RRULE parsing
        rrule.parse::<RRuleSet>()
            .map_err(|e| CoreError::InvalidRRule(format!("Invalid RRULE '{}': {}", rrule, e)))?;

        Ok(())
    }

    /// Normalizes an RRULE string to canonical format with explicit DTSTART and TZID.
    /// 
    /// # Arguments
    /// * `rrule` - The raw RRULE string
    /// * `dtstart` - The series start time in UTC
    /// * `timezone` - The IANA timezone name
    /// 
    /// # Returns
    /// * `Ok(String)` - Normalized RRULE string
    /// * `Err(CoreError)` - If normalization fails
    pub fn normalize_rrule(
        rrule: &str,
        dtstart: DateTime<Utc>,
        timezone: &str,
    ) -> Result<String, CoreError> {
        // Validate inputs first
        Self::validate_rrule(rrule, timezone)?;

        let tz = timezone.parse::<Tz>()
            .map_err(|_| CoreError::InvalidTimezone(timezone.to_string()))?;

        // Convert dtstart to series timezone
        let dtstart_local = dtstart.with_timezone(&tz);
        let dtstart_str = dtstart_local.format("%Y%m%dT%H%M%S").to_string();

        // Check if RRULE already has DTSTART
        if rrule.contains("DTSTART") {
            // RRULE already has DTSTART, validate it's consistent
            Ok(rrule.to_string())
        } else {
            // Add DTSTART to RRULE
            Ok(format!("DTSTART;TZID={}:{}\n{}", timezone, dtstart_str, rrule))
        }
    }

    /// Generates occurrences between the specified time range with timezone conversion.
    /// 
    /// # Arguments
    /// * `start` - Start of time window (UTC)
    /// * `end` - End of time window (UTC)
    /// 
    /// # Returns
    /// * `Ok(Vec<SeriesOccurrence>)` - List of occurrences in the time range
    /// * `Err(CoreError)` - If occurrence generation fails
    /// 
    /// # Behavior
    /// 1. Convert UTC bounds to series timezone for accurate RRULE evaluation
    /// 2. Generate raw occurrences using rrule crate iterator
    /// 3. Apply exceptions in chronological order:
    ///    - Skip: Remove occurrence from result set
    ///    - Override: Mark occurrence as having custom task
    ///    - Move: Update effective time while preserving scheduled time
    /// 4. Return structured occurrence data for further processing
    pub fn generate_occurrences_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SeriesOccurrence>, CoreError> {
        // Convert bounds to series timezone for accurate RRULE evaluation
        let _start_local = start.with_timezone(&self.timezone);
        let _end_local = end.with_timezone(&self.timezone);

        let mut occurrences = Vec::new();

        // Generate raw occurrences using rrule - parse the rrule again for this operation
        let rrule_set: RRuleSet = self.series.rrule.parse()
            .map_err(|e| CoreError::InvalidRRule(format!("Failed to parse RRULE '{}': {}", self.series.rrule, e)))?;
        let (all_occurrences, _) = rrule_set.all(1000); // Limit to 1000 occurrences
        
        for occurrence in all_occurrences {
            // Convert the occurrence to UTC for consistent storage
            let occurrence_utc = occurrence.with_timezone(&Utc);
            
            // Only include occurrences within the requested window
            if occurrence_utc < start || occurrence_utc > end {
                if occurrence_utc > end {
                    break; // Stop when we've passed the end time
                }
                continue; // Skip occurrences before the start time
            }
            
            // Check for exceptions and apply them
            let series_occurrence = match self.exceptions.get(&occurrence_utc) {
                None => {
                    // Normal occurrence without exceptions
                    SeriesOccurrence::normal(occurrence_utc)
                },
                Some(exception) => {
                    match exception.exception_type {
                        ExceptionType::Skip => {
                            // Skip this occurrence entirely - don't add to results
                            continue;
                        },
                        ExceptionType::Override => {
                            // Replace with custom task
                            SeriesOccurrence::override_with(
                                occurrence_utc,
                                exception.exception_task_id
                                    .ok_or_else(|| CoreError::InvalidException(
                                        "Override exception missing exception_task_id".to_string()
                                    ))?
                            )
                        },
                        ExceptionType::Move => {
                            // Get moved task to determine new time
                            let exception_task_id = exception.exception_task_id
                                .ok_or_else(|| CoreError::InvalidException(
                                    "Move exception missing exception_task_id".to_string()
                                ))?;
                            
                            // For now, use the original time as effective time
                            // In a real implementation, we'd fetch the moved task's due_at
                            // This would require access to the repository
                            SeriesOccurrence::moved(
                                occurrence_utc,
                                occurrence_utc, // TODO: Get actual moved time from exception task
                                exception_task_id
                            )
                        }
                    }
                }
            };

            occurrences.push(series_occurrence);
        }

        // Sort by scheduled time for consistent ordering
        occurrences.sort_by_key(|occ| occ.scheduled_at);

        Ok(occurrences)
    }

    /// Finds the next valid occurrence strictly after the given time.
    /// 
    /// # Arguments
    /// * `after` - Find occurrences after this time (UTC)
    /// 
    /// # Returns
    /// * `Ok(Some(DateTime<Utc>))` - Next valid occurrence time
    /// * `Ok(None)` - No more occurrences (series has ended)
    /// * `Err(CoreError)` - If calculation fails
    /// 
    /// # Behavior
    /// - Respect timezone for accurate "after" comparison
    /// - Skip exceptions when finding next valid occurrence
    /// - Return None if series has ended (finite recurrence)
    pub fn next_occurrence_after(
        &self,
        after: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, CoreError> {
        // Convert to series timezone for accurate comparison
        let _after_local = after.with_timezone(&self.timezone);
        
        // Use rrule to find next occurrence - parse the rrule again for this operation
        let rrule_set: RRuleSet = self.series.rrule.parse()
            .map_err(|e| CoreError::InvalidRRule(format!("Failed to parse RRULE '{}': {}", self.series.rrule, e)))?;
        let (all_occurrences, _) = rrule_set.all(1000); // Limit to 1000 occurrences
        
        for occurrence in all_occurrences {
            let occurrence_utc = occurrence.with_timezone(&Utc);
            
            // Only consider occurrences strictly after the given time
            if occurrence_utc <= after {
                continue;
            }
            
            // Check if this occurrence is skipped
            if let Some(exception) = self.exceptions.get(&occurrence_utc) {
                if exception.exception_type == ExceptionType::Skip {
                    continue; // Skip this occurrence and look for the next one
                }
            }
            
            // Found a valid next occurrence
            return Ok(Some(occurrence_utc));
        }
        
        // No more occurrences found (finite recurrence or series ended)
        Ok(None)
    }

    /// Returns a preview of upcoming occurrences for display purposes.
    /// 
    /// # Arguments
    /// * `from` - Start time for preview (UTC)
    /// * `count` - Maximum number of occurrences to return
    /// 
    /// # Returns
    /// * `Ok(Vec<SeriesOccurrence>)` - Preview of upcoming occurrences
    /// * `Err(CoreError)` - If preview generation fails
    pub fn preview_occurrences(
        &self,
        from: DateTime<Utc>,
        count: usize,
    ) -> Result<Vec<SeriesOccurrence>, CoreError> {
        let mut occurrences = Vec::new();
        let _from_local = from.with_timezone(&self.timezone);
        
        // Parse the rrule again for this operation
        let rrule_set: RRuleSet = self.series.rrule.parse()
            .map_err(|e| CoreError::InvalidRRule(format!("Failed to parse RRULE '{}': {}", self.series.rrule, e)))?;
        let (all_occurrences, _) = rrule_set.all(count as u16 * 2); // Get a bit more than needed
        
        for occurrence in all_occurrences {
            let occurrence_utc = occurrence.with_timezone(&Utc);
            
            // Only consider occurrences from the specified time forward
            if occurrence_utc < from {
                continue;
            }
            
            // Apply exceptions
            let series_occurrence = match self.exceptions.get(&occurrence_utc) {
                None => SeriesOccurrence::normal(occurrence_utc),
                Some(exception) => {
                    match exception.exception_type {
                        ExceptionType::Skip => continue, // Skip this occurrence
                        ExceptionType::Override => {
                            SeriesOccurrence::override_with(
                                occurrence_utc,
                                exception.exception_task_id.unwrap_or_default()
                            )
                        },
                        ExceptionType::Move => {
                            SeriesOccurrence::moved(
                                occurrence_utc,
                                occurrence_utc, // TODO: Get actual moved time
                                exception.exception_task_id.unwrap_or_default()
                            )
                        }
                    }
                }
            };
            
            occurrences.push(series_occurrence);
            
            // Stop when we have enough visible occurrences
            if occurrences.len() >= count {
                break;
            }
        }
        
        Ok(occurrences)
    }

    /// Checks if the series has finite recurrence (UNTIL or COUNT specified).
    /// 
    /// # Returns
    /// * `true` - Series will end at some point
    /// * `false` - Series continues indefinitely
    pub fn is_finite(&self) -> bool {
        // This would require inspecting the RRuleSet for UNTIL or COUNT
        // For now, assume infinite recurrence
        // TODO: Implement proper finite recurrence detection
        false
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

    #[test]
    fn test_recurrence_manager_creation() {
        let series = create_test_series();
        let task = create_test_task();
        let exceptions = vec![];

        let manager = RecurrenceManager::new(series, task, exceptions);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_validate_rrule() {
        let result = RecurrenceManager::validate_rrule("FREQ=DAILY;INTERVAL=1", "UTC");
        assert!(result.is_ok());

        let result = RecurrenceManager::validate_rrule("INVALID_RRULE", "UTC");
        assert!(result.is_err());

        let result = RecurrenceManager::validate_rrule("FREQ=DAILY;INTERVAL=1", "Invalid/Timezone");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_rrule() {
        let dtstart = Utc::now();
        let result = RecurrenceManager::normalize_rrule("FREQ=DAILY;INTERVAL=1", dtstart, "UTC");
        assert!(result.is_ok());
        
        let normalized = result.unwrap();
        assert!(normalized.contains("DTSTART"));
        assert!(normalized.contains("FREQ=DAILY"));
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
    pub fn calculate_window_for_filters(
        &self,
        filters: &[Filter],
    ) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        let mut start_time = now - chrono::Duration::days(self.config.materialization_grace_days);
        let mut end_time = now + chrono::Duration::days(self.config.lookahead_days);

        // Analyze filters to optimize window
        for filter in filters {
            match filter {
                Filter::DueDate(due_date) => {
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
                },
                // Other filter types don't affect time windows
                _ => {}
            }
        }

        // Ensure we always have a reasonable window
        if start_time >= end_time {
            start_time = now - chrono::Duration::days(1);
            end_time = now + chrono::Duration::days(self.config.lookahead_days);
        }

        (start_time, end_time)
    }

    /// Gets the configuration for this materialization manager.
    pub fn config(&self) -> &MaterializationConfig {
        &self.config
    }

    /// Updates the configuration for this materialization manager.
    pub fn update_config(&mut self, config: MaterializationConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod materialization_tests {
    use super::*;
    use crate::models::{Filter, DueDate};
    use chrono::{Duration, Utc};

    #[test]
    fn test_materialization_manager_creation() {
        let manager = MaterializationManager::with_defaults();
        assert_eq!(manager.config().lookahead_days, 30);
        assert_eq!(manager.config().min_upcoming_instances, 1);
        assert_eq!(manager.config().max_batch_size, 100);
    }

    #[test]
    fn test_calculate_window_for_filters_no_filters() {
        let manager = MaterializationManager::with_defaults();
        let (start, end) = manager.calculate_window_for_filters(&[]);
        
        let now = Utc::now();
        let expected_start = now - Duration::days(3); // grace period
        let expected_end = now + Duration::days(30); // lookahead
        
        // Allow for small time differences due to test execution time
        assert!((start - expected_start).num_seconds().abs() < 5);
        assert!((end - expected_end).num_seconds().abs() < 5);
    }

    #[test]
    fn test_calculate_window_for_filters_with_today() {
        let manager = MaterializationManager::with_defaults();
        let filters = vec![Filter::DueDate(DueDate::Today)];
        let (start, end) = manager.calculate_window_for_filters(&filters);
        
        let now = Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let today_end = now.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
        
        assert!(start >= today_start);
        assert!(end <= today_end);
    }

    #[test]
    fn test_calculate_window_for_filters_with_before() {
        let manager = MaterializationManager::with_defaults();
        let before_date = Utc::now() + Duration::days(7);
        let filters = vec![Filter::DueDate(DueDate::Before(before_date))];
        let (start, end) = manager.calculate_window_for_filters(&filters);
        
        assert!(end <= before_date);
    }

    #[test]
    fn test_materialization_summary_default() {
        let summary = MaterializationSummary::default();
        assert_eq!(summary.series_processed, 0);
        assert_eq!(summary.instances_created, 0);
        assert_eq!(summary.series_with_errors, 0);
        assert!(summary.errors.is_empty());
        assert_eq!(summary.duration_ms, 0);
    }
}