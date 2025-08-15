-- Phase 1: Series-Based Recurrence Architecture - Database Schema Rollback
-- Migration: 20250816000000_series_schema.down.sql
-- Purpose: Rollback series-based recurrence to template-instance model
-- Note: This is primarily for development; production should not use rollbacks

-- Drop all new indexes first
DROP INDEX IF EXISTS idx_tasks_series_status;
DROP INDEX IF EXISTS idx_tasks_status_due;
DROP INDEX IF EXISTS idx_tasks_due_at_series;
DROP INDEX IF EXISTS idx_tasks_series_id;
DROP INDEX IF EXISTS idx_series_exceptions_lookup;
DROP INDEX IF EXISTS idx_series_exceptions_series_id;
DROP INDEX IF EXISTS idx_task_series_materialized;
DROP INDEX IF EXISTS idx_task_series_active;
DROP INDEX IF EXISTS idx_task_series_template_task_id;

-- Restore tasks table with legacy fields
PRAGMA foreign_keys = OFF;

-- Create tasks table with legacy recurrence fields
CREATE TABLE tasks_legacy (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    priority TEXT NOT NULL DEFAULT 'none',
    due_at TIMESTAMP,
    completed_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    project_id TEXT,
    parent_id TEXT,
    rrule TEXT,                                      -- Restored legacy field
    recurrence_template_id TEXT,                     -- Restored legacy field
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL,
    FOREIGN KEY (parent_id) REFERENCES tasks_legacy(id) ON DELETE CASCADE,
    FOREIGN KEY (recurrence_template_id) REFERENCES tasks_legacy(id) ON DELETE SET NULL
);

-- Copy data back from current tasks table (excluding series_id)
INSERT INTO tasks_legacy (
    id, name, description, status, priority, due_at, completed_at,
    created_at, updated_at, project_id, parent_id, rrule, recurrence_template_id
)
SELECT 
    id, name, description, status, priority, due_at, completed_at,
    created_at, updated_at, project_id, parent_id, NULL as rrule, NULL as recurrence_template_id
FROM tasks;

-- Drop current tasks table and rename legacy one
DROP TABLE tasks;
ALTER TABLE tasks_legacy RENAME TO tasks;

-- Re-enable foreign keys
PRAGMA foreign_keys = ON;

-- Drop the new series tables
DROP TABLE IF EXISTS series_exceptions;
DROP TABLE IF EXISTS task_series;

-- Restore original indexes
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_due_at ON tasks(due_at);
CREATE INDEX idx_tasks_project_id ON tasks(project_id);
CREATE INDEX idx_tasks_parent_id ON tasks(parent_id);