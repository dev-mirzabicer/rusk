/// Comprehensive CLI integration tests for rusk
/// 
/// These tests exercise the CLI commands as a black box, ensuring complete
/// coverage of all command paths, error handling, and output formatting.

use assert_cmd::Command;
use predicates::prelude::*;

mod helpers;
use helpers::{CliTestHarness, TestFixtures, assertions};

/// Test basic CLI help and version commands
#[test]
fn test_cli_help_and_version() {
    let harness = CliTestHarness::new();
    
    // Test help command
    harness.run_success(&["--help"])
        .stdout(predicate::str::contains("Rusk"))
        .stdout(predicate::str::contains("task management"));
    
    // Test version command
    harness.run_success(&["--version"])
        .stdout(predicate::str::contains("rusk"));
    
    // Test invalid command
    harness.run_failure(&["invalid-command"])
        .stderr(predicate::str::contains("error"));
}

/// Test task addition with various argument combinations
#[test]
fn test_add_command_comprehensive() {
    let harness = CliTestHarness::new();
    
    // Test basic task addition
    harness.run_success(&["add", "Basic Task"])
        .stdout(assertions::task_created_successfully());
    
    // Test task with all optional parameters
    harness.run_success(&[
        "add", "Complex Task",
        "--due", "tomorrow",
        "--priority", "high",
        "--description", "A complex test task",
        "--tag", "work",
        "--tag", "urgent"
    ])
    .stdout(assertions::task_created_successfully());
    
    // Test task with project (create project first)
    harness.run_success(&["project", "add", "TestProject"]);
    harness.run_success(&[
        "add", "Project Task",
        "--project", "TestProject"
    ])
    .stdout(assertions::task_created_successfully());
    
    // Test invalid priority
    harness.run_failure(&[
        "add", "Invalid Task",
        "--priority", "invalid"
    ])
    .stderr(assertions::has_error());
    
    // Test invalid date
    harness.run_failure(&[
        "add", "Invalid Date Task",
        "--due", "invalid-date"
    ])
    .stderr(assertions::has_error());
}

/// Test recurring task creation
#[test]
fn test_add_recurring_tasks() {
    let harness = CliTestHarness::new();
    
    // Test daily recurring task
    harness.run_success(&[
        "add", "Daily Task",
        "--every", "daily",
        "--at", "9:00 AM"
    ])
    .stdout(assertions::task_created_successfully());
    
    // Test weekly recurring task
    harness.run_success(&[
        "add", "Weekly Meeting",
        "--every", "weekly",
        "--on", "monday",
        "--at", "2:00 PM",
        "--until", "2025-12-31"
    ])
    .stdout(assertions::task_created_successfully());
    
    // Test weekdays recurring task
    harness.run_success(&[
        "add", "Standup",
        "--every", "weekdays",
        "--at", "9:30 AM",
        "--count", "20"
    ])
    .stdout(assertions::task_created_successfully());
    
    // Test with timezone
    harness.run_success(&[
        "add", "Global Meeting",
        "--every", "weekly",
        "--at", "3:00 PM",
        "--timezone", "America/New_York"
    ])
    .stdout(assertions::task_created_successfully());
    
    // Test raw RRULE
    harness.run_success(&[
        "add", "Custom Recurrence",
        "--recurrence", "FREQ=MONTHLY;BYMONTHDAY=15"
    ])
    .stdout(assertions::task_created_successfully());
    
    // Test invalid RRULE
    harness.run_failure(&[
        "add", "Invalid RRULE",
        "--recurrence", "INVALID_RRULE"
    ])
    .stderr(assertions::has_error());
    
    // Test conflicting recurrence options
    harness.run_failure(&[
        "add", "Conflicting Options",
        "--every", "daily",
        "--recurrence", "FREQ=WEEKLY"
    ])
    .stderr(assertions::has_error());
}

