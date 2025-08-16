use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use rusk_core::recurrence::{RecurrenceManager, MaterializationManager, MaterializationConfig};
use rusk_core::models::{TaskSeries, Task, TaskStatus, TaskPriority, SeriesException};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use std::collections::HashMap;

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

fn bench_occurrence_generation(c: &mut Criterion) {
    let series = create_test_series("FREQ=DAILY;INTERVAL=1");
    let task = create_test_task();
    let exceptions = vec![];
    let manager = RecurrenceManager::new(series, task, exceptions).unwrap();
    
    let start = Utc::now();
    
    let mut group = c.benchmark_group("occurrence_generation");
    
    for days in [7, 30, 90, 365].iter() {
        let end = start + Duration::days(*days);
        group.bench_with_input(
            BenchmarkId::new("days", days),
            days,
            |b, _| {
                b.iter(|| {
                    manager.generate_occurrences_between(
                        black_box(start),
                        black_box(end)
                    ).unwrap()
                })
            }
        );
    }
    group.finish();
}

fn bench_occurrence_generation_with_exceptions(c: &mut Criterion) {
    let series = create_test_series("FREQ=DAILY;INTERVAL=1");
    let task = create_test_task();
    
    // Create exceptions for every 5th occurrence
    let mut exceptions = vec![];
    let start = Utc::now();
    for i in (0..30).step_by(5) {
        exceptions.push(SeriesException {
            series_id: series.id,
            occurrence_dt: start + Duration::days(i),
            exception_type: rusk_core::models::ExceptionType::Skip,
            exception_task_id: None,
            notes: None,
            created_at: Utc::now(),
        });
    }
    
    let manager = RecurrenceManager::new(series, task, exceptions).unwrap();
    let end = start + Duration::days(30);
    
    c.bench_function("occurrence_generation_with_exceptions", |b| {
        b.iter(|| {
            manager.generate_occurrences_between(
                black_box(start),
                black_box(end)
            ).unwrap()
        })
    });
}

fn bench_next_occurrence_calculation(c: &mut Criterion) {
    let series = create_test_series("FREQ=WEEKLY;BYDAY=MO,WE,FR");
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
    let test_rrules = vec![
        "FREQ=DAILY;INTERVAL=1",
        "FREQ=WEEKLY;BYDAY=MO,WE,FR",
        "FREQ=MONTHLY;BYMONTHDAY=1,15",
        "FREQ=YEARLY;BYMONTH=1,6;BYMONTHDAY=1",
    ];
    
    let mut group = c.benchmark_group("rrule_validation");
    
    for rrule in test_rrules {
        group.bench_with_input(
            BenchmarkId::new("rrule", rrule),
            rrule,
            |b, rrule| {
                b.iter(|| {
                    RecurrenceManager::validate_rrule(black_box(rrule), black_box("UTC")).unwrap()
                })
            }
        );
    }
    group.finish();
}

fn bench_rrule_normalization(c: &mut Criterion) {
    let rrule = "FREQ=DAILY;INTERVAL=1";
    let dtstart = Utc::now();
    let timezone = "America/New_York";
    
    c.bench_function("rrule_normalization", |b| {
        b.iter(|| {
            RecurrenceManager::normalize_rrule(
                black_box(rrule),
                black_box(dtstart),
                black_box(timezone)
            ).unwrap()
        })
    });
}

fn bench_materialization_window_calculation(c: &mut Criterion) {
    use rusk_core::models::{Filter, DueDate};
    
    let manager = MaterializationManager::with_defaults();
    
    let test_filters = vec![
        vec![],
        vec![Filter::DueDate(DueDate::Today)],
        vec![Filter::DueDate(DueDate::Before(Utc::now() + Duration::days(7)))],
        vec![Filter::DueDate(DueDate::After(Utc::now() - Duration::days(3)))],
        vec![Filter::DueDate(DueDate::Overdue)],
    ];
    
    let mut group = c.benchmark_group("window_calculation");
    
    for (i, filters) in test_filters.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("filter_set", i),
            filters,
            |b, filters| {
                b.iter(|| {
                    manager.calculate_window_for_filters(black_box(filters))
                })
            }
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_recurrence_manager_creation,
    bench_occurrence_generation,
    bench_occurrence_generation_with_exceptions,
    bench_next_occurrence_calculation,
    bench_rrule_validation,
    bench_rrule_normalization,
    bench_materialization_window_calculation
);
criterion_main!(benches);