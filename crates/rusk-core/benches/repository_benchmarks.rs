use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use rusk_core::repository::{SqliteRepository, Repository};
use rusk_core::recurrence::MaterializationManager;
use rusk_core::models::{NewTaskData, TaskStatus, TaskPriority};
use rusk_core::query::{Query, Filter, DueDate};
use rusk_core::db::establish_connection;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use tokio::runtime::Runtime;
use std::sync::Arc;

async fn setup_test_repository() -> SqliteRepository {
    let pool = establish_connection(":memory:").await.unwrap();
    let materialization_manager = MaterializationManager::with_defaults();
    SqliteRepository::new(pool, materialization_manager)
}

async fn populate_test_data(repo: &SqliteRepository, task_count: usize) -> Vec<Uuid> {
    let mut task_ids = Vec::new();
    
    for i in 0..task_count {
        let task_data = NewTaskData {
            name: format!("Task {}", i),
            description: Some(format!("Description for task {}", i)),
            priority: Some(if i % 3 == 0 { TaskPriority::High } else { TaskPriority::None }),
            due_at: Some(Utc::now() + Duration::days(i as i64 % 30)),
            project_name: if i % 5 == 0 { Some(format!("Project {}", i / 5)) } else { None },
            project_id: None,
            tags: if i % 4 == 0 { vec![format!("tag{}", i % 3)] } else { vec![] },
            depends_on: None,
            parent_id: None,
            rrule: None,
            timezone: None,
            series_id: None,
        };
        
        let task = repo.add_task(task_data).await.unwrap();
        task_ids.push(task.id);
    }
    
    task_ids
}

fn bench_task_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("task_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = setup_test_repository().await;
                
                let task_data = NewTaskData {
                    name: "Benchmark Task".to_string(),
                    description: Some("A task for benchmarking".to_string()),
                    priority: Some(TaskPriority::Medium),
                    due_at: Some(Utc::now() + Duration::days(1)),
                    project_name: None,
                    project_id: None,
                    tags: vec!["benchmark".to_string()],
                    depends_on: None,
                    parent_id: None,
                    rrule: None,
                    timezone: None,
                    series_id: None,
                };
                
                black_box(repo.add_task(task_data).await.unwrap())
            })
        })
    });
}

fn bench_task_lookup_by_id(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let (repo, task_ids) = rt.block_on(async {
        let repo = setup_test_repository().await;
        let task_ids = populate_test_data(&repo, 100).await;
        (repo, task_ids)
    });
    
    let repo = Arc::new(repo);
    
    c.bench_function("task_lookup_by_id", |b| {
        b.to_async(&rt).iter(|| {
            let repo = Arc::clone(&repo);
            let id = task_ids[fastrand::usize(..task_ids.len())];
            async move {
                black_box(repo.find_task_by_id(id).await.unwrap())
            }
        })
    });
}

fn bench_task_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let repo = rt.block_on(async {
        let repo = setup_test_repository().await;
        let _task_ids = populate_test_data(&repo, 1000).await;
        repo
    });
    
    let repo = Arc::new(repo);
    
    let test_queries = vec![
        ("all_tasks", Query::Filter(Filter::Status(TaskStatus::Pending))),
        ("high_priority", Query::Filter(Filter::Priority(TaskPriority::High))),
        ("due_today", Query::Filter(Filter::DueDate(DueDate::Today))),
        ("overdue", Query::Filter(Filter::DueDate(DueDate::Overdue))),
        ("complex_and", Query::And(
            Box::new(Query::Filter(Filter::Status(TaskStatus::Pending))),
            Box::new(Query::Filter(Filter::Priority(TaskPriority::High)))
        )),
        ("complex_or", Query::Or(
            Box::new(Query::Filter(Filter::DueDate(DueDate::Today))),
            Box::new(Query::Filter(Filter::DueDate(DueDate::Overdue)))
        )),
    ];
    
    let mut group = c.benchmark_group("task_queries");
    
    for (name, query) in test_queries {
        group.bench_with_input(
            BenchmarkId::new("query", name),
            &query,
            |b, query| {
                b.to_async(&rt).iter(|| {
                    let repo = Arc::clone(&repo);
                    let query = query.clone();
                    async move {
                        black_box(repo.find_tasks_with_details(&query).await.unwrap())
                    }
                })
            }
        );
    }
    group.finish();
}

fn bench_batch_task_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("batch_task_creation");
    
    for batch_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let repo = setup_test_repository().await;
                    
                    for i in 0..batch_size {
                        let task_data = NewTaskData {
                            name: format!("Batch Task {}", i),
                            description: None,
                            priority: TaskPriority::None,
                            due_at: Some(Utc::now() + Duration::days(i as i64)),
                            project_name: None,
                            tags: vec![],
                            depends_on: vec![],
                            parent_id: None,
                            rrule: None,
                            timezone: None,
                            series_id: None,
                        };
                        
                        black_box(repo.add_task(task_data).await.unwrap());
                    }
                })
            }
        );
    }
    group.finish();
}

fn bench_recurring_task_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("recurring_task_creation", |b| {
        b.to_async(&rt).iter(|| async {
            let repo = setup_test_repository().await;
            
            let task_data = NewTaskData {
                name: "Daily Recurring Task".to_string(),
                description: Some("A recurring task for benchmarking".to_string()),
                priority: TaskPriority::Medium,
                due_at: Some(Utc::now()),
                project_name: None,
                tags: vec!["recurring".to_string()],
                depends_on: vec![],
                parent_id: None,
                rrule: Some("FREQ=DAILY;INTERVAL=1".to_string()),
                timezone: Some("UTC".to_string()),
                series_id: None,
            };
            
            black_box(repo.add_task(task_data).await.unwrap())
        })
    });
}

fn bench_materialization_refresh(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let repo = rt.block_on(async {
        let repo = setup_test_repository().await;
        
        // Create several recurring tasks
        for i in 0..10 {
            let task_data = NewTaskData {
                name: format!("Daily Task {}", i),
                description: None,
                priority: TaskPriority::None,
                due_at: Some(Utc::now()),
                project_name: None,
                tags: vec![],
                depends_on: vec![],
                parent_id: None,
                rrule: Some("FREQ=DAILY;INTERVAL=1".to_string()),
                timezone: Some("UTC".to_string()),
                series_id: None,
            };
            
            repo.add_task(task_data).await.unwrap();
        }
        
        repo
    });
    
    let repo = Arc::new(repo);
    
    c.bench_function("materialization_refresh", |b| {
        b.to_async(&rt).iter(|| {
            let repo = Arc::clone(&repo);
            async move {
                let start = Utc::now();
                let end = start + Duration::days(30);
                black_box(repo.refresh_series_materialization(start, end).await.unwrap())
            }
        })
    });
}

criterion_group!(
    benches,
    bench_task_creation,
    bench_task_lookup_by_id,
    bench_task_queries,
    bench_batch_task_creation,
    bench_recurring_task_creation,
    bench_materialization_refresh
);
criterion_main!(benches);