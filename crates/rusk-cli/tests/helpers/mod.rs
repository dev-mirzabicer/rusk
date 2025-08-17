use assert_cmd::Command;
use predicates::prelude::*;
use std::env;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test harness for running CLI commands with temporary databases
pub struct CliTestHarness {
    temp_dir: TempDir,
    db_path: PathBuf,
}

impl CliTestHarness {
    /// Create a new test harness with a temporary database
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        
        Self {
            temp_dir,
            db_path,
        }
    }
    
    /// Get a Command instance configured for testing
    pub fn command(&self) -> Command {
        let mut cmd = Command::cargo_bin("rusk").expect("Failed to find rusk binary");
        
        // Set the database path via environment variable
        cmd.env("RUSK_DATABASE_PATH", &self.db_path);
        
        cmd
    }
    
    /// Get the database path for this test instance
    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }
    
    /// Helper to run a command and assert success
    pub fn run_success(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        self.command()
            .args(args)
            .assert()
            .success()
    }
    
    /// Helper to run a command and assert failure
    pub fn run_failure(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        self.command()
            .args(args)
            .assert()
            .failure()
    }
    
    /// Helper to run a command and capture output
    pub fn run_and_capture(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        self.command()
            .args(args)
            .assert()
    }
}

/// Common test fixtures
pub struct TestFixtures;

impl TestFixtures {
    /// Sample task data for testing
    pub fn sample_task_args() -> Vec<&'static str> {
        vec!["add", "Sample Task", "--due", "tomorrow", "--priority", "medium"]
    }
    
    /// Sample project data for testing
    pub fn sample_project_args() -> Vec<&'static str> {
        vec!["project", "add", "Sample Project", "--description", "Test project"]
    }
    
    /// Sample recurring task data for testing
    pub fn sample_recurring_task_args() -> Vec<&'static str> {
        vec![
            "add", "Daily Task", 
            "--every", "daily", 
            "--at", "9:00 AM",
            "--timezone", "UTC"
        ]
    }
}

/// Utility functions for test assertions
pub mod assertions {
    use predicates::prelude::*;
    
    /// Predicate to check if output contains task table headers
    pub fn has_task_table_headers() -> impl Predicate<str> {
        predicate::str::contains("ID")
            .and(predicate::str::contains("Name"))
            .and(predicate::str::contains("Status"))
    }
    
    /// Predicate to check if output contains project table headers
    pub fn has_project_table_headers() -> impl Predicate<str> {
        predicate::str::contains("Name")
            .and(predicate::str::contains("Description"))
    }
    
    /// Predicate to check if output indicates successful task creation
    pub fn task_created_successfully() -> impl Predicate<str> {
        predicate::str::contains("✓")
            .or(predicate::str::contains("Created task"))
            .or(predicate::str::contains("Created recurring task"))
            .or(predicate::str::contains("Added"))
    }
    
    /// Predicate to check if output indicates successful task completion
    pub fn task_completed_successfully() -> impl Predicate<str> {
        predicate::str::contains("✅")
            .or(predicate::str::contains("Completed"))
            .or(predicate::str::contains("Done"))
    }
    
    /// Predicate to check for empty result set
    pub fn empty_result() -> impl Predicate<str> {
        predicate::str::contains("No tasks found")
            .or(predicate::str::contains("No results"))
            .or(predicate::str::is_empty())
    }
    
    /// Predicate to check for error messages
    pub fn has_error() -> impl Predicate<str> {
        predicate::str::contains("Error")
            .or(predicate::str::contains("error"))
            .or(predicate::str::contains("❌"))
    }
}