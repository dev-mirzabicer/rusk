use crate::models::Task;
use chrono::{DateTime, Utc};
use rrule::{RRuleSet, Tz};

pub struct RecurrenceManager {
    task: Task,
}

impl RecurrenceManager {
    pub fn new(task: Task) -> Self {
        Self { task }
    }

    /// Calculates the next occurrence of a task based on its RRULE.
    ///
    /// This is the final, robust implementation. It avoids the complex and
    /// unreliable `after()` method and instead uses a standard iterator `find()`
    /// call. This directly expresses the logic: "iterate through all possible
    /// occurrences and find the first one that is strictly greater than the
    /// last due date." This is simpler, more readable, and correct.
    pub fn get_next_occurrence(&self, last_due: DateTime<Utc>) -> Option<DateTime<Utc>> {
        if let Some(rrule_str) = &self.task.rrule {
            let rrule: RRuleSet = rrule_str.parse().ok()?;

            // Find the first occurrence that is strictly after the last due date.
            rrule
                .into_iter()
                .find(|occurrence| *occurrence > last_due.with_timezone(&Tz::UTC))
                .map(|dt| dt.with_timezone(&Utc))
        } else {
            None
        }
    }
}