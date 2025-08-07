use crate::db::DbPool;
use crate::error::CoreError;
use crate::models::{
    CompletionResult, DueDate, Filter, NewTaskData, Project, Task, TaskPriority, TaskStatus,
    UpdateTaskData,
};
use crate::recurrence::RecurrenceManager;
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
    pub rrule: Option<String>,
    pub recurrence_template_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub depth: i32,
    pub tags: Option<String>,
}

#[async_trait]
pub trait Repository {
    async fn add_task(&self, data: NewTaskData) -> Result<Task, CoreError>;
    async fn find_task_by_id(&self, id: Uuid) -> Result<Option<Task>, CoreError>;
    async fn find_tasks_by_short_id_prefix(&self, short_id: &str) -> Result<Vec<Task>, CoreError>;
    async fn find_tasks(&self, filters: &[Filter]) -> Result<Vec<Task>, CoreError>;
    async fn find_tasks_with_details(
        &self,
        filters: &[Filter],
    ) -> Result<Vec<TaskQueryResult>, CoreError>;
    async fn delete_task(&self, id: Uuid) -> Result<(), CoreError>;
    async fn complete_task(&self, id: Uuid) -> Result<CompletionResult, CoreError>;
    async fn cancel_task(&self, id: Uuid) -> Result<Task, CoreError>;
    async fn update_task(&self, id: Uuid, data: UpdateTaskData) -> Result<Task, CoreError>;
    async fn add_project(
        &self,
        name: String,
        description: Option<String>,
    ) -> Result<Project, CoreError>;
    async fn find_project_by_id(&self, id: Uuid) -> Result<Option<Project>, CoreError>;
    async fn find_project_by_name(&self, name: &str) -> Result<Option<Project>, CoreError>;
    async fn find_projects(&self) -> Result<Vec<Project>, CoreError>;
    async fn delete_project(&self, name: String) -> Result<(), CoreError>;
}

pub struct SqliteRepository {
    pool: DbPool,
}

