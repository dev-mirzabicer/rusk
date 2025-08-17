use crate::error::CoreError;
use crate::models::{
    CompletionResult, NewTaskData, Project, Task, TaskPriority, TaskStatus,
    UpdateTaskData, TaskSeries, SeriesException, NewSeriesData,
};
use crate::query::Query;
use crate::recurrence::RecurrenceManager;
use crate::repository::{TaskQueryResult, SqliteRepository, SeriesRepository};
use crate::repository::query_builder::SqlQueryBuilder;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, Sqlite, Transaction};
use uuid::Uuid;

#[async_trait]
impl super::TaskRepository for SqliteRepository {
    async fn add_task(&self, data: NewTaskData) -> Result<Task, CoreError> {
        let mut tx = self.pool().begin().await?;
        
        // Check if this is a recurring task
        if let Some(rrule) = &data.rrule {
            // Create template task first
            let mut template_data = data.clone();
            template_data.rrule = None; // Remove rrule for template task
            template_data.series_id = None; // Template tasks don't have series_id
            
            let template_task = Self::add_task_in_transaction(&mut tx, template_data).await?;
            
            // Create the series
            let _series_data = NewSeriesData {
                template_task_id: template_task.id,
                rrule: rrule.clone(),
                dtstart: data.due_at.unwrap_or_else(|| Utc::now()),
                timezone: data.timezone.unwrap_or_else(|| "UTC".to_string()),
            };
            
            // Create series using the domain trait
            let series = self.create_series(_series_data).await?;
            
            // Trigger initial materialization for lookahead window
            let (window_start, window_end) = self.materialization_manager().calculate_window_for_filters(&[]);
            self.refresh_single_series_materialization(series.id, window_start, window_end).await?;
            
            tx.commit().await?;
            Ok(template_task)
        } else {
            // Regular task
            let task = Self::add_task_in_transaction(&mut tx, data).await?;
            tx.commit().await?;
            Ok(task)
        }
    }

    async fn find_task_by_id(&self, id: Uuid) -> Result<Option<Task>, CoreError> {
        let task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(self.pool())
            .await?;
        Ok(task)
    }

    async fn find_tasks_by_short_id_prefix(&self, short_id: &str) -> Result<Vec<Task>, CoreError> {
        // Optimize: avoid format! allocation by using a more efficient pattern
        let mut pattern = String::with_capacity(short_id.len() + 1);
        pattern.push_str(short_id);
        pattern.push('%');
        
        let tasks: Vec<Task> = sqlx::query_as("SELECT * FROM tasks WHERE id LIKE ?")
            .bind(pattern)
            .fetch_all(self.pool())
            .await?;
        Ok(tasks)
    }

    async fn find_tasks_with_details(&self, query: &Query) -> Result<Vec<TaskQueryResult>, CoreError> {
        // ALWAYS ensure materialization before any query to prevent missing recurring tasks
        self.ensure_materialization_for_query(query).await?;

        let mut query_builder: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new(
            r#"WITH RECURSIVE task_hierarchy (id, name, description, status, priority, due_at, completed_at, created_at, updated_at, project_id, parent_id, series_id, depth, path) AS (
                SELECT
                    t.id, t.name, t.description, t.status, t.priority, t.due_at, t.completed_at, t.created_at, t.updated_at, t.project_id, t.parent_id, t.series_id,
                    0 as depth,
                    CAST(t.created_at AS TEXT) as path
                FROM tasks t
                WHERE t.parent_id IS NULL
                UNION ALL
                SELECT
                    t.id, t.name, t.description, t.status, t.priority, t.due_at, t.completed_at, t.created_at, t.updated_at, t.project_id, t.parent_id, t.series_id,
                    th.depth + 1,
                    th.path || ' -> ' || CAST(t.created_at AS TEXT)
                FROM tasks t
                JOIN task_hierarchy th ON t.parent_id = th.id
            )
            SELECT
                th.id, th.name, th.description, th.status, th.priority, th.due_at, th.completed_at, th.created_at, th.updated_at, th.project_id, th.parent_id, th.series_id, th.depth, th.path,
                p.name as project_name,
                GROUP_CONCAT(tt.tag_name) as tags
            FROM task_hierarchy th
            LEFT JOIN projects p ON th.project_id = p.id
            LEFT JOIN task_tags tt ON th.id = tt.task_id
            "#,
        );