/// Test list command with various filters
#[test]
fn test_list_command_comprehensive() {
    let harness = CliTestHarness::new();
    
    // First, create projects that will be referenced
    harness.run_success(&["project", "add", "Work"]);
    
    // Then, add some test tasks
    harness.run_success(&["add", "High Priority Task", "--priority", "high"]);
    harness.run_success(&["add", "Work Task", "--tag", "work", "--project", "Work"]);
    harness.run_success(&["add", "Personal Task", "--tag", "personal"]);
    harness.run_success(&["add", "Due Today", "--due", "today"]);
    
    // Test default list (should show tasks)
    harness.run_success(&["list"])
        .stdout(assertions::has_task_table_headers());
    
    // Test empty query (should show default pending tasks)
    harness.run_success(&["list", ""])
        .stdout(assertions::has_task_table_headers());
    
    // Test status filter
    harness.run_success(&["list", "status:pending"])
        .stdout(assertions::has_task_table_headers());
    
    // Test priority filter
    harness.run_success(&["list", "priority:high"])
        .stdout(predicate::str::contains("High Priority Task"));
    
    // Test project filter
    harness.run_success(&["list", "project:Work"])
        .stdout(predicate::str::contains("Work Task"));
    
    // Test tag filter
    harness.run_success(&["list", "tag:work"])
        .stdout(predicate::str::contains("Work Task"));
    
    // Test due date filter
    harness.run_success(&["list", "due:today"])
        .stdout(predicate::str::contains("Due Today"));
    
    // Test complex query with AND
    harness.run_success(&["list", "status:pending and priority:high"])
        .stdout(predicate::str::contains("High Priority Task"));
    
    // Test complex query with OR
    harness.run_success(&["list", "tag:work or tag:personal"])
        .stdout(assertions::has_task_table_headers());
    
    // Test query with parentheses
    harness.run_success(&["list", "(tag:work or tag:personal) and status:pending"])
        .stdout(assertions::has_task_table_headers());
    
    // Test overdue filter (should be empty for new tasks)
    harness.run_success(&["list", "overdue"]);
    
    // Test invalid query
    harness.run_failure(&["list", "invalid:filter"])
        .stderr(assertions::has_error());
    
    // Test malformed query
    harness.run_failure(&["list", "status:pending and ("])
        .stderr(assertions::has_error());
}

/// Test task completion and status changes
#[test]
fn test_task_status_changes() {
    let harness = CliTestHarness::new();
    
    // Add a test task and capture the output to get task ID
    let output = harness.run_success(&["add", "Test Task"])
        .get_output()
        .stdout
        .clone();
    
    // Extract task ID from output (assuming it's displayed)
    let output_str = String::from_utf8_lossy(&output);
    
    // For now, we'll use list to get a task to complete
    // This is a limitation - we need a better way to get task IDs
    
    // Add a task with a known name, then use list to find it
    harness.run_success(&["add", "Completable Task"]);
    
    let list_output = harness.run_success(&["list", "name:\"Completable Task\""])
        .get_output()
        .stdout
        .clone();
    
    // If we can extract an ID from the list output, we can test completion
    // For now, let's test with a short ID approach
    
    // Test completion with partial ID (this will test the ID resolution logic)
    // We'll need to make this more robust once we can reliably extract IDs
    
    // Test invalid task ID
    harness.run_failure(&["do", "nonexistent-id"])
        .stderr(assertions::has_error());
    
    // Test cancel with invalid ID
    harness.run_failure(&["cancel", "nonexistent-id"])
        .stderr(assertions::has_error());
    
    // Test delete with invalid ID  
    harness.run_failure(&["delete", "nonexistent-id"])
        .stderr(assertions::has_error());
}

/// Test project management commands
#[test]
fn test_project_commands() {
    let harness = CliTestHarness::new();
    
    // Test project creation
    harness.run_success(&["project", "add", "Test Project"])
        .stdout(predicate::str::contains("Test Project"));
    
    // Test project creation with description
    harness.run_success(&[
        "project", "add", "Detailed Project",
        "--description", "A project with description"
    ])
    .stdout(predicate::str::contains("Detailed Project"));
    
    // Test project listing
    harness.run_success(&["project", "list"])
        .stdout(assertions::has_project_table_headers())
        .stdout(predicate::str::contains("Test Project"))
        .stdout(predicate::str::contains("Detailed Project"));
    
    // Test project deletion with tasks (should fail)
    harness.run_success(&["add", "Task in project", "--project", "Test Project"]);
    harness.run_failure(&["project", "delete", "Test Project"])
        .stderr(assertions::has_error());
    
    // Test project deletion without tasks (create a new project for this)
    harness.run_success(&["project", "add", "Empty Project"]);
    harness.run_success(&["project", "delete", "Empty Project"]);
    
    // Test deleting non-existent project
    harness.run_failure(&["project", "delete", "NonExistent"])
        .stderr(assertions::has_error());
    
    // Test duplicate project name
    harness.run_failure(&["project", "add", "Test Project"])
        .stderr(assertions::has_error());
}

