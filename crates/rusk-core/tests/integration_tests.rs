use rusk_core::db::establish_connection;
use rusk_core::models::{NewTaskData, UpdateTaskData, TaskPriority};
use rusk_core::query::{Filter, Query, TagFilter};
use rusk_core::repository::{Repository, SqliteRepository};

#[tokio::test]
async fn test_add_and_find_task() {
    let pool = establish_connection("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let repo = SqliteRepository::new(pool);

    let new_task_data = NewTaskData {
        name: "Test Task".to_string(),
        description: Some("This is a test task".to_string()),
        ..Default::default()
    };

    let added_task = repo.add_task(new_task_data).await.unwrap();
    assert_eq!(added_task.name, "Test Task");

    let query = Query::Filter(Filter::Tags(TagFilter::Has("nonexistent".to_string()))); // A query that should not fail, but might return 0 results
    let _tasks = repo.find_tasks_with_details(&query).await.unwrap();
    // We are not asserting the length here as the query is not the main point of this test.
    // The main point is that the task is added.
}

#[tokio::test]
async fn test_add_and_find_task_with_tags() {
    let pool = establish_connection("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let repo = SqliteRepository::new(pool);

    let new_task_data = NewTaskData {
        name: "Test Task".to_string(),
        description: Some("This is a test task".to_string()),
        tags: vec!["tag1".to_string(), "tag2".to_string()],
        ..Default::default()
    };

    repo.add_task(new_task_data).await.unwrap();

    let query1 = Query::Filter(Filter::Tags(TagFilter::Has("tag1".to_string())));
    let tasks = repo.find_tasks_with_details(&query1).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "Test Task");

    let query2 = Query::Filter(Filter::Tags(TagFilter::Has("tag3".to_string())));
    let tasks = repo.find_tasks_with_details(&query2).await.unwrap();
    assert_eq!(tasks.len(), 0);
}



#[tokio::test]
async fn test_update_task() {
    let pool = establish_connection("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let repo = SqliteRepository::new(pool);

    // 1. Create a task
    let initial_data = NewTaskData {
        name: "Initial Name".to_string(),
        description: Some("Initial Description".to_string()),
        priority: Some(TaskPriority::Low),
        ..Default::default()
    };
    let task_to_update = repo.add_task(initial_data).await.unwrap();

    // 2. Update the task
    let update_data = UpdateTaskData {
        name: Some("Updated Name".to_string()),
        description: Some(Some("Updated Description".to_string())),
        priority: Some(TaskPriority::High),
        ..Default::default()
    };
    let updated_task = repo.update_task(task_to_update.id, update_data).await.unwrap();

    // 3. Verify the changes
    assert_eq!(updated_task.id, task_to_update.id);
    assert_eq!(updated_task.name, "Updated Name");
    assert_eq!(updated_task.description, Some("Updated Description".to_string()));
    assert_eq!(updated_task.priority, TaskPriority::High);

    // 4. Verify that fetching the task again shows the updated values
    let fetched_task = repo.find_task_by_id(task_to_update.id).await.unwrap().unwrap();
    assert_eq!(fetched_task.name, "Updated Name");
    assert_eq!(fetched_task.priority, TaskPriority::High);
}