        query_builder.push(" WHERE ");
        SqlQueryBuilder::build_sql_where_clause(query, &mut query_builder);

        query_builder.push(" GROUP BY th.id, th.name, th.description, th.status, th.priority, th.due_at, th.completed_at, th.created_at, th.updated_at, th.project_id, th.parent_id, th.series_id, th.depth, th.path, p.name");
        query_builder.push(" ORDER BY th.path");

        let tasks = query_builder.build_query_as().fetch_all(self.pool()).await?;
        Ok(tasks)
    }

    async fn delete_task(&self, id: Uuid) -> Result<(), CoreError> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
            .bind(id)
            .execute(self.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn complete_task(&self, id: Uuid) -> Result<CompletionResult, CoreError> {
        let mut tx = self.pool().begin().await?;

        // Get the task to check if it's part of a series
        let task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        // Check for blocking dependencies
        let dependencies: Vec<Task> = sqlx::query_as(
            r#"SELECT t.* FROM tasks t
            INNER JOIN task_dependencies td ON t.id = td.depends_on_id
            WHERE td.task_id = $1 AND t.status != 'completed'"#,
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;

        if !dependencies.is_empty() {
            // More efficient string collection without intermediate clones
            let dependency_names = dependencies
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(CoreError::TaskBlocked(dependency_names));
        }

        // Mark the current task as completed
        let completed_task: Task = sqlx::query_as(
            r#"UPDATE tasks
            SET status = $1, completed_at = $2, updated_at = $2
            WHERE id = $3
            RETURNING *
            "#,
        )
        .bind(TaskStatus::Completed)
        .bind(Utc::now())
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| CoreError::NotFound(id.to_string()))?;

        // Handle series-aware completion
        if let Some(series_id) = task.series_id {
            // This is a series instance, handle next occurrence
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

            // Create RecurrenceManager to calculate next occurrence
            let recurrence_manager = RecurrenceManager::new(series, template_task, exceptions)?;
            
            // Find the next occurrence after the completed task's due date
            let next_occurrence = if let Some(due_at) = completed_task.due_at {
                recurrence_manager.next_occurrence_after(due_at)?
            } else {
                recurrence_manager.next_occurrence_after(Utc::now())?
            };

            // If there's a next occurrence, check if it needs to be materialized
            let next_task = if let Some(next_due) = next_occurrence {
                // Calculate materialization window
                let (window_start, window_end) = self.materialization_manager().calculate_window_for_filters(&[]);
                
                // Check if next occurrence is within materialization window
                if next_due >= window_start && next_due <= window_end {
                    // Check if it's already materialized
                    let existing_task: Option<Task> = sqlx::query_as(
                        "SELECT * FROM tasks WHERE series_id = $1 AND due_at = $2"
                    )
                    .bind(series_id)
                    .bind(next_due)
                    .fetch_optional(&mut *tx)
                    .await?;

                    if existing_task.is_none() {
                        // Materialize the next occurrence
                        self.refresh_single_series_materialization(
                            series_id, 
                            next_due - chrono::Duration::minutes(1), 
                            next_due + chrono::Duration::minutes(1)
                        ).await?;

                        // Fetch the newly created task
                        sqlx::query_as(
                            "SELECT * FROM tasks WHERE series_id = $1 AND due_at = $2"
                        )
                        .bind(series_id)
                        .bind(next_due)
                        .fetch_optional(&mut *tx)
                        .await?
                    } else {
                        existing_task
                    }
                } else {
                    None
                }
            } else {
                None
            };

            tx.commit().await?;
            
            Ok(CompletionResult::SeriesInstance {
                completed: completed_task,
                next: next_task,
                series_id,
                next_occurrence,
            })
        } else {
            // Regular task completion
            tx.commit().await?;
            Ok(CompletionResult::Single(completed_task))
        }
    }

    async fn cancel_task(&self, id: Uuid) -> Result<Task, CoreError> {
        let updated_task: Task = sqlx::query_as(
            r#"UPDATE tasks
            SET status = $1, updated_at = $2
            WHERE id = $3
            RETURNING *
            "#,
        )
        .bind(TaskStatus::Cancelled)
        .bind(Utc::now())
        .bind(id)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        Ok(updated_task)
    }

    async fn update_task(&self, id: Uuid, data: UpdateTaskData, scope: Option<crate::models::EditScope>) -> Result<Task, CoreError> {
        let mut tx = self.pool().begin().await?;

        let current_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        // Dispatch based on series membership and edit scope
        match (current_task.series_id, scope.unwrap_or(crate::models::EditScope::ThisOccurrence)) {
            (None, _) => {
                // Regular task - validate no recurrence changes
                self.update_regular_task(&mut tx, id, &data).await?;
            }
            (Some(_), crate::models::EditScope::ThisOccurrence) => {
                // Single occurrence edit
                self.update_single_occurrence(&mut tx, id, &data).await?;
            }
            (Some(_series_id), crate::models::EditScope::ThisAndFuture) => {
                // Update from this occurrence forward
                self.update_series_from_future(&mut tx, &current_task, &data).await?;
            }
            (Some(series_id), crate::models::EditScope::EntireSeries) => {
                // Update entire series
                self.update_entire_series(&mut tx, series_id, &data).await?;
            }
        }

        let updated_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(updated_task)
    }
}

