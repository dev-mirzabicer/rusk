use rusk_core::db::establish_connection;
use rusk_core::models::*;
use rusk_core::recurrence::*;
use rusk_core::repository::{Repository, SqliteRepository};
use chrono::{DateTime, Utc, Duration};
use tempfile::TempDir;
use uuid::Uuid;

/// Helper function to create a test database
async fn setup_test_db() -> (SqliteRepository, TempDir) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("test.db");
    
    let pool = establish_connection(&db_path.to_string_lossy())
        .await
        .expect("Failed to establish test database connection");
    
    let materialization_manager = MaterializationManager::with_defaults();
    let repository = SqliteRepository::new(pool, materialization_manager);
    
    (repository, temp_dir)
}

/// Helper function to create a test project
async fn create_test_project(repo: &SqliteRepository, name: &str) -> Project {
    repo.add_project(
        name.to_string(),
        Some(format!("Test project: {}", name))
    )
    .await
    .expect("Failed to create test project")
}

/// Helper function to create a test task
async fn create_test_task(repo: &SqliteRepository, name: &str, project_id: Option<Uuid>) -> Task {
    let task_data = NewTaskData {
        name: name.to_string(),
        description: Some(format!("Test task: {}", name)),
        priority: Some(TaskPriority::Medium),
        due_at: Some(Utc::now() + Duration::hours(24)),
        project_id,
        ..Default::default()
    };
    
    repo.add_task(task_data)
        .await
        .expect("Failed to create test task")
}

#[tokio::test]
async fn test_basic_task_crud_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Create a project
    let project = create_test_project(&repo, "Test Project").await;
    
    // Create a task in the project
    let task = create_test_task(&repo, "Test Task", Some(project.id)).await;
    
    // Verify task was created correctly
    assert_eq!(task.name, "Test Task");
    assert_eq!(task.project_id, Some(project.id));
    assert_eq!(task.status, TaskStatus::Pending);
    assert_eq!(task.priority, TaskPriority::Medium);
    
    // Update the task
    let update_data = UpdateTaskData {
        name: Some("Updated Task".to_string()),
        priority: Some(TaskPriority::High),
        ..Default::default()
    };
    
    let updated_task = repo
        .update_task(task.id, update_data, Some(EditScope::ThisOccurrence))
        .await
        .expect("Failed to update task");
    
    assert_eq!(updated_task.name, "Updated Task");
    assert_eq!(updated_task.priority, TaskPriority::High);
    
    // Complete the task
    let completion_result = repo
        .complete_task(task.id)
        .await
        .expect("Failed to complete task");
    
    match completion_result {
        CompletionResult::Single(completed_task) => {
            assert_eq!(completed_task.status, TaskStatus::Completed);
            assert!(completed_task.completed_at.is_some());
        }
        _ => panic!("Expected single task completion"),
    }
    
    // Delete the task
    repo.delete_task(task.id)
        .await
        .expect("Failed to delete task");
    
    // Verify task is deleted
    let result = repo.find_task_by_id(task.id).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_task_filtering_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Create a project
    let project = create_test_project(&repo, "Filter Test Project").await;
    
    // Create tasks with different attributes
    let now = Utc::now();
    
    // High priority task due today
    let task1_data = NewTaskData {
        name: "High Priority Today".to_string(),
        priority: Some(TaskPriority::High),
        due_at: Some(now + Duration::hours(12)),
        project_id: Some(project.id),
        tags: vec!["urgent".to_string(), "work".to_string()],
        ..Default::default()
    };
    let task1 = repo.add_task(task1_data).await.unwrap();
    
    // Medium priority task due tomorrow
    let task2_data = NewTaskData {
        name: "Medium Priority Tomorrow".to_string(),
        priority: Some(TaskPriority::Medium),
        due_at: Some(now + Duration::days(1)),
        project_id: Some(project.id),
        tags: vec!["personal".to_string()],
        ..Default::default()
    };
    let task2 = repo.add_task(task2_data).await.unwrap();
    
    // Overdue task
    let task3_data = NewTaskData {
        name: "Overdue Task".to_string(),
        priority: Some(TaskPriority::Low),
        due_at: Some(now - Duration::days(1)),
        tags: vec!["work".to_string()],
        ..Default::default()
    };
    let task3 = repo.add_task(task3_data).await.unwrap();
    
    // Test filtering by priority
    let high_priority_filters = vec![Filter::Priority(TaskPriority::High)];
    let high_priority_tasks = repo
        .find_tasks_with_details(&high_priority_filters)
        .await
        .unwrap();
    
    assert_eq!(high_priority_tasks.len(), 1);
    assert_eq!(high_priority_tasks[0].id, task1.id);
    
    // Test filtering by project
    let project_filters = vec![Filter::Project(project.id)];
    let project_tasks = repo
        .find_tasks_with_details(&project_filters)
        .await
        .unwrap();
    
    assert_eq!(project_tasks.len(), 2); // task1 and task2
    
    // Test filtering by tag
    let work_tag_filters = vec![Filter::Tag("work".to_string())];
    let work_tasks = repo
        .find_tasks_with_details(&work_tag_filters)
        .await
        .unwrap();
    
    assert_eq!(work_tasks.len(), 2); // task1 and task3
    
    // Test filtering by status
    let pending_filters = vec![Filter::Status(TaskStatus::Pending)];
    let pending_tasks = repo
        .find_tasks_with_details(&pending_filters)
        .await
        .unwrap();
    
    assert_eq!(pending_tasks.len(), 3); // All tasks are pending
    
    // Test filtering by due date
    let overdue_filters = vec![Filter::DueDate(DueDate::Overdue)];
    let overdue_tasks = repo
        .find_tasks_with_details(&overdue_filters)
        .await
        .unwrap();
    
    assert_eq!(overdue_tasks.len(), 1);
    assert_eq!(overdue_tasks[0].id, task3.id);
    
    // Test combined filters
    let combined_filters = vec![
        Filter::Project(project.id),
        Filter::Priority(TaskPriority::High),
    ];
    let combined_tasks = repo
        .find_tasks_with_details(&combined_filters)
        .await
        .unwrap();
    
    assert_eq!(combined_tasks.len(), 1);
    assert_eq!(combined_tasks[0].id, task1.id);
}

