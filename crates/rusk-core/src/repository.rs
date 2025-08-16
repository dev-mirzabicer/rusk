use crate::db::DbPool;
use crate::error::CoreError;
use crate::models::{
    CompletionResult, NewTaskData, Project, Task, TaskPriority, TaskStatus,
    UpdateTaskData, TaskSeries, SeriesException, NewSeriesData, UpdateSeriesData, 
    NewSeriesException, EditScope, ExceptionType, SeriesStatistics,
};
use crate::query::{Filter, Operator, Query};
use crate::recurrence::{RecurrenceManager, MaterializationManager};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, QueryBuilder, Sqlite, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct TaskQueryResult {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub due_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub project_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub series_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub depth: i32,
    pub tags: Option<String>,
}

#[async_trait]
pub trait Repository {
    async fn add_task(&self, data: NewTaskData) -> Result<Task, CoreError>;
    async fn find_task_by_id(&self, id: Uuid) -> Result<Option<Task>, CoreError>;
    async fn find_tasks_by_short_id_prefix(&self, short_id: &str) -> Result<Vec<Task>, CoreError>;
    async fn find_tasks_with_details(
        &self,
        query: &Query,
    ) -> Result<Vec<TaskQueryResult>, CoreError>;
    async fn delete_task(&self, id: Uuid) -> Result<(), CoreError>;
    async fn complete_task(&self, id: Uuid) -> Result<CompletionResult, CoreError>;
    async fn cancel_task(&self, id: Uuid) -> Result<Task, CoreError>;
    async fn update_task(&self, id: Uuid, data: UpdateTaskData, scope: Option<EditScope>) -> Result<Task, CoreError>;
    async fn add_project(
        &self,
        name: String,
        description: Option<String>,
    ) -> Result<Project, CoreError>;
    async fn find_project_by_id(&self, id: Uuid) -> Result<Option<Project>, CoreError>;
    async fn find_project_by_name(&self, name: &str) -> Result<Option<Project>, CoreError>;
    async fn find_projects(&self) -> Result<Vec<Project>, CoreError>;
    async fn delete_project(&self, name: String) -> Result<(), CoreError>;
    
    // Series Management Methods (Phase 3)
    async fn create_series(&self, data: NewSeriesData) -> Result<TaskSeries, CoreError>;
    async fn find_series_by_id(&self, id: Uuid) -> Result<Option<TaskSeries>, CoreError>;
    async fn find_series_by_template(&self, template_id: Uuid) -> Result<Option<TaskSeries>, CoreError>;
    async fn update_series(&self, id: Uuid, data: UpdateSeriesData) -> Result<TaskSeries, CoreError>;
    async fn delete_series(&self, id: Uuid) -> Result<(), CoreError>;
    async fn find_active_series(&self) -> Result<Vec<TaskSeries>, CoreError>;
    
    // Exception Management Methods (Phase 3)
    async fn add_series_exception(&self, exception: NewSeriesException) -> Result<SeriesException, CoreError>;
    async fn find_series_exceptions(&self, series_id: Uuid) -> Result<Vec<SeriesException>, CoreError>;
    async fn remove_series_exception(&self, series_id: Uuid, occurrence_dt: DateTime<Utc>) -> Result<(), CoreError>;
    
    // Advanced Exception Management Methods (Phase 5)
    async fn add_bulk_series_exceptions(&self, exceptions: Vec<NewSeriesException>) -> Result<Vec<SeriesException>, CoreError>;
    async fn remove_bulk_series_exceptions(&self, series_id: Uuid, occurrence_dts: Vec<DateTime<Utc>>) -> Result<usize, CoreError>;
    async fn validate_exception_conflicts(&self, series_id: Uuid, new_exception: &NewSeriesException) -> Result<Vec<SeriesException>, CoreError>;
    async fn override_occurrence_with_task(&self, series_id: Uuid, occurrence_dt: DateTime<Utc>, override_task_data: NewTaskData) -> Result<Task, CoreError>;
    async fn move_occurrence_with_validation(&self, series_id: Uuid, from_dt: DateTime<Utc>, to_dt: DateTime<Utc>, timezone: &str) -> Result<Task, CoreError>;
    