impl SqliteRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
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
            id: Uuid::new_v4(),
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
            rrule: data.rrule,
            recurrence_template_id: data.recurrence_template_id,
        };

        sqlx::query(
            r#"INSERT INTO tasks (id, name, description, status, priority, due_at, created_at, updated_at, project_id, parent_id, rrule, recurrence_template_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
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
        .bind(&task.rrule)
        .bind(task.recurrence_template_id)
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
        let task = self.add_task_in_transaction(&mut tx, data).await?;
        tx.commit().await?;
        Ok(task)
    }

    async fn find_task_by_id(&self, id: Uuid) -> Result<Option<Task>, CoreError> {
        let task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(task)
    }

    async fn find_tasks_by_short_id_prefix(&self, short_id: &str) -> Result<Vec<Task>, CoreError> {
        let tasks: Vec<Task> = sqlx::query_as("SELECT * FROM tasks WHERE id LIKE ?")
            .bind(format!("{}%", short_id))
            .fetch_all(&self.pool)
            .await?;
        Ok(tasks)
    }

    async fn find_tasks(&self, filters: &[Filter]) -> Result<Vec<Task>, CoreError> {
        let mut query_builder: QueryBuilder<sqlx::Sqlite> =
            QueryBuilder::new("SELECT t.* FROM tasks t");

        if !filters.is_empty() {
            query_builder.push(" WHERE ");
            let mut first = true;
            for filter in filters {
                if !first {
                    query_builder.push(" AND ");
                }
                match filter {
                    Filter::Status(status) => {
                        query_builder.push("t.status = ");
                        query_builder.push_bind(status.clone());
                    }
                    Filter::Tag(tag) => {
                        query_builder
                            .push("t.id IN (SELECT task_id FROM task_tags WHERE tag_name = ");
                        query_builder.push_bind(tag);
                        query_builder.push(")");
                    }
                    Filter::TagNot(tag) => {
                        query_builder
                            .push("t.id NOT IN (SELECT task_id FROM task_tags WHERE tag_name = ");
                        query_builder.push_bind(tag);
                        query_builder.push(")");
                    }
                    Filter::Project(project) => {
                        query_builder.push("t.project_id = (SELECT id FROM projects WHERE name = ");
                        query_builder.push_bind(project);
                        query_builder.push(")");
                    }
                    Filter::Priority(priority) => {
                        query_builder.push("t.priority = ");
                        query_builder.push_bind(priority.clone());
                    }
                    Filter::DueDate(due_date) => match due_date {
                        DueDate::Today => {
                            query_builder.push("date(t.due_at) = date('now')");
                        }
                        DueDate::Tomorrow => {
                            query_builder.push("date(t.due_at) = date('now', '+1 day')");
                        }
                        DueDate::Overdue => {
                            query_builder
                                .push("date(t.due_at) < date('now') AND t.status = 'pending'");
                        }
                        DueDate::Before(date) => {
                            query_builder.push("t.due_at < ");
                            query_builder.push_bind(date);
                        }
                        DueDate::After(date) => {
                            query_builder.push("t.due_at > ");
                            query_builder.push_bind(date);
                        }
                    },
                }
                first = false;
            }
        }

        let tasks = query_builder.build_query_as().fetch_all(&self.pool).await?;
        Ok(tasks)
    }

    async fn find_tasks_with_details(
        &self,
        filters: &[Filter],
    ) -> Result<Vec<TaskQueryResult>, CoreError> {
        let mut query_builder: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new(
            r#"WITH RECURSIVE task_hierarchy (id, name, description, status, priority, due_at, completed_at, created_at, updated_at, project_id, parent_id, rrule, recurrence_template_id, depth, path) AS (
                SELECT
                    t.id, t.name, t.description, t.status, t.priority, t.due_at, t.completed_at, t.created_at, t.updated_at, t.project_id, t.parent_id, t.rrule, t.recurrence_template_id,
                    0 as depth,
                    CAST(t.created_at AS TEXT) as path
                FROM tasks t
                WHERE t.parent_id IS NULL
                UNION ALL
                SELECT
                    t.id, t.name, t.description, t.status, t.priority, t.due_at, t.completed_at, t.created_at, t.updated_at, t.project_id, t.parent_id, t.rrule, t.recurrence_template_id,
                    th.depth + 1,
                    th.path || ' -> ' || CAST(t.created_at AS TEXT)
                FROM tasks t
                JOIN task_hierarchy th ON t.parent_id = th.id
            )
            SELECT
                th.id, th.name, th.description, th.status, th.priority, th.due_at, th.completed_at, th.created_at, th.updated_at, th.project_id, th.parent_id, th.rrule, th.recurrence_template_id, th.depth, th.path,
                p.name as project_name,
                GROUP_CONCAT(tt.tag_name) as tags
            FROM task_hierarchy th
            LEFT JOIN projects p ON th.project_id = p.id
            LEFT JOIN task_tags tt ON th.id = tt.task_id
            "#,
        );

        if !filters.is_empty() {
            query_builder.push(" WHERE ");
            let mut first = true;
            for filter in filters {
                if !first {
                    query_builder.push(" AND ");
                }
                match filter {
                    Filter::Status(status) => {
                        query_builder.push("th.status = ");
                        query_builder.push_bind(status.clone());
                    }
                    Filter::Tag(tag) => {
                        query_builder
                            .push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name = ");
                        query_builder.push_bind(tag);
                        query_builder.push(")");
                    }
                    Filter::TagNot(tag) => {
                        query_builder
                            .push("th.id NOT IN (SELECT task_id FROM task_tags WHERE tag_name = ");
                        query_builder.push_bind(tag);
                        query_builder.push(")");
                    }
                    Filter::Project(project) => {
                        query_builder.push("p.name = ");
                        query_builder.push_bind(project);
                    }
                    Filter::Priority(priority) => {
                        query_builder.push("th.priority = ");
                        query_builder.push_bind(priority.clone());
                    }
                    Filter::DueDate(due_date) => match due_date {
                        DueDate::Today => {
                            query_builder.push("date(th.due_at) = date('now')");
                        }
                        DueDate::Tomorrow => {
                            query_builder.push("date(th.due_at) = date('now', '+1 day')");
                        }
                        DueDate::Overdue => {
                            query_builder
                                .push("date(th.due_at) < date('now') AND th.status = 'pending'");
                        }
                        DueDate::Before(date) => {
                            query_builder.push("th.due_at < ");
                            query_builder.push_bind(date);
                        }
                        DueDate::After(date) => {
                            query_builder.push("th.due_at > ");
                            query_builder.push_bind(date);
                        }
                    },
                }
                first = false;
            }
        }

        query_builder.push(" GROUP BY th.id, th.name, th.description, th.status, th.priority, th.due_at, th.completed_at, th.created_at, th.updated_at, th.project_id, th.parent_id, th.rrule, th.recurrence_template_id, th.depth, th.path, p.name");
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

        // Check for blocking dependencies (unchanged)
        let dependencies: Vec<Task> = sqlx::query_as(
            r#"SELECT t.* FROM tasks t
            INNER JOIN task_dependencies td ON t.id = td.depends_on_id
            WHERE td.task_id = $1 AND t.status != 'completed'"#,
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;

        if !dependencies.is_empty() {
            let dependency_names: Vec<String> =
                dependencies.iter().map(|t| t.name.clone()).collect();
            return Err(CoreError::TaskBlocked(dependency_names.join(", ")));
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

        // New "Template-Instance" Recurrence Logic
        let template_task = if let Some(template_id) = completed_task.recurrence_template_id {
            // This is an instance, fetch its template
            self.find_task_by_id_in_transaction(&mut tx, template_id)
                .await?
                .ok_or(CoreError::NotFound(
                    "Recurrence template not found".to_string(),
                ))?
        } else {
            // This is the first task in a series, so it is its own template
            completed_task.clone()
        };

        if template_task.rrule.is_some() {
            let recurrence_manager = RecurrenceManager::new(template_task.clone());
            let last_due = completed_task.due_at.unwrap_or_else(Utc::now);

            // The crucial fix: ensure we search strictly *after* the last due date.
            if let Some(next_due) = recurrence_manager.get_next_occurrence(last_due) {
                let tags: Vec<String> =
                    sqlx::query_scalar("SELECT tag_name FROM task_tags WHERE task_id = $1")
                        .bind(template_task.id)
                        .fetch_all(&mut *tx)
                        .await?;

                let new_task_data = NewTaskData {
                    name: template_task.name.clone(),
                    description: template_task.description.clone(),
                    due_at: Some(next_due),
                    priority: Some(template_task.priority.clone()),
                    project_id: template_task.project_id,
                    tags,
                    parent_id: template_task.parent_id,
                    rrule: template_task.rrule.clone(),
                    recurrence_template_id: Some(template_task.id),
                    ..Default::default()
                };

                let next_task = self.add_task_in_transaction(&mut tx, new_task_data).await?;

                tx.commit().await?;
                return Ok(CompletionResult::Recurring {
                    completed: completed_task,
                    next: next_task,
                });
            }
        }

        tx.commit().await?;
        Ok(CompletionResult::Single(completed_task))
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

    async fn update_task(&self, id: Uuid, data: UpdateTaskData) -> Result<Task, CoreError> {
        let mut tx = self.pool.begin().await?;

        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("UPDATE tasks SET ");
        let mut updated = false;

        if let Some(name) = data.name {
            qb.push("name = ");
            qb.push_bind(name);
            updated = true;
        }

        if let Some(description) = data.description {
            if updated {
                qb.push(", ");
            }
            qb.push("description = ");
            qb.push_bind(description);
            updated = true;
        }

        if let Some(due_at) = data.due_at {
            if updated {
                qb.push(", ");
            }
            qb.push("due_at = ");
            qb.push_bind(due_at);
            updated = true;
        }

        if let Some(priority) = data.priority {
            if updated {
                qb.push(", ");
            }
            qb.push("priority = ");
            qb.push_bind(priority);
            updated = true;
        }

        if let Some(status) = data.status {
            if updated {
                qb.push(", ");
            }
            qb.push("status = ");
            qb.push_bind(status);
            updated = true;
        }

        if let Some(parent_id) = data.parent_id {
            if updated {
                qb.push(", ");
            }
            qb.push("parent_id = ");
            qb.push_bind(parent_id);
            updated = true;
        }

        if let Some(rrule) = data.rrule {
            if updated {
                qb.push(", ");
            }
            qb.push("rrule = ");
            qb.push_bind(rrule);
            updated = true;
        }

        if let Some(recurrence_template_id) = data.recurrence_template_id {
            if updated {
                qb.push(", ");
            }
            qb.push("recurrence_template_id = ");
            qb.push_bind(recurrence_template_id);
            updated = true;
        }

        if let Some(project_name_option) = data.project_name {
            let project_id = match project_name_option {
                Some(project_name) => {
                    let project: Option<Project> =
                        sqlx::query_as("SELECT * FROM projects WHERE name = $1")
                            .bind(project_name.clone())
                            .fetch_optional(&mut *tx)
                            .await?;
                    Some(
                        project
                            .map(|p| p.id)
                            .ok_or_else(|| CoreError::NotFound(project_name))?,
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

        if let Some(depends_on_option) = data.depends_on {
            // First, remove any existing dependency for this task.
            sqlx::query("DELETE FROM task_dependencies WHERE task_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;

            if let Some(depends_on_id) = depends_on_option {
                // A new dependency is being set.
                if id == depends_on_id {
                    return Err(CoreError::InvalidInput(
                        "A task cannot depend on itself.".to_string(),
                    ));
                }

                if self.path_exists(&mut tx, depends_on_id, id).await? {
                    let task_name = self
                        .find_task_by_id_in_transaction(&mut tx, id)
                        .await?
                        .map(|t| t.name)
                        .unwrap_or_else(|| id.to_string());
                    let depends_on_task_name = self
                        .find_task_by_id_in_transaction(&mut tx, depends_on_id)
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
                    .execute(&mut *tx)
                    .await?;
            }
            // If depends_on_option is None, the dependency is just cleared, which we already did.
            updated = true;
        }

        if let Some(tags_to_add) = data.add_tags {
            if !tags_to_add.is_empty() {
                let mut query_builder: QueryBuilder<sqlx::Sqlite> =
                    QueryBuilder::new("INSERT OR IGNORE INTO task_tags (task_id, tag_name) ");
                query_builder.push_values(tags_to_add.iter(), |mut b, tag| {
                    b.push_bind(id).push_bind(tag);
                });
                query_builder.build().execute(&mut *tx).await?;
            }
        }

        if let Some(tags_to_remove) = data.remove_tags {
            if !tags_to_remove.is_empty() {
                let mut query_builder: QueryBuilder<sqlx::Sqlite> =
                    QueryBuilder::new("DELETE FROM task_tags WHERE task_id = $1 AND tag_name IN (");
                query_builder.push_bind(id);
                let mut separated = query_builder.separated(", ");
                for tag in tags_to_remove.iter() {
                    separated.push_bind(tag);
                }
                separated.push_unseparated(")");
                query_builder.build().execute(&mut *tx).await?;
            }
        }

        if updated {
            qb.push(", updated_at = ");
            qb.push_bind(Utc::now());
            qb.push(" WHERE id = ");
            qb.push_bind(id);
            qb.build().execute(&mut *tx).await?;
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
        let project_id = Uuid::new_v4();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::establish_connection;
    use crate::models::{CompletionResult, TaskPriority, TaskStatus};
    use std::collections::HashSet;

    async fn setup() -> SqliteRepository {
        let pool = establish_connection("sqlite::memory:").await.unwrap();
        SqliteRepository::new(pool)
    }

    #[tokio::test]
    async fn test_add_and_get_task_with_details() {
        let repo = setup().await;
        let new_task_data = NewTaskData {
            name: "Test Task".to_string(),
            description: Some("Test Description".to_string()),
            due_at: None,
            priority: Some(TaskPriority::High),
            project_name: None,
            tags: vec!["test".to_string(), "rust".to_string()],
            parent_id: None,
            rrule: None,
            depends_on: None,
            project_id: None,
            recurrence_template_id: None,
        };

        let added_task = repo.add_task(new_task_data.clone()).await.unwrap();
        assert_eq!(added_task.name, new_task_data.name);

        let fetched_tasks = repo.find_tasks_with_details(&[]).await.unwrap();
        let fetched_task = fetched_tasks
            .iter()
            .find(|t| t.id == added_task.id)
            .unwrap();

        assert_eq!(fetched_task.id, added_task.id);
        assert_eq!(fetched_task.name, "Test Task");

        let mut tags: Vec<String> = fetched_task
            .tags
            .as_ref()
            .unwrap()
            .split(',')
            .map(|s| s.to_string())
            .collect();
        tags.sort();
        assert_eq!(tags, vec!["rust".to_string(), "test".to_string()]);
    }

    #[tokio::test]
    async fn test_find_tasks_with_details_tags() {
        use std::collections::HashMap;
        let repo = setup().await;
        let task1_data = NewTaskData {
            name: "Task 1".to_string(),
            tags: vec!["a".to_string(), "b".to_string()],
            ..Default::default()
        };
        let task1 = repo.add_task(task1_data).await.unwrap();

        let task2_data = NewTaskData {
            name: "Task 2".to_string(),
            tags: vec!["b".to_string(), "c".to_string()],
            ..Default::default()
        };
        let task2 = repo.add_task(task2_data).await.unwrap();

        let task3_data = NewTaskData {
            name: "Task 3".to_string(),
            ..Default::default()
        };
        let task3 = repo.add_task(task3_data).await.unwrap();

        let results = repo.find_tasks_with_details(&[]).await.unwrap();
        let results_map: HashMap<Uuid, TaskQueryResult> =
            results.into_iter().map(|t| (t.id, t)).collect();

        // Task 1
        let task1_result = results_map.get(&task1.id).unwrap();
        let task1_tags: HashSet<String> = task1_result
            .tags
            .as_ref()
            .unwrap()
            .split(',')
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            task1_tags,
            ["a".to_string(), "b".to_string()].iter().cloned().collect()
        );

        // Task 2
        let task2_result = results_map.get(&task2.id).unwrap();
        let task2_tags: HashSet<String> = task2_result
            .tags
            .as_ref()
            .unwrap()
            .split(',')
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            task2_tags,
            ["b".to_string(), "c".to_string()].iter().cloned().collect()
        );

        // Task 3
        let task3_result = results_map.get(&task3.id).unwrap();
        assert!(task3_result.tags.is_none());
    }

    #[tokio::test]
    async fn test_complete_task_blocked() {
        let repo = setup().await;
        let task1_data = NewTaskData {
            name: "Task 1".to_string(),
            ..Default::default()
        };
        let task1 = repo.add_task(task1_data).await.unwrap();

        let task2_data = NewTaskData {
            name: "Task 2".to_string(),
            depends_on: Some(task1.id),
            ..Default::default()
        };
        let task2 = repo.add_task(task2_data).await.unwrap();

        let result = repo.complete_task(task2.id).await;
        assert!(matches!(result, Err(CoreError::TaskBlocked(_))));
    }

    #[tokio::test]
    async fn test_complete_task_unblocked() {
        let repo = setup().await;
        let task1_data = NewTaskData {
            name: "Task 1".to_string(),
            ..Default::default()
        };
        let task1 = repo.add_task(task1_data).await.unwrap();

        let task2_data = NewTaskData {
            name: "Task 2".to_string(),
            depends_on: Some(task1.id),
            ..Default::default()
        };
        let task2 = repo.add_task(task2_data).await.unwrap();

        repo.complete_task(task1.id).await.unwrap();
        let result = repo.complete_task(task2.id).await;
        assert!(result.is_ok());
        match result.unwrap() {
            CompletionResult::Single(task) => {
                assert_eq!(task.status, TaskStatus::Completed);
            }
            _ => panic!("Expected a single completion"),
        }
    }
}