use crate::error::CoreError;
use crate::models::{
    NewTaskData, Task, TaskSeries, SeriesException, NewSeriesException, ExceptionType,
};
use crate::repository::SqliteRepository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, Transaction};
use uuid::Uuid;

#[async_trait]
impl super::ExceptionRepository for SqliteRepository {
    async fn add_series_exception(&self, exception: NewSeriesException) -> Result<SeriesException, CoreError> {
        let mut tx = self.pool().begin().await?;

        // Validate that the series exists
        let series: Option<TaskSeries> = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(exception.series_id)
            .fetch_optional(&mut *tx)
            .await?;

        if series.is_none() {
            return Err(CoreError::NotFound(format!("Series with id {} not found", exception.series_id)));
        }

        // Validate exception_task_id requirements based on exception type
        match exception.exception_type {
            ExceptionType::Skip => {
                if exception.exception_task_id.is_some() {
                    return Err(CoreError::InvalidInput(
                        "Skip exceptions should not have an exception_task_id".to_string()
                    ));
                }
            }
            ExceptionType::Override | ExceptionType::Move => {
                if exception.exception_task_id.is_none() {
                    return Err(CoreError::InvalidInput(
                        "Override and Move exceptions require an exception_task_id".to_string()
                    ));
                }
                
                // Validate that the exception task exists
                if let Some(task_id) = exception.exception_task_id {
                    let task_exists: Option<Task> = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
                        .bind(task_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                    
                    if task_exists.is_none() {
                        return Err(CoreError::NotFound(format!("Exception task with id {} not found", task_id)));
                    }
                }
            }
        }

        let series_exception = SeriesException {
            series_id: exception.series_id,
            occurrence_dt: exception.occurrence_dt,
            exception_type: exception.exception_type,
            exception_task_id: exception.exception_task_id,
            notes: exception.notes,
            created_at: Utc::now(),
        };

        sqlx::query(
            r#"INSERT INTO series_exceptions (series_id, occurrence_dt, exception_type, exception_task_id, notes, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)"#
        )
        .bind(series_exception.series_id)
        .bind(series_exception.occurrence_dt)
        .bind(&series_exception.exception_type)
        .bind(series_exception.exception_task_id)
        .bind(&series_exception.notes)
        .bind(series_exception.created_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(series_exception)
    }

    async fn find_series_exceptions(&self, series_id: Uuid) -> Result<Vec<SeriesException>, CoreError> {
        let exceptions = sqlx::query_as(
            "SELECT * FROM series_exceptions WHERE series_id = $1 ORDER BY occurrence_dt"
        )
        .bind(series_id)
        .fetch_all(self.pool())
        .await?;
        Ok(exceptions)
    }

    async fn remove_series_exception(&self, series_id: Uuid, occurrence_dt: DateTime<Utc>) -> Result<(), CoreError> {
        let result = sqlx::query(
            "DELETE FROM series_exceptions WHERE series_id = $1 AND occurrence_dt = $2"
        )
        .bind(series_id)
        .bind(occurrence_dt)
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(
                format!("Exception not found for series {} at occurrence {}", series_id, occurrence_dt)
            ));
        }

        Ok(())
    }

    async fn add_bulk_series_exceptions(&self, exceptions: Vec<NewSeriesException>) -> Result<Vec<SeriesException>, CoreError> {
        let mut tx = self.pool().begin().await?;
        let mut created_exceptions = Vec::new();

        for exception in exceptions {
            // Validate each exception
            Self::validate_exception_data(self, &exception).await?;
            
            let created = Self::add_series_exception_in_transaction(&mut tx, exception).await?;
            created_exceptions.push(created);
        }

        tx.commit().await?;
        Ok(created_exceptions)
    }

    async fn remove_bulk_series_exceptions(&self, series_id: Uuid, occurrence_dts: Vec<DateTime<Utc>>) -> Result<usize, CoreError> {
        let mut tx = self.pool().begin().await?;
        let mut total_removed = 0;

        for occurrence_dt in occurrence_dts {
            let result = sqlx::query(
                "DELETE FROM series_exceptions WHERE series_id = $1 AND occurrence_dt = $2"
            )
            .bind(series_id)
            .bind(occurrence_dt)
            .execute(&mut *tx)
            .await?;

            total_removed += result.rows_affected() as usize;
        }

        tx.commit().await?;
        Ok(total_removed)
    }