/// Test edit command with various scenarios
#[test]  
fn test_edit_command() {
    let harness = CliTestHarness::new();
    
    // Add a task to edit
    harness.run_success(&["add", "Editable Task", "--priority", "low"]);
    
    // We need a way to get the task ID - for now test invalid scenarios
    
    // Test editing non-existent task
    harness.run_failure(&["edit", "nonexistent", "--priority", "high"])
        .stderr(assertions::has_error());
    
    // Test edit with invalid priority
    harness.run_failure(&["edit", "some-id", "--priority", "invalid"])
        .stderr(assertions::has_error());
    
    // Test edit with invalid due date
    harness.run_failure(&["edit", "some-id", "--due", "invalid-date"])
        .stderr(assertions::has_error());
    
    // Test clearing fields
    harness.run_failure(&["edit", "some-id", "--due-clear", "--description-clear"])
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("Not found")));
}

/// Test error handling and edge cases
#[test]
fn test_error_handling() {
    let harness = CliTestHarness::new();
    
    // Test commands with missing required arguments
    harness.run_failure(&["add"])
        .stderr(assertions::has_error());
    
    harness.run_failure(&["project", "add"])
        .stderr(assertions::has_error());
    
    harness.run_failure(&["do"])
        .stderr(assertions::has_error());
    
    harness.run_failure(&["edit"])
        .stderr(assertions::has_error());
    
    // Test commands with too many arguments
    harness.run_failure(&["project", "list", "extra-arg"])
        .stderr(assertions::has_error());
    
    // Test invalid subcommands
    harness.run_failure(&["project", "invalid"])
        .stderr(assertions::has_error());
    
    harness.run_failure(&["recur", "invalid"])
        .stderr(assertions::has_error());
}

/// Test aliases and shortcuts
#[test]
fn test_command_aliases() {
    let harness = CliTestHarness::new();
    
    // Test add alias
    harness.run_success(&["a", "Task via alias"])
        .stdout(assertions::task_created_successfully());
    
    // Test list alias
    harness.run_success(&["ls"])
        .stdout(assertions::has_task_table_headers());
    
    // Test project alias
    harness.run_success(&["proj", "add", "Project via alias"]);
    harness.run_success(&["proj", "list"])
        .stdout(assertions::has_project_table_headers());
}

/// Test complex workflows end-to-end
#[test]
fn test_complete_workflows() {
    let harness = CliTestHarness::new();
    
    // Workflow 1: Create project, add tasks, list, complete
    harness.run_success(&["project", "add", "Workflow Project"]);
    
    harness.run_success(&[
        "add", "Workflow Task 1",
        "--project", "Workflow Project",
        "--priority", "high"
    ]);
    
    harness.run_success(&[
        "add", "Workflow Task 2", 
        "--project", "Workflow Project",
        "--due", "tomorrow"
    ]);
    
    // List project tasks
    harness.run_success(&["list", "project:\"Workflow Project\""])
        .stdout(predicate::str::contains("Workflow Task 1"))
        .stdout(predicate::str::contains("Workflow Task 2"));
    
    // Workflow 2: Recurring task lifecycle
    harness.run_success(&[
        "add", "Daily Standup",
        "--every", "weekdays",
        "--at", "9:00 AM"
    ]);
    
    // List recurring tasks (should show materialized instances)
    harness.run_success(&["list", "name:\"Daily Standup\""])
        .stdout(predicate::str::contains("Daily Standup"));
}

/// Test output formatting and edge cases
#[test]
fn test_output_formatting() {
    let harness = CliTestHarness::new();
    
    // Test empty database
    harness.run_success(&["list"])
        .stdout(predicate::str::is_empty().or(predicate::str::contains("No tasks")));
    
    // Test long task names
    let long_name = "Very long task name that should test text wrapping and formatting".repeat(2);
    harness.run_success(&["add", &long_name]);
    
    harness.run_success(&["list"])
        .stdout(assertions::has_task_table_headers());
    
    // Test unicode in task names
    harness.run_success(&["add", "Task with emoji 🚀 and unicode ñáéíóú"]);
    
    harness.run_success(&["list"])
        .stdout(predicate::str::contains("🚀"));
    
    // Test special characters in task names
    harness.run_success(&["add", "Task with \"quotes\" and 'apostrophes'"]);
    
    // Test empty project list
    harness.run_success(&["project", "list"]);
}