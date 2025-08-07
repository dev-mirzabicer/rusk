use chrono::{TimeZone, Utc}; // Make sure TimeZone is imported
use task_core::db::establish_connection;
use task_core::models::{CompletionResult, NewTaskData};
use task_core::repository::{Repository, SqliteRepository};

#[tokio::test]
// REMOVE the #[ignore] attribute to ensure this test always runs.
async fn test_recurrence_template_instance_model() {
    // 1. Setup
    let pool = establish_connection("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let repo = SqliteRepository::new(pool);

    // 2. Create a recurring task with a FIXED date for deterministic tests.
    let due_date = Utc.with_ymd_and_hms(2025, 8, 8, 0, 0, 0).unwrap();

    // THE FIX: Restore DTSTART so the parser works. Our robust `get_next_occurrence`
    // function is now able to handle the boundary conditions this creates.
    let rrule_str = format!("DTSTART:{}\nFREQ=DAILY", due_date.format("%Y%m%dT%H%M%SZ"));

    let new_task_data = NewTaskData {
        name: "Recurring Task".to_string(),
        description: Some("This is a daily recurring task".to_string()),
        due_at: Some(due_date),
        rrule: Some(rrule_str),
        ..Default::default()
    };

    println!("--- \"Test Start\" ---");
    println!("Initial due_date: {:?}", due_date);

    let template_task = repo.add_task(new_task_data).await.unwrap();
    assert!(template_task.recurrence_template_id.is_none());
    println!(
        "Template task created with due_at: {:?}",
        template_task.due_at
    );

    // 3. Complete the first instance (the template itself)
    let completion_result = repo.complete_task(template_task.id).await.unwrap();

    let (completed_task1, next_instance1) = match completion_result {
        CompletionResult::Recurring { completed, next } => (completed, next),
        _ => panic!("Expected a recurring completion result"),
    };

    println!("Completed task 1 at: {:?}", completed_task1.completed_at);
    println!(
        "Next instance 1 created with due_at: {:?}",
        next_instance1.due_at
    );

    // 4. Verify the first completion
    assert_eq!(completed_task1.id, template_task.id);
    assert!(completed_task1.completed_at.is_some());

    // 5. Verify the new instance
    let expected_due_1 = Utc.with_ymd_and_hms(2025, 8, 9, 0, 0, 0).unwrap();
    assert_eq!(next_instance1.name, "Recurring Task");
    assert_eq!(
        next_instance1.recurrence_template_id,
        Some(template_task.id)
    );
    assert_eq!(next_instance1.due_at, Some(expected_due_1));

    // 6. Complete the second instance
    println!("\n--- \"Completing second instance\" ---");
    let completion_result2 = repo.complete_task(next_instance1.id).await.unwrap();

    let (completed_task2, next_instance2) = match completion_result2 {
        CompletionResult::Recurring { completed, next } => (completed, next),
        _ => panic!("Expected a recurring completion result"),
    };

    println!("Completed task 2 at: {:?}", completed_task2.completed_at);
    println!(
        "Next instance 2 created with due_at: {:?}",
        next_instance2.due_at
    );

    // 7. Verify the second completion
    assert_eq!(completed_task2.id, next_instance1.id);
    assert!(completed_task2.completed_at.is_some());

    // 8. Verify the third instance
    let expected_due_2 = Utc.with_ymd_and_hms(2025, 8, 10, 0, 0, 0).unwrap();
    println!("Expected due_at for instance 2: {:?}", Some(expected_due_2));
    assert_eq!(next_instance2.name, "Recurring Task");
    assert_eq!(
        next_instance2.recurrence_template_id,
        Some(template_task.id)
    );
    assert_eq!(next_instance2.due_at, Some(expected_due_2));
}