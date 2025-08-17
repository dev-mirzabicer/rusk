use crate::error::CoreError;
use crate::models::{Task, TaskSeries, TaskStatus, SeriesException};
use crate::recurrence::RecurrenceManager;
use crate::repository::{SqliteRepository, SeriesRepository};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[async_trait]
impl super::MaterializationRepository for SqliteRepository {
    async fn refresh_series_materialization(&self, window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> Result<(), CoreError> {
        let active_series = self.find_active_series().await?;
        
        for series in active_series {
            Self::refresh_single_series_materialization(self, series.id, window_start, window_end).await?;
        }
        
        Ok(())
    }

    async fn find_materialized_tasks_for_series(&self, series_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Task>, CoreError> {
        let tasks = sqlx::query_as(
            r#"SELECT * FROM tasks 
            WHERE series_id = $1 
            AND due_at BETWEEN $2 AND $3 
            ORDER BY due_at"#
        )
        .bind(series_id)
        .bind(start)
        .bind(end)
        .fetch_all(self.pool())
        .await?;
        Ok(tasks)
    }

    async fn update_series_materialization_boundary(&self, series_id: Uuid, boundary: DateTime<Utc>) -> Result<(), CoreError> {
        let result = sqlx::query(
            "UPDATE task_series SET last_materialized_until = $1, updated_at = $2 WHERE id = $3"
        )
        .bind(boundary)
        .bind(Utc::now())
        .bind(series_id)
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(format!("Series with id {} not found", series_id)));
        }

        Ok(())
    }
}