#[tokio::test]
async fn test_task_dependencies_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Create dependent tasks
    let task1 = create_test_task(&repo, "Task 1", None).await;
    let task2_data = NewTaskData {
        name: "Task 2 (depends on Task 1)".to_string(),
        depends_on: Some(task1.id),
        ..Default::default()
    };
    let task2 = repo.add_task(task2_data).await.unwrap();
    
    // Try to complete dependent task first (should fail)
    let result = repo.complete_task(task2.id).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), rusk_core::error::CoreError::TaskBlocked(_)));
    
    // Complete prerequisite task
    repo.complete_task(task1.id).await.unwrap();
    
    // Now complete dependent task (should succeed)
    let completion_result = repo.complete_task(task2.id).await.unwrap();
    match completion_result {
        CompletionResult::Single(completed_task) => {
            assert_eq!(completed_task.status, TaskStatus::Completed);
        }
        _ => panic!("Expected single task completion"),
    }
}

#[tokio::test]
async fn test_subtask_hierarchy_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Create parent task
    let parent_task = create_test_task(&repo, "Parent Task", None).await;
    
    // Create subtasks
    let subtask1_data = NewTaskData {
        name: "Subtask 1".to_string(),
        parent_id: Some(parent_task.id),
        ..Default::default()
    };
    let subtask1 = repo.add_task(subtask1_data).await.unwrap();
    
    let subtask2_data = NewTaskData {
        name: "Subtask 2".to_string(),
        parent_id: Some(parent_task.id),
        ..Default::default()
    };
    let subtask2 = repo.add_task(subtask2_data).await.unwrap();
    
    // Create nested subtask
    let nested_subtask_data = NewTaskData {
        name: "Nested Subtask".to_string(),
        parent_id: Some(subtask1.id),
        ..Default::default()
    };
    let nested_subtask = repo.add_task(nested_subtask_data).await.unwrap();
    
    // Fetch all tasks and verify hierarchy
    let all_tasks = repo.find_tasks_with_details(&[]).await.unwrap();
    
    // Find the tasks in the hierarchy
    let found_parent = all_tasks.iter().find(|t| t.id == parent_task.id).unwrap();
    let found_subtask1 = all_tasks.iter().find(|t| t.id == subtask1.id).unwrap();
    let found_subtask2 = all_tasks.iter().find(|t| t.id == subtask2.id).unwrap();
    let found_nested = all_tasks.iter().find(|t| t.id == nested_subtask.id).unwrap();
    
    // Verify hierarchy relationships
    assert!(found_parent.parent_id.is_none());
    assert_eq!(found_subtask1.parent_id, Some(parent_task.id));
    assert_eq!(found_subtask2.parent_id, Some(parent_task.id));
    assert_eq!(found_nested.parent_id, Some(subtask1.id));
}