    async fn validate_exception_conflicts(&self, series_id: Uuid, new_exception: &NewSeriesException) -> Result<Vec<SeriesException>, CoreError> {
        // Check for existing exceptions at the same occurrence time
        let existing_exceptions: Vec<SeriesException> = sqlx::query_as(
            "SELECT * FROM series_exceptions WHERE series_id = $1 AND occurrence_dt = $2"
        )
        .bind(series_id)
        .bind(new_exception.occurrence_dt)
        .fetch_all(self.pool())
        .await?;

        Ok(existing_exceptions)
    }

    async fn override_occurrence_with_task(&self, series_id: Uuid, occurrence_dt: DateTime<Utc>, override_task_data: NewTaskData) -> Result<Task, CoreError> {
        let mut tx = self.pool().begin().await?;

        // Create the override task
        let override_task = Self::add_task_in_transaction(&mut tx, override_task_data).await?;

        // Create or update the exception
        let exception = NewSeriesException {
            series_id,
            occurrence_dt,
            exception_type: ExceptionType::Override,
            exception_task_id: Some(override_task.id),
            notes: Some(format!("Override task created: {}", override_task.name)),
        };

        Self::add_series_exception_in_transaction(&mut tx, exception).await?;

        tx.commit().await?;
        Ok(override_task)
    }

    async fn move_occurrence_with_validation(&self, series_id: Uuid, from_dt: DateTime<Utc>, to_dt: DateTime<Utc>, timezone: &str) -> Result<Task, CoreError> {
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

        // Validate timezone
        crate::timezone::validate_timezone(timezone)?;

        // Create moved task based on template
        let moved_task_data = NewTaskData {
            name: template_task.name.clone(),
            description: template_task.description.clone(),
            due_at: Some(to_dt),
            priority: Some(template_task.priority.clone()),
            project_id: template_task.project_id,
            tags: vec![], // Will be copied separately if needed
            parent_id: template_task.parent_id,
            depends_on: None,
            rrule: None,
            series_id: None, // This is a standalone moved task
            timezone: Some(timezone.to_string()),
            ..Default::default()
        };

        let moved_task = Self::add_task_in_transaction(&mut tx, moved_task_data).await?;

        // Create move exception
        let exception = NewSeriesException {
            series_id,
            occurrence_dt: from_dt,
            exception_type: ExceptionType::Move,
            exception_task_id: Some(moved_task.id),
            notes: Some(format!("Moved from {} to {} ({})", from_dt.format("%Y-%m-%d %H:%M"), to_dt.format("%Y-%m-%d %H:%M"), timezone)),
        };

        Self::add_series_exception_in_transaction(&mut tx, exception).await?;

        tx.commit().await?;
        Ok(moved_task)
    }
}

impl SqliteRepository {
    /// Validate exception data for consistency and business rules
    pub(crate) async fn validate_exception_data(&self, exception: &NewSeriesException) -> Result<(), CoreError> {
        // Validate series exists
        let series_exists = sqlx::query("SELECT 1 FROM task_series WHERE id = $1")
            .bind(exception.series_id)
            .fetch_optional(self.pool())
            .await?
            .is_some();

        if !series_exists {
            return Err(CoreError::NotFound(format!("Series with id {} not found", exception.series_id)));
        }

        // Validate exception type constraints
        match exception.exception_type {
            ExceptionType::Skip => {
                if exception.exception_task_id.is_some() {
                    return Err(CoreError::InvalidException(
                        "Skip exceptions cannot have an exception_task_id".to_string()
                    ));
                }
            }
            ExceptionType::Override | ExceptionType::Move => {
                if exception.exception_task_id.is_none() {
                    return Err(CoreError::InvalidException(
                        format!("{:?} exceptions must have an exception_task_id", exception.exception_type)
                    ));
                }
            }
        }

        Ok(())
    }

    /// Add a series exception within an existing transaction
    pub(crate) async fn add_series_exception_in_transaction(
        tx: &mut Transaction<'_, Sqlite>, 
        exception: NewSeriesException
    ) -> Result<SeriesException, CoreError> {
        let now = Utc::now();

        let created_exception = SeriesException {
            series_id: exception.series_id,
            occurrence_dt: exception.occurrence_dt,
            exception_type: exception.exception_type,
            exception_task_id: exception.exception_task_id,
            notes: exception.notes,
            created_at: now,
        };

        sqlx::query(
            "INSERT INTO series_exceptions (series_id, occurrence_dt, exception_type, exception_task_id, notes, created_at) 
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(created_exception.series_id)
        .bind(created_exception.occurrence_dt)
        .bind(&created_exception.exception_type)
        .bind(created_exception.exception_task_id)
        .bind(&created_exception.notes)
        .bind(created_exception.created_at)
        .execute(&mut **tx)
        .await?;

        Ok(created_exception)
    }
}