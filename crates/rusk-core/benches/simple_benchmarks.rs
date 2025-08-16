use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rusk_core::recurrence::{RecurrenceManager, MaterializationManager};
use rusk_core::models::{TaskSeries, Task, TaskStatus, TaskPriority};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;

fn create_test_series(rrule: &str) -> TaskSeries {
    TaskSeries {
        id: Uuid::now_v7(),
        template_task_id: Uuid::now_v7(),
        rrule: rrule.to_string(),
        dtstart: Utc::now(),
        timezone: "UTC".to_string(),
        active: true,
        last_materialized_until: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn create_test_task() -> Task {
    Task {
        id: Uuid::now_v7(),
        name: "Benchmark Task".to_string(),
        description: None,
        status: TaskStatus::Pending,
        priority: TaskPriority::None,
        due_at: Some(Utc::now()),
        completed_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        project_id: None,
        parent_id: None,
        series_id: None,
    }
}

fn bench_recurrence_manager_creation(c: &mut Criterion) {
    let series = create_test_series("FREQ=DAILY;INTERVAL=1");
    let task = create_test_task();
    let exceptions = vec![];

    c.bench_function("recurrence_manager_creation", |b| {
        b.iter(|| {
            RecurrenceManager::new(
                black_box(series.clone()),
                black_box(task.clone()),
                black_box(exceptions.clone())
            ).unwrap()
        })
    });
}

fn bench_occurrence_generation_daily(c: &mut Criterion) {
    let series = create_test_series("FREQ=DAILY;INTERVAL=1");
    let task = create_test_task();
    let exceptions = vec![];
    let manager = RecurrenceManager::new(series, task, exceptions).unwrap();
    
    let start = Utc::now();
    let end = start + Duration::days(30);
    
    c.bench_function("occurrence_generation_daily_30_days", |b| {
        b.iter(|| {
            manager.generate_occurrences_between(
                black_box(start),
                black_box(end)
            ).unwrap()
        })
    });
}

fn bench_occurrence_generation_weekly(c: &mut Criterion) {
    let series = create_test_series("FREQ=WEEKLY;BYDAY=MO,WE,FR");
    let task = create_test_task();
    let exceptions = vec![];
    let manager = RecurrenceManager::new(series, task, exceptions).unwrap();
    
    let start = Utc::now();
    let end = start + Duration::days(90);
    
    c.bench_function("occurrence_generation_weekly_90_days", |b| {
        b.iter(|| {
            manager.generate_occurrences_between(
                black_box(start),
                black_box(end)
            ).unwrap()
        })
    });
}

fn bench_next_occurrence_calculation(c: &mut Criterion) {
    let series = create_test_series("FREQ=DAILY;INTERVAL=1");
    let task = create_test_task();
    let exceptions = vec![];
    let manager = RecurrenceManager::new(series, task, exceptions).unwrap();
    
    let after = Utc::now();
    
    c.bench_function("next_occurrence_calculation", |b| {
        b.iter(|| {
            manager.next_occurrence_after(black_box(after)).unwrap()
        })
    });
}

fn bench_rrule_validation(c: &mut Criterion) {
    c.bench_function("rrule_validation", |b| {
        b.iter(|| {
            RecurrenceManager::validate_rrule(
                black_box("FREQ=DAILY;INTERVAL=1"), 
                black_box("UTC")
            ).unwrap()
        })
    });
}

fn bench_materialization_window_calculation(c: &mut Criterion) {
    use rusk_core::models::Filter;
    
    let manager = MaterializationManager::with_defaults();
    let filters = vec![];
    
    c.bench_function("materialization_window_calculation", |b| {
        b.iter(|| {
            manager.calculate_window_for_filters(black_box(&filters))
        })
    });
}

criterion_group!(
    benches,
    bench_recurrence_manager_creation,
    bench_occurrence_generation_daily,
    bench_occurrence_generation_weekly,
    bench_next_occurrence_calculation,
    bench_rrule_validation,
    bench_materialization_window_calculation
);
criterion_main!(benches);