#[tokio::test]
async fn test_recurring_task_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Create a recurring task (daily for 3 occurrences)
    let series_data = NewSeriesData {
        template_task_id: Uuid::now_v7(), // Will be set by the repository
        rrule: "FREQ=DAILY;COUNT=3".to_string(),
        dtstart: Utc::now(),
        timezone: "UTC".to_string(),
    };
    
    let task_data = NewTaskData {
        name: "Daily Recurring Task".to_string(),
        description: Some("A task that occurs daily".to_string()),
        priority: Some(TaskPriority::Medium),
        due_at: Some(Utc::now() + Duration::hours(1)),
        rrule: Some("FREQ=DAILY;COUNT=3".to_string()),
        ..Default::default()
    };
    
    // Create the recurring task
    let template_task = repo.add_task(task_data).await.unwrap();
    
    // Create the series manually (this would normally be done in add_task)
    let mut series_data = series_data;
    series_data.template_task_id = template_task.id;
    let series = repo.create_series(series_data).await.unwrap();
    
    // Generate occurrences for the next 7 days
    let recurrence_manager = RecurrenceManager::new(
        series.clone(),
        template_task.clone(),
        vec![]
    ).unwrap();
    
    let now = Utc::now();
    let one_week_later = now + Duration::days(7);
    let occurrences = recurrence_manager
        .generate_occurrences_between(now, one_week_later)
        .unwrap();
    
    // Should have 3 occurrences (due to COUNT=3)
    assert_eq!(occurrences.len(), 3);
    
    // Verify occurrences are daily
    for i in 1..occurrences.len() {
        let prev_dt = occurrences[i-1].occurrence_dt;
        let curr_dt = occurrences[i].occurrence_dt;
        let diff = curr_dt - prev_dt;
        assert_eq!(diff.num_days(), 1);
    }
    
    // Test skipping an occurrence
    let second_occurrence_dt = occurrences[1].occurrence_dt;
    let skip_exception = NewSeriesException {
        series_id: series.id,
        occurrence_dt: second_occurrence_dt,
        exception_type: ExceptionType::Skip,
        exception_task_id: None,
        notes: Some("Skipping this occurrence".to_string()),
    };
    
    repo.add_series_exception(skip_exception).await.unwrap();
    
    // Generate occurrences again and verify the second one is skipped
    let exceptions = repo.find_series_exceptions(series.id).await.unwrap();
    let recurrence_manager_with_exceptions = RecurrenceManager::new(
        series,
        template_task,
        exceptions
    ).unwrap();
    
    let occurrences_with_exceptions = recurrence_manager_with_exceptions
        .generate_occurrences_between(now, one_week_later)
        .unwrap();
    
    // Should now have 2 occurrences (third one skipped)
    assert_eq!(occurrences_with_exceptions.len(), 2);
    
    // Verify the skipped occurrence is not in the list
    let occurrence_dts: Vec<DateTime<Utc>> = occurrences_with_exceptions
        .iter()
        .map(|o| o.occurrence_dt)
        .collect();
    assert!(!occurrence_dts.contains(&second_occurrence_dt));
}

#[tokio::test]
async fn test_project_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Create a project
    let project = create_test_project(&repo, "Workflow Project").await;
    
    // Update project
    let updated_project = repo
        .update_project(
            project.id,
            Some("Updated Workflow Project".to_string()),
            Some("Updated description".to_string())
        )
        .await
        .unwrap();
    
    assert_eq!(updated_project.name, "Updated Workflow Project");
    assert_eq!(updated_project.description, Some("Updated description".to_string()));
    
    // Create tasks in the project
    let task1 = create_test_task(&repo, "Task 1", Some(project.id)).await;
    let task2 = create_test_task(&repo, "Task 2", Some(project.id)).await;
    
    // List all projects
    let projects = repo.find_all_projects().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, project.id);
    
    // Cannot delete project with tasks
    let delete_result = repo.delete_project(project.id).await;
    assert!(delete_result.is_err());
    
    // Delete tasks first
    repo.delete_task(task1.id).await.unwrap();
    repo.delete_task(task2.id).await.unwrap();
    
    // Now delete project should succeed
    repo.delete_project(project.id).await.unwrap();
    
    // Verify project is deleted
    let result = repo.find_project_by_id(project.id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_error_handling_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Test finding non-existent task
    let non_existent_id = Uuid::now_v7();
    let result = repo.find_task_by_id(non_existent_id).await.unwrap();
    assert!(result.is_none());
    
    // Test updating non-existent task
    let update_data = UpdateTaskData {
        name: Some("Updated".to_string()),
        ..Default::default()
    };
    let result = repo.update_task(non_existent_id, update_data, Some(EditScope::ThisOccurrence)).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), rusk_core::error::CoreError::NotFound));
    
    // Test deleting non-existent task
    let result = repo.delete_task(non_existent_id).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), rusk_core::error::CoreError::NotFound));
    
    // Test completing non-existent task
    let result = repo.complete_task(non_existent_id).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), rusk_core::error::CoreError::NotFound));
    
    // Test circular dependency detection
    let task1 = create_test_task(&repo, "Task 1", None).await;
    let task2_data = NewTaskData {
        name: "Task 2".to_string(),
        depends_on: Some(task1.id),
        ..Default::default()
    };
    let task2 = repo.add_task(task2_data).await.unwrap();
    
    // Try to make task1 depend on task2 (circular dependency)
    let circular_update = UpdateTaskData {
        depends_on: Some(Some(task2.id)),
        ..Default::default()
    };
    let result = repo.update_task(task1.id, circular_update, Some(EditScope::ThisOccurrence)).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), rusk_core::error::CoreError::CircularDependency));
}