    // Series Management Methods (Phase 5)
    async fn duplicate_series(&self, series_id: Uuid, new_name: String, new_timezone: Option<String>) -> Result<TaskSeries, CoreError>;
    async fn archive_completed_series(&self, series_id: Uuid) -> Result<(), CoreError>;
    async fn bulk_update_series(&self, updates: Vec<(Uuid, UpdateSeriesData)>) -> Result<Vec<TaskSeries>, CoreError>;
    async fn find_series_by_pattern(&self, pattern: &str) -> Result<Vec<TaskSeries>, CoreError>;
    async fn get_series_statistics(&self, series_id: Uuid) -> Result<SeriesStatistics, CoreError>;
    
    // Materialization Support Methods (Phase 3)
    async fn refresh_series_materialization(&self, window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> Result<(), CoreError>;
    async fn find_materialized_tasks_for_series(&self, series_id: Uuid, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Task>, CoreError>;
    async fn update_series_materialization_boundary(&self, series_id: Uuid, boundary: DateTime<Utc>) -> Result<(), CoreError>;
}

pub struct SqliteRepository {
    pool: DbPool,
    materialization_manager: MaterializationManager,
}

impl SqliteRepository {
    pub fn new(pool: DbPool, materialization_manager: MaterializationManager) -> Self {
        Self { pool, materialization_manager }
    }

    fn build_sql_where_clause<'a>(
        &self,
        query: &Query,
        qb: &mut QueryBuilder<'a, Sqlite>,
    ) {
        match query {
            Query::Filter(filter) => match filter {
                Filter::Project(name) => {
                    qb.push("p.name = ");
                    qb.push_bind(name.clone());
                }
                Filter::Status(status) => {
                    qb.push("th.status = ");
                    qb.push_bind(status.clone());
                }
                Filter::Priority(priority) => {
                    qb.push("th.priority = ");
                    qb.push_bind(priority.clone());
                }
                Filter::Tags(tag_filter) => {
                    use crate::query::TagFilter;
                    
                    match tag_filter {
                        TagFilter::Has(tag) => {
                            qb.push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name = ");
                            qb.push_bind(tag.clone());
                            qb.push(")");
                        }
                        TagFilter::HasAll(tags) => {
                            qb.push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name IN (");
                            for (i, tag) in tags.iter().enumerate() {
                                if i > 0 {
                                    qb.push(", ");
                                }
                                qb.push_bind(tag.clone());
                            }
                            qb.push(") GROUP BY task_id HAVING COUNT(DISTINCT tag_name) = ");
                            qb.push_bind(tags.len() as i64);
                            qb.push(")");
                        }
                        TagFilter::HasAny(tags) => {
                            qb.push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name IN (");
                            for (i, tag) in tags.iter().enumerate() {
                                if i > 0 {
                                    qb.push(", ");
                                }
                                qb.push_bind(tag.clone());
                            }
                            qb.push("))");
                        }
                        TagFilter::Exact(tags) => {
                            // Tasks that have exactly these tags (no more, no less)
                            qb.push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name IN (");
                            for (i, tag) in tags.iter().enumerate() {
                                if i > 0 {
                                    qb.push(", ");
                                }
                                qb.push_bind(tag.clone());
                            }
                            qb.push(") GROUP BY task_id HAVING COUNT(tag_name) = ");
                            qb.push_bind(tags.len() as i64);
                            qb.push(") AND th.id NOT IN (SELECT task_id FROM task_tags WHERE tag_name NOT IN (");
                            for (i, tag) in tags.iter().enumerate() {
                                if i > 0 {
                                    qb.push(", ");
                                }
                                qb.push_bind(tag.clone());
                            }
                            qb.push("))");
                        }
                        TagFilter::NotHas(tag) => {
                            qb.push("th.id NOT IN (SELECT task_id FROM task_tags WHERE tag_name = ");
                            qb.push_bind(tag.clone());
                            qb.push(")");
                        }
                        TagFilter::NotHasAny(tags) => {
                            qb.push("th.id NOT IN (SELECT task_id FROM task_tags WHERE tag_name IN (");
                            for (i, tag) in tags.iter().enumerate() {
                                if i > 0 {
                                    qb.push(", ");
                                }
                                qb.push_bind(tag.clone());
                            }
                            qb.push("))");
                        }
                    }
                }
                Filter::Name(text_filter) => {
                    use crate::query::TextFilter;
                    
                    match text_filter {
                        TextFilter::Contains(text) => {
                            qb.push("LOWER(th.name) LIKE LOWER(");
                            // Optimize: avoid format! allocation
                            let mut pattern = String::with_capacity(text.len() + 2);
                            pattern.push('%');
                            pattern.push_str(text);
                            pattern.push('%');
                            qb.push_bind(pattern);
                            qb.push(")");
                        }
                        TextFilter::Equals(text) => {
                            qb.push("LOWER(th.name) = LOWER(");
                            qb.push_bind(text.clone());
                            qb.push(")");
                        }
                        TextFilter::StartsWith(text) => {
                            qb.push("LOWER(th.name) LIKE LOWER(");
                            // Optimize: avoid format! allocation
                            let mut pattern = String::with_capacity(text.len() + 1);
                            pattern.push_str(text);
                            pattern.push('%');
                            qb.push_bind(pattern);
                            qb.push(")");
                        }
                        TextFilter::EndsWith(text) => {
                            qb.push("LOWER(th.name) LIKE LOWER(");
                            // Optimize: avoid format! allocation  
                            let mut pattern = String::with_capacity(text.len() + 1);
                            pattern.push('%');
                            pattern.push_str(text);
                            qb.push_bind(pattern);
                            qb.push(")");
                        }
                        TextFilter::NotContains(text) => {
                            qb.push("LOWER(th.name) NOT LIKE LOWER(");
                            // Optimize: avoid format! allocation
                            let mut pattern = String::with_capacity(text.len() + 2);
                            pattern.push('%');
                            pattern.push_str(text);
                            pattern.push('%');
                            qb.push_bind(pattern);
                            qb.push(")");
                        }
                    }
                }
                Filter::Description(text_filter) => {
                    use crate::query::TextFilter;
                    
                    match text_filter {
                        TextFilter::Contains(text) => {
                            qb.push("LOWER(th.description) LIKE LOWER(");
                            qb.push_bind(format!("%{}%", text));
                            qb.push(")");
                        }
                        TextFilter::Equals(text) => {
                            qb.push("LOWER(th.description) = LOWER(");
                            qb.push_bind(text.clone());
                            qb.push(")");
                        }
                        TextFilter::StartsWith(text) => {
                            qb.push("LOWER(th.description) LIKE LOWER(");
                            qb.push_bind(format!("{}%", text));
                            qb.push(")");
                        }
                        TextFilter::EndsWith(text) => {
                            qb.push("LOWER(th.description) LIKE LOWER(");
                            qb.push_bind(format!("%{}", text));
                            qb.push(")");
                        }
                        TextFilter::NotContains(text) => {
                            qb.push("LOWER(th.description) NOT LIKE LOWER(");
                            qb.push_bind(format!("%{}%", text));
                            qb.push(")");
                        }
                    }
                }
                Filter::Due(due_date) => {
                    use crate::query::DueDate;
                    use chrono::Utc;
                    
                    match due_date {
                        DueDate::On(date_time) => {
                            qb.push("DATE(th.due_at) = DATE(");
                            qb.push_bind(date_time.clone());
                            qb.push(")");
                        }
                        DueDate::Before(date_time) => {
                            qb.push("th.due_at < ");
                            qb.push_bind(date_time.clone());
                        }
                        DueDate::After(date_time) => {
                            qb.push("th.due_at > ");
                            qb.push_bind(date_time.clone());
                        }
                        DueDate::Today => {
                            qb.push("DATE(th.due_at) = DATE('now')");
                        }
                        DueDate::Tomorrow => {
                            qb.push("DATE(th.due_at) = DATE('now', '+1 day')");
                        }
                        DueDate::Yesterday => {
                            qb.push("DATE(th.due_at) = DATE('now', '-1 day')");
                        }
                        DueDate::Overdue => {
                            qb.push("th.due_at < datetime('now') AND th.status = 'pending'");
                        }
                        DueDate::Within(duration) => {
                            let target_date = Utc::now() + *duration;
                            qb.push("th.due_at BETWEEN datetime('now') AND ");
                            qb.push_bind(target_date);
                        }
                        DueDate::Ago(duration) => {
                            let start_date = Utc::now() - *duration;
                            qb.push("th.due_at BETWEEN ");
                            qb.push_bind(start_date);
                            qb.push(" AND datetime('now')");
                        }
                    }
                }
            },
            Query::Not(query) => {
                qb.push("NOT (");
                self.build_sql_where_clause(query, qb);
                qb.push(")");
            }
            Query::Binary { op, left, right } => {
                qb.push("(");
                self.build_sql_where_clause(left, qb);
                match op {
                    Operator::And => qb.push(") AND ("),
                    Operator::Or => qb.push(") OR ("),
                };
                self.build_sql_where_clause(right, qb);
                qb.push(")");
            }
        }
    }

    async fn find_task_by_id_in_transaction<'a>(
        &self,
        tx: &mut Transaction<'a, Sqlite>,
        id: Uuid,
    ) -> Result<Option<Task>, CoreError> {
        let task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
        Ok(task)
    }

    async fn add_task_in_transaction<'a>(
        &self,
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

            if self
                .path_exists(&mut *tx, depends_on_id, task.id)
                .await?
            {
                let depends_on_task_name = self
                    .find_task_by_id_in_transaction(&mut *tx, depends_on_id)
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

    /// Checks if a dependency path exists from a start_node to an end_node.
    ///
    /// This is used to detect circular dependencies. If a path exists from B to A,
    /// then adding a dependency A -> B would create a cycle.
    async fn path_exists<'a>(
        &self,
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

#[async_trait]
impl Repository for SqliteRepository {
    async fn add_task(&self, data: NewTaskData) -> Result<Task, CoreError> {
        let mut tx = self.pool.begin().await?;
        
        // Check if this is a recurring task
        if let Some(rrule) = &data.rrule {
            // Create template task first
            let mut template_data = data.clone();
            template_data.rrule = None; // Remove rrule for template task
            template_data.series_id = None; // Template tasks don't have series_id
            
            let template_task = self.add_task_in_transaction(&mut tx, template_data).await?;
            
            // Create the series
            let series_data = NewSeriesData {
                template_task_id: template_task.id,
                rrule: rrule.clone(),
                dtstart: data.due_at.unwrap_or_else(|| Utc::now()),
                timezone: data.timezone.unwrap_or_else(|| "UTC".to_string()),
            };
            
            // Create series using internal method
            let series = self.create_series_in_transaction(&mut tx, series_data).await?;
            
            // Trigger initial materialization for lookahead window
            let (window_start, window_end) = self.materialization_manager.calculate_window_for_filters(&[]);
            self.refresh_single_series_materialization_in_transaction(&mut tx, series.id, window_start, window_end).await?;
            
            tx.commit().await?;
            Ok(template_task)
        } else {
            // Regular task
            let task = self.add_task_in_transaction(&mut tx, data).await?;
            tx.commit().await?;
            Ok(task)
        }
    }

    async fn find_task_by_id(&self, id: Uuid) -> Result<Option<Task>, CoreError> {
        let task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
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
            .fetch_all(&self.pool)
            .await?;
        Ok(tasks)
    }

    async fn find_tasks_with_details(
        &self,
        query: &Query,
    ) -> Result<Vec<TaskQueryResult>, CoreError> {
        // PHASE 3: Add materialization hooks to ensure current data
        // Calculate window based on filters and trigger materialization if needed
        let (window_start, window_end) = self.materialization_manager.calculate_window_for_filters(&self.extract_filters_from_query(query));
        
        // Trigger materialization for all active series within the window
        self.refresh_series_materialization(window_start, window_end).await?;

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
        self.build_sql_where_clause(query, &mut query_builder);

        query_builder.push(" GROUP BY th.id, th.name, th.description, th.status, th.priority, th.due_at, th.completed_at, th.created_at, th.updated_at, th.project_id, th.parent_id, th.series_id, th.depth, th.path, p.name");
        query_builder.push(" ORDER BY th.path");

        let tasks = query_builder.build_query_as().fetch_all(&self.pool).await?;
        Ok(tasks)
    }

    async fn delete_task(&self, id: Uuid) -> Result<(), CoreError> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn complete_task(&self, id: Uuid) -> Result<CompletionResult, CoreError> {
        let mut tx = self.pool.begin().await?;

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
                let (window_start, window_end) = self.materialization_manager.calculate_window_for_filters(&[]);
                
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
                        // Need to materialize the next occurrence
                        self.refresh_single_series_materialization_in_transaction(
                            &mut tx, 
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
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        Ok(updated_task)
    }

    async fn update_task(&self, id: Uuid, data: UpdateTaskData, scope: Option<EditScope>) -> Result<Task, CoreError> {
        let mut tx = self.pool.begin().await?;

        // Get the current task to determine if it's part of a series
        let current_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        // Determine the edit scope and handle series-aware updates
        if let Some(series_id) = current_task.series_id {
            // This is a series instance, handle scope-aware editing
            let edit_scope = scope.unwrap_or(EditScope::ThisOccurrence);
            
            match edit_scope {
                EditScope::ThisOccurrence => {
                    // Create an exception for this specific occurrence
                    if data.rrule.is_some() || data.timezone.is_some() {
                        return Err(CoreError::InvalidInput(
                            "Cannot modify recurrence for single occurrence. Use EditScope::ThisAndFuture or EditScope::EntireSeries".to_string()
                        ));
                    }
                    
                    // Update this task instance only
                    self.update_task_fields(&mut tx, id, &data).await?;
                }
                EditScope::ThisAndFuture | EditScope::EntireSeries => {
                    // Update the series and re-materialize affected instances
                    let series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
                        .bind(series_id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .ok_or_else(|| CoreError::NotFound(format!("Series with id {} not found", series_id)))?;

                    // Build UpdateSeriesData from UpdateTaskData
                    let mut series_update = UpdateSeriesData::default();
                    
                    if let Some(rrule) = data.rrule.as_ref() {
                        series_update.rrule = rrule.clone();
                    }
                    
                    if let Some(timezone) = data.timezone.as_ref() {
                        series_update.timezone = timezone.clone();
                    }

                    // Update the series if needed
                    if series_update.rrule.is_some() || series_update.timezone.is_some() {
                        self.update_series_in_transaction(&mut tx, series_id, series_update).await?;
                    }

                    // Update the template task with non-recurrence fields
                    let mut template_update = data.clone();
                    template_update.rrule = None; // Don't update rrule on template
                    template_update.timezone = None; // Don't update timezone on template
                    
                    self.update_task_fields(&mut tx, series.template_task_id, &template_update).await?;

                    // Re-materialize instances based on scope
                    match edit_scope {
                        EditScope::ThisAndFuture => {
                            // Delete future instances and re-materialize
                            if let Some(due_at) = current_task.due_at {
                                sqlx::query("DELETE FROM tasks WHERE series_id = $1 AND due_at >= $2 AND id != $3")
                                    .bind(series_id)
                                    .bind(due_at)
                                    .bind(series.template_task_id) // Don't delete template
                                    .execute(&mut *tx)
                                    .await?;
                                
                                // Reset materialization boundary to trigger re-materialization
                                sqlx::query("UPDATE task_series SET last_materialized_until = $1 WHERE id = $2")
                                    .bind(due_at - chrono::Duration::days(1))
                                    .bind(series_id)
                                    .execute(&mut *tx)
                                    .await?;
                            }
                        }
                        EditScope::EntireSeries => {
                            // Delete all instances and re-materialize
                            sqlx::query("DELETE FROM tasks WHERE series_id = $1 AND id != $2")
                                .bind(series_id)
                                .bind(series.template_task_id) // Don't delete template
                                .execute(&mut *tx)
                                .await?;
                            
                            // Reset materialization boundary to trigger full re-materialization
                            sqlx::query("UPDATE task_series SET last_materialized_until = NULL WHERE id = $1")
                                .bind(series_id)
                                .execute(&mut *tx)
                                .await?;
                        }
                        _ => unreachable!()
                    }
                }
            }
        } else {
            // Regular task or template task, standard update
            if data.rrule.is_some() || data.timezone.is_some() {
                return Err(CoreError::InvalidInput(
                    "Cannot add recurrence to existing task. Create a new recurring task instead".to_string()
                ));
            }
            
            self.update_task_fields(&mut tx, id, &data).await?;
        }

        let updated_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(updated_task)
    }

    async fn add_project(
        &self,
        name: String,
        description: Option<String>,
    ) -> Result<Project, CoreError> {
        let project_id = Uuid::now_v7();
        let project = sqlx::query_as(
            r#"INSERT INTO projects (id, name, description)
            VALUES ($1, $2, $3)
            RETURNING id, name, description, created_at
            "#,
        )
        .bind(project_id)
        .bind(name)
        .bind(description)
        .fetch_one(&self.pool)
        .await?;

        Ok(project)
    }

    async fn find_project_by_id(&self, id: Uuid) -> Result<Option<Project>, CoreError> {
        let project = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(project)
    }

    async fn find_project_by_name(&self, name: &str) -> Result<Option<Project>, CoreError> {
        let project = sqlx::query_as("SELECT * FROM projects WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(project)
    }

    async fn find_projects(&self) -> Result<Vec<Project>, CoreError> {
        let projects = sqlx::query_as("SELECT id, name, description, created_at FROM projects")
            .fetch_all(&self.pool)
            .await?;
        Ok(projects)
    }

    async fn delete_project(&self, name: String) -> Result<(), CoreError> {
        let result = sqlx::query("DELETE FROM projects WHERE name = $1")
            .bind(name)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound("Project not found".to_string()));
        }
        Ok(())
    }

    // ============================================================================
    // Series Management Methods (Phase 3)
    // ============================================================================

    async fn create_series(&self, data: NewSeriesData) -> Result<TaskSeries, CoreError> {
        let mut tx = self.pool.begin().await?;

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
            .fetch_optional(&self.pool)
            .await?;
        Ok(series)
    }

    async fn find_series_by_template(&self, template_id: Uuid) -> Result<Option<TaskSeries>, CoreError> {
        let series = sqlx::query_as("SELECT * FROM task_series WHERE template_task_id = $1")
            .bind(template_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(series)
    }

    async fn find_active_series(&self) -> Result<Vec<TaskSeries>, CoreError> {
        let series = sqlx::query_as("SELECT * FROM task_series WHERE active = true ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;
        Ok(series)
    }

    async fn update_series(&self, id: Uuid, data: UpdateSeriesData) -> Result<TaskSeries, CoreError> {
        let mut tx = self.pool.begin().await?;

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
        let mut tx = self.pool.begin().await?;

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

    // ============================================================================
    // Exception Management Methods (Phase 3)
    // ============================================================================

    async fn add_series_exception(&self, exception: NewSeriesException) -> Result<SeriesException, CoreError> {
        let mut tx = self.pool.begin().await?;

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
            crate::models::ExceptionType::Skip => {
                if exception.exception_task_id.is_some() {
                    return Err(CoreError::InvalidInput(
                        "Skip exceptions should not have an exception_task_id".to_string()
                    ));
                }
            }
            crate::models::ExceptionType::Override | crate::models::ExceptionType::Move => {
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
        .fetch_all(&self.pool)
        .await?;
        Ok(exceptions)
    }

    async fn remove_series_exception(&self, series_id: Uuid, occurrence_dt: DateTime<Utc>) -> Result<(), CoreError> {
        let result = sqlx::query(
            "DELETE FROM series_exceptions WHERE series_id = $1 AND occurrence_dt = $2"
        )
        .bind(series_id)
        .bind(occurrence_dt)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(
                format!("Exception not found for series {} at occurrence {}", series_id, occurrence_dt)
            ));
        }

        Ok(())
    }

    // ============================================================================
    // Materialization Support Methods (Phase 3)
    // ============================================================================

    async fn refresh_series_materialization(&self, window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> Result<(), CoreError> {
        let active_series = self.find_active_series().await?;
        
        for series in active_series {
            self.refresh_single_series_materialization(series.id, window_start, window_end).await?;
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
        .fetch_all(&self.pool)
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
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(format!("Series with id {} not found", series_id)));
        }

        Ok(())
    }

    // ========== Phase 5: Advanced Exception Management Methods ==========

    async fn add_bulk_series_exceptions(&self, exceptions: Vec<NewSeriesException>) -> Result<Vec<SeriesException>, CoreError> {
        let mut tx = self.pool.begin().await?;
        let mut created_exceptions = Vec::new();

        for exception in exceptions {
            // Validate each exception
            self.validate_exception_data(&exception).await?;
            
            let created = self.add_series_exception_in_transaction(&mut tx, exception).await?;
            created_exceptions.push(created);
        }

        tx.commit().await?;
        Ok(created_exceptions)
    }

    async fn remove_bulk_series_exceptions(&self, series_id: Uuid, occurrence_dts: Vec<DateTime<Utc>>) -> Result<usize, CoreError> {
        let mut tx = self.pool.begin().await?;
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
        .fetch_all(&self.pool)
        .await?;

        Ok(existing_exceptions)
    }

    async fn override_occurrence_with_task(&self, series_id: Uuid, occurrence_dt: DateTime<Utc>, override_task_data: NewTaskData) -> Result<Task, CoreError> {
        let mut tx = self.pool.begin().await?;

        // Create the override task
        let override_task = self.add_task_in_transaction(&mut tx, override_task_data).await?;

        // Create or update the exception
        let exception = NewSeriesException {
            series_id,
            occurrence_dt,
            exception_type: ExceptionType::Override,
            exception_task_id: Some(override_task.id),
            notes: Some(format!("Override task created: {}", override_task.name)),
        };

        self.add_series_exception_in_transaction(&mut tx, exception).await?;

        tx.commit().await?;
        Ok(override_task)
    }

    async fn move_occurrence_with_validation(&self, series_id: Uuid, from_dt: DateTime<Utc>, to_dt: DateTime<Utc>, timezone: &str) -> Result<Task, CoreError> {
        let mut tx = self.pool.begin().await?;

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

        let moved_task = self.add_task_in_transaction(&mut tx, moved_task_data).await?;

        // Create move exception
        let exception = NewSeriesException {
            series_id,
            occurrence_dt: from_dt,
            exception_type: ExceptionType::Move,
            exception_task_id: Some(moved_task.id),
            notes: Some(format!("Moved from {} to {} ({})", from_dt.format("%Y-%m-%d %H:%M"), to_dt.format("%Y-%m-%d %H:%M"), timezone)),
        };

        self.add_series_exception_in_transaction(&mut tx, exception).await?;

        tx.commit().await?;
        Ok(moved_task)
    }

    // ========== Phase 5: Series Management Methods ==========

    async fn duplicate_series(&self, series_id: Uuid, new_name: String, new_timezone: Option<String>) -> Result<TaskSeries, CoreError> {
        let mut tx = self.pool.begin().await?;

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

        let new_template = self.add_task_in_transaction(&mut tx, new_template_data).await?;

        // Create new series
        let new_series_data = NewSeriesData {
            template_task_id: new_template.id,
            rrule: original_series.rrule.clone(),
            dtstart: original_series.dtstart,
            timezone: new_timezone.unwrap_or(original_series.timezone.clone()),
        };

        let new_series = self.create_series_in_transaction(&mut tx, new_series_data).await?;

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
        let mut tx = self.pool.begin().await?;

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
        let mut tx = self.pool.begin().await?;
        let mut updated_series = Vec::new();

        for (series_id, update_data) in updates {
            let updated = self.update_series_in_transaction(&mut tx, series_id, update_data).await?;
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
        .fetch_all(&self.pool)
        .await?;

        Ok(series)
    }

    async fn get_series_statistics(&self, series_id: Uuid) -> Result<SeriesStatistics, CoreError> {
        // Get basic series info
        let series: TaskSeries = sqlx::query_as("SELECT * FROM task_series WHERE id = $1")
            .bind(series_id)
            .fetch_optional(&self.pool)
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
        .fetch_one(&self.pool)
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
        .fetch_one(&self.pool)
        .await?;

        // Get time-based statistics
        let time_stats: (Option<DateTime<Utc>>, Option<DateTime<Utc>>) = sqlx::query_as(
            "SELECT MIN(due_at), MAX(due_at) FROM tasks WHERE series_id = $1 AND due_at IS NOT NULL"
        )
        .bind(series_id)
        .fetch_one(&self.pool)
        .await?;

        // Calculate next occurrence using RecurrenceManager
        let template_task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(series.template_task_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("Template task not found")))?;

        let exceptions: Vec<SeriesException> = sqlx::query_as(
            "SELECT * FROM series_exceptions WHERE series_id = $1"
        )
        .bind(series_id)
        .fetch_all(&self.pool)
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
    async fn refresh_single_series_materialization(&self, series_id: Uuid, window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> Result<(), CoreError> {
        let mut tx = self.pool.begin().await?;

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

        let existing_due_dates: std::collections::HashSet<DateTime<Utc>> = 
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
            if created_count >= self.materialization_manager.config().max_batch_size {
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

    // ============================================================================
    // Helper Methods (Implementation-only, not part of trait)
    // ============================================================================

    async fn create_series_in_transaction<'a>(
        &self,
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

    async fn refresh_single_series_materialization_in_transaction<'a>(
        &self,
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

        let existing_due_dates: std::collections::HashSet<DateTime<Utc>> = 
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

            // Respect batch size limits
            if created_count >= self.materialization_manager.config().max_batch_size {
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

    async fn update_task_fields<'a>(
        &self,
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

                if self.path_exists(&mut *tx, *depends_on_id, id).await? {
                    let task_name = self
                        .find_task_by_id_in_transaction(&mut *tx, id)
                        .await?
                        .map(|t| t.name)
                        .unwrap_or_else(|| id.to_string());
                    let depends_on_task_name = self
                        .find_task_by_id_in_transaction(&mut *tx, *depends_on_id)
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


    /// Extract filters from Query structure for materialization window calculation
    fn extract_filters_from_query(&self, query: &Query) -> Vec<crate::models::Filter> {
        let mut filters = Vec::new();
        self.collect_filters_recursive(query, &mut filters);
        filters
    }

    fn collect_filters_recursive(&self, query: &Query, filters: &mut Vec<crate::models::Filter>) {
        match query {
            Query::Filter(filter) => {
                // Convert query::Filter to models::Filter if possible
                // For now, only convert DueDate filters since that's what MaterializationManager needs
                match filter {
                    crate::query::Filter::Due(due_date) => {
                        // Convert query::DueDate to models::DueDate
                        let models_due_date = match due_date {
                            crate::query::DueDate::Today => crate::models::DueDate::Today,
                            crate::query::DueDate::Tomorrow => crate::models::DueDate::Tomorrow,
                            crate::query::DueDate::Overdue => crate::models::DueDate::Overdue,
                            crate::query::DueDate::Before(dt) => crate::models::DueDate::Before(*dt),
                            crate::query::DueDate::After(dt) => crate::models::DueDate::After(*dt),
                            _ => return, // Skip other types for now
                        };
                        filters.push(crate::models::Filter::DueDate(models_due_date));
                    }
                    // Skip other filter types for now as they don't affect materialization windows
                    _ => {}
                }
            }
            Query::Not(inner_query) => {
                self.collect_filters_recursive(inner_query, filters);
            }
            Query::Binary { left, right, .. } => {
                self.collect_filters_recursive(left, filters);
                self.collect_filters_recursive(right, filters);
            }
        }
    }

    // ========== Phase 5: Helper Methods ==========

    async fn validate_exception_data(&self, exception: &NewSeriesException) -> Result<(), CoreError> {
        // Validate series exists
        let series_exists = sqlx::query("SELECT 1 FROM task_series WHERE id = $1")
            .bind(exception.series_id)
            .fetch_optional(&self.pool)
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

    async fn add_series_exception_in_transaction(&self, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, exception: NewSeriesException) -> Result<SeriesException, CoreError> {
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

    async fn update_series_in_transaction(&self, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, series_id: Uuid, data: UpdateSeriesData) -> Result<TaskSeries, CoreError> {
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
                .fetch_optional(&self.pool)
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