impl SqliteRepository {
    /// Refresh materialization for a single series (public method)
    pub async fn refresh_single_series_materialization(&self, series_id: Uuid, window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> Result<(), CoreError> {
        let mut tx = self.pool().begin().await?;

        // Get series and template task
        let series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(series_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Series with id {} not found", series_id)))?;

        let template_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(series.template_task_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Template task with id {} not found", series.template_task_id)))?;

        // Get exceptions for this series
        let exceptions: Vec<SeriesException> = sqlx::query_as(
            "SELECT * FROM series_exceptions WHERE series_id = $1"
        )
        .bind(series_id)
        .fetch_all(&mut *tx)
        .await?;

        // Create RecurrenceManager for occurrence generation
        let recurrence_manager = RecurrenceManager::new(series, template_task.clone(), exceptions)?;

        // Generate occurrences for the window
        let occurrences = recurrence_manager.generate_occurrences_between(window_start, window_end)?;

        // Get existing materialized tasks in this window
        let existing_tasks: Vec<Task> = sqlx::query_as(
            r#"SELECT * FROM tasks 
            WHERE series_id = $1 
            AND due_at BETWEEN $2 AND $3"#
        )
        .bind(series_id)
        .bind(window_start)
        .bind(window_end)
        .fetch_all(&mut *tx)
        .await?;

        let existing_due_dates: HashSet<DateTime<Utc>> = 
            existing_tasks.iter().filter_map(|t| t.due_at).collect();

        // Create missing instances
        let mut created_count = 0;
        for occurrence in occurrences {
            if !occurrence.is_visible() {
                continue; // Skip hidden occurrences
            }

            if existing_due_dates.contains(&occurrence.effective_dt) {
                continue; // Already materialized
            }

            // Create instance task
            let instance_task = Task {
                id: Uuid::now_v7(),
                name: template_task.name.clone(),
                description: template_task.description.clone(),
                status: TaskStatus::Pending,
                priority: template_task.priority.clone(),
                due_at: Some(occurrence.effective_dt),
                completed_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                project_id: template_task.project_id,
                parent_id: template_task.parent_id,
                series_id: Some(series_id),
            };

            sqlx::query(
                r#"INSERT INTO tasks (id, name, description, status, priority, due_at, completed_at, created_at, updated_at, project_id, parent_id, series_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#
            )
            .bind(instance_task.id)
            .bind(&instance_task.name)
            .bind(&instance_task.description)
            .bind(&instance_task.status)
            .bind(&instance_task.priority)
            .bind(instance_task.due_at)
            .bind(instance_task.completed_at)
            .bind(instance_task.created_at)
            .bind(instance_task.updated_at)
            .bind(instance_task.project_id)
            .bind(instance_task.parent_id)
            .bind(instance_task.series_id)
            .execute(&mut *tx)
            .await?;

            created_count += 1;

            // Respect batch size limits
            if created_count >= self.materialization_manager().config().max_batch_size {
                break;
            }
        }

        // Update materialization boundary
        if created_count > 0 {
            sqlx::query(
                "UPDATE task_series SET last_materialized_until = $1, updated_at = $2 WHERE id = $3"
            )
            .bind(window_end)
            .bind(Utc::now())
            .bind(series_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Refresh materialization for a single series within an existing transaction
    pub(crate) async fn refresh_single_series_materialization_in_transaction<'a>(
        tx: &mut Transaction<'a, Sqlite>,
        series_id: Uuid,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        // Get series and template task
        let series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(series_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Series with id {} not found", series_id)))?;

        let template_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(series.template_task_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Template task with id {} not found", series.template_task_id)))?;

        // Get exceptions for this series
        let exceptions: Vec<SeriesException> = sqlx::query_as(
            "SELECT * FROM series_exceptions WHERE series_id = $1"
        )
        .bind(series_id)
        .fetch_all(&mut **tx)
        .await?;

        // Create RecurrenceManager for occurrence generation
        let recurrence_manager = RecurrenceManager::new(series, template_task.clone(), exceptions)?;

        // Generate occurrences for the window
        let occurrences = recurrence_manager.generate_occurrences_between(window_start, window_end)?;

        // Get existing materialized tasks in this window
        let existing_tasks: Vec<Task> = sqlx::query_as(
            r#"SELECT * FROM tasks 
            WHERE series_id = $1 
            AND due_at BETWEEN $2 AND $3"#
        )
        .bind(series_id)
        .bind(window_start)
        .bind(window_end)
        .fetch_all(&mut **tx)
        .await?;

        let existing_due_dates: HashSet<DateTime<Utc>> = 
            existing_tasks.iter().filter_map(|t| t.due_at).collect();

        // Create missing instances
        let mut created_count = 0;
        for occurrence in occurrences {
            if !occurrence.is_visible() {
                continue; // Skip hidden occurrences
            }

            if existing_due_dates.contains(&occurrence.effective_dt) {
                continue; // Already materialized
            }

            // Create instance task
            let instance_task = Task {
                id: Uuid::now_v7(),
                name: template_task.name.clone(),
                description: template_task.description.clone(),
                status: TaskStatus::Pending,
                priority: template_task.priority.clone(),
                due_at: Some(occurrence.effective_dt),
                completed_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                project_id: template_task.project_id,
                parent_id: template_task.parent_id,
                series_id: Some(series_id),
            };

            sqlx::query(
                r#"INSERT INTO tasks (id, name, description, status, priority, due_at, completed_at, created_at, updated_at, project_id, parent_id, series_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#
            )
            .bind(instance_task.id)
            .bind(&instance_task.name)
            .bind(&instance_task.description)
            .bind(&instance_task.status)
            .bind(&instance_task.priority)
            .bind(instance_task.due_at)
            .bind(instance_task.completed_at)
            .bind(instance_task.created_at)
            .bind(instance_task.updated_at)
            .bind(instance_task.project_id)
            .bind(instance_task.parent_id)
            .bind(instance_task.series_id)
            .execute(&mut **tx)
            .await?;

            created_count += 1;

            // Respect batch size limits to prevent overwhelming the database
            const DEFAULT_MAX_BATCH_SIZE: usize = 100;
            if created_count >= DEFAULT_MAX_BATCH_SIZE {
                break;
            }
        }

        // Update materialization boundary
        if created_count > 0 {
            sqlx::query(
                "UPDATE task_series SET last_materialized_until = $1, updated_at = $2 WHERE id = $3"
            )
            .bind(window_end)
            .bind(Utc::now())
            .bind(series_id)
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }
}