use crate::error::CoreError;
use crate::models::Project;
use crate::repository::SqliteRepository;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
impl super::ProjectRepository for SqliteRepository {
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
        .fetch_one(self.pool())
        .await?;

        Ok(project)
    }

    async fn find_project_by_id(&self, id: Uuid) -> Result<Option<Project>, CoreError> {
        let project = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
            .bind(id)
            .fetch_optional(self.pool())
            .await?;
        Ok(project)
    }

    async fn find_project_by_name(&self, name: &str) -> Result<Option<Project>, CoreError> {
        let project = sqlx::query_as("SELECT * FROM projects WHERE name = $1")
            .bind(name)
            .fetch_optional(self.pool())
            .await?;
        Ok(project)
    }

    async fn find_projects(&self) -> Result<Vec<Project>, CoreError> {
        let projects = sqlx::query_as("SELECT id, name, description, created_at FROM projects")
            .fetch_all(self.pool())
            .await?;
        Ok(projects)
    }

    async fn delete_project(&self, name: String) -> Result<(), CoreError> {
        // First, check if the project exists and get its ID
        let project: Option<Project> = sqlx::query_as("SELECT * FROM projects WHERE name = $1")
            .bind(&name)
            .fetch_optional(self.pool())
            .await?;
            
        let project = project.ok_or_else(|| CoreError::NotFound("Project not found".to_string()))?;
        
        // Check if there are any tasks associated with this project
        let task_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE project_id = $1")
            .bind(project.id)
            .fetch_one(self.pool())
            .await?;
            
        if task_count.0 > 0 {
            return Err(CoreError::InvalidInput(format!(
                "Cannot delete project '{}' because it has {} associated task(s). Delete or move the tasks first.",
                name, task_count.0
            )));
        }

        // Now safe to delete the project
        let result = sqlx::query("DELETE FROM projects WHERE name = $1")
            .bind(name)
            .execute(self.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound("Project not found".to_string()));
        }
        Ok(())
    }
}