#[tokio::test] 
async fn test_timezone_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Create a series with New York timezone
    let now_utc = Utc::now();
    let series_data = NewSeriesData {
        template_task_id: Uuid::now_v7(),
        rrule: "FREQ=DAILY;INTERVAL=1".to_string(),
        dtstart: now_utc,
        timezone: "America/New_York".to_string(),
    };
    
    let task_data = NewTaskData {
        name: "Timezone Test Task".to_string(),
        due_at: Some(now_utc),
        rrule: Some("FREQ=DAILY;INTERVAL=1".to_string()),
        ..Default::default()
    };
    
    let template_task = repo.add_task(task_data).await.unwrap();
    
    let mut series_data = series_data;
    series_data.template_task_id = template_task.id;
    let series = repo.create_series(series_data).await.unwrap();
    
    // Verify timezone is stored correctly
    assert_eq!(series.timezone, "America/New_York");
    
    // Create RecurrenceManager and verify timezone handling
    let recurrence_manager = RecurrenceManager::new(
        series,
        template_task,
        vec![]
    ).unwrap();
    
    // Verify timezone parsing
    assert_eq!(recurrence_manager.timezone().to_string(), "America/New_York");
    
    // Test RRULE validation with timezone
    assert!(RecurrenceManager::validate_rrule("FREQ=WEEKLY;BYDAY=MO", "America/New_York").is_ok());
    assert!(RecurrenceManager::validate_rrule("FREQ=DAILY", "Invalid/Timezone").is_err());
}

#[tokio::test]
async fn test_materialization_workflow() {
    let (repo, _temp_dir) = setup_test_db().await;
    
    // Test materialization manager configuration
    let config = MaterializationConfig {
        lookahead_days: 14,
        min_upcoming_instances: 2,
        max_batch_size: 50,
        enable_catchup: true,
        materialization_grace_days: 1,
    };
    
    let manager = MaterializationManager::new(config.clone());
    assert_eq!(manager.config().lookahead_days, 14);
    assert_eq!(manager.config().min_upcoming_instances, 2);
    assert_eq!(manager.config().enable_catchup, true);
    
    // Test window calculation with different filters
    let filters = vec![Filter::DueDate(DueDate::Today)];
    let (start, end) = manager.calculate_window_for_filters(&filters);
    
    let now = Utc::now();
    let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let today_end = now.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
    
    // Should narrow window to today
    assert!(start >= today_start);
    assert!(end <= today_end);
    
    // Test with before filter
    let future_date = now + Duration::days(5);
    let before_filters = vec![Filter::DueDate(DueDate::Before(future_date))];
    let (_, end_before) = manager.calculate_window_for_filters(&before_filters);
    assert!(end_before <= future_date);
    
    // Test materialization summary
    let mut summary = MaterializationSummary::default();
    assert_eq!(summary.series_processed, 0);
    assert_eq!(summary.instances_created, 0);
    assert!(summary.errors.is_empty());
    
    summary.series_processed = 5;
    summary.instances_created = 15;
    summary.errors.push("Test error".to_string());
    
    let cloned_summary = summary.clone();
    assert_eq!(cloned_summary.series_processed, 5);
    assert_eq!(cloned_summary.instances_created, 15);
    assert_eq!(cloned_summary.errors.len(), 1);
}