impl SqliteRepository {
    /// Updates a regular (non-series) task with validation
    async fn update_regular_task<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        task_id: Uuid,
        data: &UpdateTaskData,
    ) -> Result<(), CoreError> {
        if data.rrule.is_some() || data.timezone.is_some() {
            return Err(CoreError::InvalidInput(
                "Cannot add recurrence to existing task. Create a new recurring task instead".to_string()
            ));
        }
        
        Self::update_task_fields(tx, task_id, data).await
    }

    /// Updates a single task occurrence with validation
    async fn update_single_occurrence<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        task_id: Uuid,
        data: &UpdateTaskData,
    ) -> Result<(), CoreError> {
        if data.rrule.is_some() || data.timezone.is_some() {
            return Err(CoreError::InvalidInput(
                "Cannot modify recurrence for single occurrence. Use EditScope::ThisAndFuture or EditScope::EntireSeries".to_string()
            ));
        }
        
        Self::update_task_fields(tx, task_id, data).await
    }

    /// Updates series and re-materializes instances from a specific point forward
    async fn update_series_from_future<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        current_task: &Task,
        data: &UpdateTaskData,
    ) -> Result<(), CoreError> {
        let series_id = current_task.series_id.unwrap();
        
        // Update series metadata
        self.update_series_metadata(tx, series_id, data).await?;
        
        // Update template task
        self.update_template_task(tx, series_id, data).await?;
        
        // Clean future instances and reset materialization boundary
        if let Some(due_at) = current_task.due_at {
            self.clean_future_instances(tx, series_id, due_at).await?;
            self.reset_materialization_boundary(tx, series_id, Some(due_at)).await?;
        }
        
        Ok(())
    }

    /// Updates entire series and re-materializes all instances
    async fn update_entire_series<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        series_id: Uuid,
        data: &UpdateTaskData,
    ) -> Result<(), CoreError> {
        // Update series metadata
        self.update_series_metadata(tx, series_id, data).await?;
        
        // Update template task
        self.update_template_task(tx, series_id, data).await?;
        
        // Clean all instances and reset materialization
        self.clean_all_instances(tx, series_id).await?;
        self.reset_materialization_boundary(tx, series_id, None).await?;
        
        Ok(())
    }

    /// Updates series recurrence metadata (rrule, timezone)
    async fn update_series_metadata<'a>(
        &self,
        _tx: &mut Transaction<'a, Sqlite>,
        series_id: Uuid,
        data: &UpdateTaskData,
    ) -> Result<(), CoreError> {
        if data.rrule.is_some() || data.timezone.is_some() {
            let mut series_update = crate::models::UpdateSeriesData::default();
            if let Some(rrule) = &data.rrule {
                series_update.rrule = rrule.clone();
            }
            if let Some(timezone) = &data.timezone {
                series_update.timezone = timezone.clone();
            }
            self.update_series(series_id, series_update).await?;
        }
        Ok(())
    }

    /// Updates template task with non-recurrence fields
    async fn update_template_task<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        series_id: Uuid,
        data: &UpdateTaskData,
    ) -> Result<(), CoreError> {
        let series: crate::models::TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&mut **tx)
            .await?;

        let mut template_update = data.clone();
        template_update.rrule = None;    // Don't update rrule on template
        template_update.timezone = None; // Don't update timezone on template
        
        Self::update_task_fields(tx, series.template_task_id, &template_update).await
    }

    /// Cleans future instances from a specific date forward
    async fn clean_future_instances<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        series_id: Uuid,
        from_date: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        let series: crate::models::TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&mut **tx)
            .await?;

        sqlx::query("DELETE FROM tasks WHERE series_id = $1 AND due_at >= $2 AND id != $3")
            .bind(series_id)
            .bind(from_date)
            .bind(series.template_task_id) // Don't delete template
            .execute(&mut **tx)
            .await?;
        
        Ok(())
    }

    /// Cleans all instances (except template) for a series
    async fn clean_all_instances<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        series_id: Uuid,
    ) -> Result<(), CoreError> {
        let series: crate::models::TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&mut **tx)
            .await?;

        sqlx::query("DELETE FROM tasks WHERE series_id = $1 AND id != $2")
            .bind(series_id)
            .bind(series.template_task_id) // Don't delete template
            .execute(&mut **tx)
            .await?;
        
        Ok(())
    }

    /// Resets materialization boundary to trigger re-materialization
    async fn reset_materialization_boundary<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        series_id: Uuid,
        boundary_date: Option<DateTime<Utc>>,
    ) -> Result<(), CoreError> {
        match boundary_date {
            Some(date) => {
                // Reset to one day before the boundary to trigger re-materialization
                sqlx::query("UPDATE task_series SET last_materialized_until = $1 WHERE id = $2")
                    .bind(date - chrono::Duration::days(1))
                    .bind(series_id)
                    .execute(&mut **tx)
                    .await?;
            }
            None => {
                // Reset to NULL for full re-materialization
                sqlx::query("UPDATE task_series SET last_materialized_until = NULL WHERE id = $1")
                    .bind(series_id)
                    .execute(&mut **tx)
                    .await?;
            }
        }
        
        Ok(())
    }
}

