use crate::error::CoreError;
use crate::models::{
    NewTaskData, Task, TaskSeries, NewSeriesData, UpdateSeriesData, SeriesException, SeriesStatistics,
};
use crate::recurrence::RecurrenceManager;
use crate::repository::SqliteRepository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, Sqlite, Transaction};
use uuid::Uuid;

#[async_trait]
impl super::SeriesRepository for SqliteRepository {
    async fn create_series(&self, data: NewSeriesData) -> Result<TaskSeries, CoreError> {
        let mut tx = self.pool().begin().await?;

        // Validate RRULE and normalize it
        let normalized_rrule = RecurrenceManager::normalize_rrule(
            &data.rrule, 
            data.dtstart, 
            &data.timezone
        )?;

        // Ensure template task exists and is not already part of a series
        let template_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(data.template_task_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(data.template_task_id.to_string()))?;

        if template_task.series_id.is_some() {
            return Err(CoreError::InvalidInput(
                "Template task is already part of a series".to_string()
            ));
        }

        // Check if a series already exists for this template
        let existing_series: Option<TaskSeries> = sqlx::query_as(
            "SELECT * FROM task_series WHERE template_task_id = $1"
        )
        .bind(data.template_task_id)
        .fetch_optional(&mut *tx)
        .await?;

        if existing_series.is_some() {
            return Err(CoreError::InvalidInput(
                "A series already exists for this template task".to_string()
            ));
        }

        // Create the series
        let series = TaskSeries {
            id: Uuid::now_v7(),
            template_task_id: data.template_task_id,
            rrule: normalized_rrule,
            dtstart: data.dtstart,
            timezone: data.timezone,
            active: true,
            last_materialized_until: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        sqlx::query(
            r#"INSERT INTO task_series (id, template_task_id, rrule, dtstart, timezone, active, last_materialized_until, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#
        )
        .bind(series.id)
        .bind(series.template_task_id)
        .bind(&series.rrule)
        .bind(series.dtstart)
        .bind(&series.timezone)
        .bind(series.active)
        .bind(series.last_materialized_until)
        .bind(series.created_at)
        .bind(series.updated_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(series)
    }

    async fn find_series_by_id(&self, id: Uuid) -> Result<Option<TaskSeries>, CoreError> {
        let series = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(id)
            .fetch_optional(self.pool())
            .await?;
        Ok(series)
    }

    async fn find_series_by_template(&self, template_id: Uuid) -> Result<Option<TaskSeries>, CoreError> {
        let series = sqlx::query_as("SELECT * FROM task_series WHERE template_task_id = $1")
            .bind(template_id)
            .fetch_optional(self.pool())
            .await?;
        Ok(series)
    }

    async fn update_series(&self, id: Uuid, data: UpdateSeriesData) -> Result<TaskSeries, CoreError> {
        let mut tx = self.pool().begin().await?;

        // Check if series exists
        let current_series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Series with id {} not found", id)))?;

        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("UPDATE task_series SET ");
        let mut updated = false;

        if let Some(rrule) = &data.rrule {
            // Validate the new RRULE
            let timezone = data.timezone.as_deref().unwrap_or(&current_series.timezone);
            RecurrenceManager::validate_rrule(rrule, timezone)?;
            
            qb.push("rrule = ");
            qb.push_bind(rrule);
            updated = true;
        }

        if let Some(dtstart) = data.dtstart {
            if updated {
                qb.push(", ");
            }
            qb.push("dtstart = ");
            qb.push_bind(dtstart);
            updated = true;
        }

        if let Some(timezone) = &data.timezone {
            // Validate timezone
            RecurrenceManager::validate_rrule(&current_series.rrule, timezone)?;
            
            if updated {
                qb.push(", ");
            }
            qb.push("timezone = ");
            qb.push_bind(timezone);
            updated = true;
        }

        if let Some(active) = data.active {
            if updated {
                qb.push(", ");
            }
            qb.push("active = ");
            qb.push_bind(active);
            updated = true;
        }

        if updated {
            qb.push(", updated_at = ");
            qb.push_bind(Utc::now());
            qb.push(" WHERE id = ");
            qb.push_bind(id);

            qb.build().execute(&mut *tx).await?;

            // If RRULE or timezone changed, reset materialization boundary
            if data.rrule.is_some() || data.timezone.is_some() {
                sqlx::query("UPDATE task_series SET last_materialized_until = NULL WHERE id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        let updated_series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(updated_series)
    }

    async fn delete_series(&self, id: Uuid) -> Result<(), CoreError> {
        let mut tx = self.pool().begin().await?;

        // Check if series exists
        let series: Option<TaskSeries> = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;

        if series.is_none() {
            return Err(CoreError::NotFound(format!("Series with id {} not found", id)));
        }

        // Delete all series exceptions first (due to foreign key constraints)
        sqlx::query("DELETE FROM series_exceptions WHERE series_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        // Delete all instance tasks that belong to this series
        sqlx::query("DELETE FROM tasks WHERE series_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        // Delete the series itself
        let result = sqlx::query("DELETE FROM task_series WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(format!("Series with id {} not found", id)));
        }

        tx.commit().await?;
        Ok(())
    }

    async fn find_active_series(&self) -> Result<Vec<TaskSeries>, CoreError> {
        let series = sqlx::query_as("SELECT * FROM task_series WHERE active = true ORDER BY created_at")
            .fetch_all(self.pool())
            .await?;
        Ok(series)
    }

    async fn duplicate_series(&self, series_id: Uuid, new_name: String, new_timezone: Option<String>) -> Result<TaskSeries, CoreError> {
        let mut tx = self.pool().begin().await?;

        // Get original series and template
        let original_series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(series_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Series with id {} not found", series_id)))?;

        let original_template: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(original_series.template_task_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Template task with id {} not found", original_series.template_task_id)))?;

        // Create new template task
        let new_template_data = NewTaskData {
            name: new_name,
            description: original_template.description.clone(),
            due_at: original_template.due_at,
            priority: Some(original_template.priority.clone()),
            project_id: original_template.project_id,
            tags: vec![], // Will be copied from original if needed
            parent_id: None, // Don't duplicate parent relationships
            depends_on: None,
            rrule: None,
            series_id: None,
            timezone: new_timezone.clone(),
            ..Default::default()
        };

        let new_template = Self::add_task_in_transaction(&mut tx, new_template_data).await?;

        // Create new series
        let new_series_data = NewSeriesData {
            template_task_id: new_template.id,
            rrule: original_series.rrule.clone(),
            dtstart: original_series.dtstart,
            timezone: new_timezone.unwrap_or(original_series.timezone.clone()),
        };

        let new_series = Self::create_series_in_transaction(&mut tx, new_series_data).await?;

        // Copy tags from original template if any
        let original_tags: Vec<(String,)> = sqlx::query_as("SELECT tag_name FROM task_tags WHERE task_id = $1")
            .bind(original_template.id)
            .fetch_all(&mut *tx)
            .await?;

        for (tag_name,) in original_tags {
            sqlx::query("INSERT INTO task_tags (task_id, tag_name) VALUES ($1, $2)")
                .bind(new_template.id)
                .bind(tag_name)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(new_series)
    }

    async fn archive_completed_series(&self, series_id: Uuid) -> Result<(), CoreError> {
        let mut tx = self.pool().begin().await?;

        // Verify all instances are completed or cancelled
        let pending_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tasks WHERE series_id = $1 AND status = 'pending'"
        )
        .bind(series_id)
        .fetch_one(&mut *tx)
        .await?;

        if pending_count.0 > 0 {
            return Err(CoreError::SeriesNotCompleted(format!(
                "Series has {} pending tasks that must be completed or cancelled before archiving", 
                pending_count.0
            )));
        }

        // Set series to inactive
        sqlx::query("UPDATE task_series SET active = false, updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(series_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn bulk_update_series(&self, updates: Vec<(Uuid, UpdateSeriesData)>) -> Result<Vec<TaskSeries>, CoreError> {
        let mut tx = self.pool().begin().await?;
        let mut updated_series = Vec::new();

        for (series_id, update_data) in updates {
            let updated = Self::update_series_in_transaction(&mut tx, series_id, update_data).await?;
            updated_series.push(updated);
        }

        tx.commit().await?;
        Ok(updated_series)
    }

    async fn find_series_by_pattern(&self, pattern: &str) -> Result<Vec<TaskSeries>, CoreError> {
        let series: Vec<TaskSeries> = sqlx::query_as(
            "SELECT ts.* FROM task_series ts 
             JOIN tasks t ON ts.template_task_id = t.id 
             WHERE t.name LIKE $1 OR ts.rrule LIKE $1"
        )
        .bind(format!("%{}%", pattern))
        .fetch_all(self.pool())
        .await?;

        Ok(series)
    }

    async fn get_series_statistics(&self, series_id: Uuid) -> Result<SeriesStatistics, CoreError> {
        // Get basic series info
        let series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(series_id)
            .fetch_optional(self.pool())
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Series with id {} not found", series_id)))?;

        // Get task statistics
        let task_stats: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT 
                COUNT(*) as total,
                COUNT(CASE WHEN status = 'completed' THEN 1 END) as completed,
                COUNT(CASE WHEN status = 'pending' THEN 1 END) as pending,
                COUNT(CASE WHEN status = 'cancelled' THEN 1 END) as cancelled
             FROM tasks 
             WHERE series_id = $1"
        )
        .bind(series_id)
        .fetch_one(self.pool())
        .await?;

        // Get exception statistics
        let exception_stats: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT 
                COUNT(*) as total,
                COUNT(CASE WHEN exception_type = 'skip' THEN 1 END) as skip,
                COUNT(CASE WHEN exception_type = 'override' THEN 1 END) as override,
                COUNT(CASE WHEN exception_type = 'move' THEN 1 END) as move
             FROM series_exceptions 
             WHERE series_id = $1"
        )
        .bind(series_id)
        .fetch_one(self.pool())
        .await?;

        // Get time-based statistics
        let time_stats: (Option<DateTime<Utc>>, Option<DateTime<Utc>>) = sqlx::query_as(
            "SELECT MIN(due_at), MAX(due_at) FROM tasks WHERE series_id = $1 AND due_at IS NOT NULL"
        )
        .bind(series_id)
        .fetch_one(self.pool())
        .await?;

        // Calculate next occurrence using RecurrenceManager
        let template_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(series.template_task_id)
            .fetch_optional(self.pool())
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Template task not found")))?;

        let exceptions: Vec<SeriesException> = sqlx::query_as(
            "SELECT * FROM series_exceptions WHERE series_id = $1"
        )
        .bind(series_id)
        .fetch_all(self.pool())
        .await?;

        let next_occurrence = if series.active {
            let recurrence_manager = RecurrenceManager::new(series.clone(), template_task, exceptions)?;
            recurrence_manager.next_occurrence_after(Utc::now())? 
        } else {
            None
        };

        // Calculate completion rate for health score
        let completion_rate = if task_stats.0 > 0 {
            task_stats.1 as f64 / task_stats.0 as f64
        } else {
            1.0
        };

        // Simple health score: completion rate * activity factor * consistency factor
        let activity_factor = if series.active { 1.0 } else { 0.8 };
        let consistency_factor = if (exception_stats.0 as f64) / (task_stats.0.max(1) as f64) < 0.2 { 1.0 } else { 0.9 };
        let health_score = completion_rate * activity_factor * consistency_factor;

        Ok(SeriesStatistics {
            series_id,
            total_occurrences_created: task_stats.0 as u32,
            completed_occurrences: task_stats.1 as u32,
            pending_occurrences: task_stats.2 as u32,
            cancelled_occurrences: task_stats.3 as u32,
            total_exceptions: exception_stats.0 as u32,
            skip_exceptions: exception_stats.1 as u32,
            override_exceptions: exception_stats.2 as u32,
            move_exceptions: exception_stats.3 as u32,
            first_occurrence: time_stats.0,
            last_occurrence: time_stats.1,
            next_occurrence,
            average_completion_time_hours: None, // Could be calculated if needed
            series_health_score: health_score,
        })
    }
}

impl SqliteRepository {
    /// Create a series within an existing transaction
    pub(crate) async fn create_series_in_transaction<'a>(
        tx: &mut Transaction<'a, Sqlite>,
        data: NewSeriesData,
    ) -> Result<TaskSeries, CoreError> {
        // Validate RRULE and normalize it
        let normalized_rrule = RecurrenceManager::normalize_rrule(
            &data.rrule, 
            data.dtstart, 
            &data.timezone
        )?;

        // Ensure template task exists and is not already part of a series
        let template_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(data.template_task_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(data.template_task_id.to_string()))?;

        if template_task.series_id.is_some() {
            return Err(CoreError::InvalidInput(
                "Template task is already part of a series".to_string()
            ));
        }

        // Check if a series already exists for this template
        let existing_series: Option<TaskSeries> = sqlx::query_as(
            "SELECT * FROM task_series WHERE template_task_id = $1"
        )
        .bind(data.template_task_id)
        .fetch_optional(&mut **tx)
        .await?;

        if existing_series.is_some() {
            return Err(CoreError::InvalidInput(
                "A series already exists for this template task".to_string()
            ));
        }

        // Create the series
        let series = TaskSeries {
            id: Uuid::now_v7(),
            template_task_id: data.template_task_id,
            rrule: normalized_rrule,
            dtstart: data.dtstart,
            timezone: data.timezone,
            active: true,
            last_materialized_until: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        sqlx::query(
            r#"INSERT INTO task_series (id, template_task_id, rrule, dtstart, timezone, active, last_materialized_until, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#
        )
        .bind(series.id)
        .bind(series.template_task_id)
        .bind(&series.rrule)
        .bind(series.dtstart)
        .bind(&series.timezone)
        .bind(series.active)
        .bind(series.last_materialized_until)
        .bind(series.created_at)
        .bind(series.updated_at)
        .execute(&mut **tx)
        .await?;

        Ok(series)
    }

    /// Update a series within an existing transaction
    pub(crate) async fn update_series_in_transaction(
        tx: &mut Transaction<'_, Sqlite>, 
        series_id: Uuid, 
        data: UpdateSeriesData
    ) -> Result<TaskSeries, CoreError> {
        let mut query_parts = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(rrule) = &data.rrule {
            query_parts.push("rrule = ?");
            params.push(rrule.clone());
        }

        if let Some(dtstart) = &data.dtstart {
            query_parts.push("dtstart = ?");
            params.push(dtstart.to_rfc3339());
        }

        if let Some(timezone) = &data.timezone {
            query_parts.push("timezone = ?");
            params.push(timezone.clone());
        }

        if let Some(active) = &data.active {
            query_parts.push("active = ?");
            params.push(if *active { "1".to_string() } else { "0".to_string() });
        }

        if query_parts.is_empty() {
            // No updates, just return current series
            let series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = ?")
                .bind(series_id)
                .fetch_optional(&mut **tx)
                .await?
                .ok_or_else(|| CoreError::NotFound(format!("Series with id {} not found", series_id)))?;
            return Ok(series);
        }

        query_parts.push("updated_at = ?");
        let now = Utc::now();
        params.push(now.to_rfc3339());

        let query = format!(
            "UPDATE task_series SET {} WHERE id = ?",
            query_parts.join(", ")
        );

        let mut sqlx_query = sqlx::query(&query);
        for param in params {
            sqlx_query = sqlx_query.bind(param);
        }
        sqlx_query = sqlx_query.bind(series_id);

        let result = sqlx_query.execute(&mut **tx).await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(format!("Series with id {} not found", series_id)));
        }

        // Reset materialization boundary if RRULE or timezone changed
        if data.rrule.is_some() || data.timezone.is_some() || data.dtstart.is_some() {
            sqlx::query("UPDATE task_series SET last_materialized_until = NULL WHERE id = ?")
                .bind(series_id)
                .execute(&mut **tx)
                .await?;
        }

        // Fetch and return updated series
        let updated_series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = ?")
            .bind(series_id)
            .fetch_one(&mut **tx)
            .await?;

        Ok(updated_series)
    }
}