impl SqliteRepository {
    /// Add a task within an existing transaction
    pub(crate) async fn add_task_in_transaction<'a>(
        tx: &mut Transaction<'a, Sqlite>,
        mut data: NewTaskData,
    ) -> Result<Task, CoreError> {
        if data.project_id.is_none() {
            if let Some(project_name) = &data.project_name {
                let project: Option<Project> =
                    sqlx::query_as("SELECT * FROM projects WHERE name = $1")
                        .bind(project_name)
                        .fetch_optional(&mut **tx)
                        .await?;
                data.project_id = Some(
                    project
                        .map(|p| p.id)
                        .ok_or_else(|| CoreError::NotFound(project_name.clone()))?,
                );
            }
        }

        let task = Task {
            id: Uuid::now_v7(),
            name: data.name,
            description: data.description,
            status: TaskStatus::Pending,
            priority: data.priority.unwrap_or(TaskPriority::None),
            due_at: data.due_at,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            project_id: data.project_id,
            parent_id: data.parent_id,
            series_id: data.series_id,
        };

        sqlx::query(
            r#"INSERT INTO tasks (id, name, description, status, priority, due_at, created_at, updated_at, project_id, parent_id, series_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(task.id)
        .bind(&task.name)
        .bind(&task.description)
        .bind(&task.status)
        .bind(&task.priority)
        .bind(task.due_at)
        .bind(task.created_at)
        .bind(task.updated_at)
        .bind(task.project_id)
        .bind(task.parent_id)
        .bind(task.series_id)
        .execute(&mut **tx)
        .await?;

        if let Some(depends_on_id) = data.depends_on {
            if task.id == depends_on_id {
                return Err(CoreError::InvalidInput(
                    "A task cannot depend on itself.".to_string(),
                ));
            }

            if Self::path_exists(&mut *tx, depends_on_id, task.id).await? {
                let depends_on_task_name = Self::find_task_by_id_in_transaction(&mut *tx, depends_on_id)
                    .await?
                    .map(|t| t.name)
                    .unwrap_or_else(|| depends_on_id.to_string());
                return Err(CoreError::CircularDependency(
                    task.name.clone(),
                    depends_on_task_name,
                ));
            }

            sqlx::query("INSERT INTO task_dependencies (task_id, depends_on_id) VALUES ($1, $2)")
                .bind(task.id)
                .bind(depends_on_id)
                .execute(&mut **tx)
                .await?;
        }

        let tags = data.tags;
        if !tags.is_empty() {
            let mut query_builder: QueryBuilder<sqlx::Sqlite> =
                QueryBuilder::new("INSERT INTO task_tags (task_id, tag_name) ");
            query_builder.push_values(tags.iter(), |mut b, tag| {
                b.push_bind(task.id).push_bind(tag);
            });
            query_builder.build().execute(&mut **tx).await?;
        }

        Ok(task)
    }

    /// Find a task by ID within an existing transaction
    pub(crate) async fn find_task_by_id_in_transaction<'a>(
        tx: &mut Transaction<'a, Sqlite>,
        id: Uuid,
    ) -> Result<Option<Task>, CoreError> {
        let task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
        Ok(task)
    }

    /// Update task fields within an existing transaction
    pub(crate) async fn update_task_fields<'a>(
        tx: &mut Transaction<'a, Sqlite>,
        id: Uuid,
        data: &UpdateTaskData,
    ) -> Result<(), CoreError> {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("UPDATE tasks SET ");
        let mut updated = false;

        if let Some(name) = &data.name {
            qb.push("name = ");
            qb.push_bind(name);
            updated = true;
        }

        if let Some(description) = &data.description {
            if updated {
                qb.push(", ");
            }
            qb.push("description = ");
            qb.push_bind(description);
            updated = true;
        }

        if let Some(due_at) = &data.due_at {
            if updated {
                qb.push(", ");
            }
            qb.push("due_at = ");
            qb.push_bind(due_at);
            updated = true;
        }

        if let Some(priority) = &data.priority {
            if updated {
                qb.push(", ");
            }
            qb.push("priority = ");
            qb.push_bind(priority);
            updated = true;
        }

        if let Some(status) = &data.status {
            if updated {
                qb.push(", ");
            }
            qb.push("status = ");
            qb.push_bind(status);
            updated = true;
        }

        if let Some(parent_id) = &data.parent_id {
            if updated {
                qb.push(", ");
            }
            qb.push("parent_id = ");
            qb.push_bind(parent_id);
            updated = true;
        }

        if let Some(series_id) = &data.series_id {
            if updated {
                qb.push(", ");
            }
            qb.push("series_id = ");
            qb.push_bind(series_id);
            updated = true;
        }

        if let Some(project_name_option) = &data.project_name {
            let project_id = match project_name_option {
                Some(project_name) => {
                    let project: Option<Project> =
                        sqlx::query_as("SELECT * FROM projects WHERE name = $1")
                            .bind(project_name.clone())
                            .fetch_optional(&mut **tx)
                            .await?;
                    Some(
                        project
                            .map(|p| p.id)
                            .ok_or_else(|| CoreError::NotFound(project_name.clone()))?,
                    )
                }
                None => None,
            };
            if updated {
                qb.push(", ");
            }
            qb.push("project_id = ");
            qb.push_bind(project_id);
            updated = true;
        }

        if let Some(depends_on_option) = &data.depends_on {
            // First, remove any existing dependency for this task.
            sqlx::query("DELETE FROM task_dependencies WHERE task_id = $1")
                .bind(id)
                .execute(&mut **tx)
                .await?;

            if let Some(depends_on_id) = depends_on_option {
                // A new dependency is being set.
                if id == *depends_on_id {
                    return Err(CoreError::InvalidInput(
                        "A task cannot depend on itself.".to_string(),
                    ));
                }

                if Self::path_exists(&mut *tx, *depends_on_id, id).await? {
                    let task_name = Self::find_task_by_id_in_transaction(&mut *tx, id)
                        .await?
                        .map(|t| t.name)
                        .unwrap_or_else(|| id.to_string());
                    let depends_on_task_name = Self::find_task_by_id_in_transaction(&mut *tx, *depends_on_id)
                        .await?
                        .map(|t| t.name)
                        .unwrap_or_else(|| depends_on_id.to_string());
                    return Err(CoreError::CircularDependency(
                        task_name,
                        depends_on_task_name,
                    ));
                }

                sqlx::query("INSERT INTO task_dependencies (task_id, depends_on_id) VALUES ($1, $2)")
                    .bind(id)
                    .bind(depends_on_id)
                    .execute(&mut **tx)
                    .await?;
            }
            // If depends_on_option is None, the dependency is just cleared, which we already did.
            updated = true;
        }

        if let Some(tags_to_add) = &data.add_tags {
            if !tags_to_add.is_empty() {
                let mut query_builder: QueryBuilder<sqlx::Sqlite> =
                    QueryBuilder::new("INSERT OR IGNORE INTO task_tags (task_id, tag_name) ");
                query_builder.push_values(tags_to_add.iter(), |mut b, tag| {
                    b.push_bind(id).push_bind(tag);
                });
                query_builder.build().execute(&mut **tx).await?;
            }
        }

        if let Some(tags_to_remove) = &data.remove_tags {
            if !tags_to_remove.is_empty() {
                let mut query_builder: QueryBuilder<sqlx::Sqlite> =
                    QueryBuilder::new("DELETE FROM task_tags WHERE task_id = $1 AND tag_name IN (");
                query_builder.push_bind(id);
                let mut separated = query_builder.separated(", ");
                for tag in tags_to_remove.iter() {
                    separated.push_bind(tag);
                }
                separated.push_unseparated(")");
                query_builder.build().execute(&mut **tx).await?;
            }
        }

        if updated {
            qb.push(", updated_at = ");
            qb.push_bind(Utc::now());
            qb.push(" WHERE id = ");
            qb.push_bind(id);
            qb.build().execute(&mut **tx).await?;
        }

        Ok(())
    }

    /// Check if a dependency path exists from start_node to end_node (for circular dependency detection)
    pub(crate) async fn path_exists<'a>(
        tx: &mut Transaction<'a, Sqlite>,
        start_node_id: Uuid,
        end_node_id: Uuid,
    ) -> Result<bool, CoreError> {
        let path_found: Option<i32> = sqlx::query_scalar(
            r#"
            WITH RECURSIVE dependency_path (id) AS (
                SELECT depends_on_id FROM task_dependencies WHERE task_id = $1
                UNION ALL
                SELECT td.depends_on_id
                FROM task_dependencies td
                JOIN dependency_path dp ON td.task_id = dp.id
                WHERE td.depends_on_id IS NOT NULL
            )
            SELECT 1 FROM dependency_path WHERE id = $2 LIMIT 1;
            "#,
        )
        .bind(start_node_id)
        .bind(end_node_id)
        .fetch_optional(&mut **tx)
        .await?;

        Ok(path_found.is_some())
    }
}