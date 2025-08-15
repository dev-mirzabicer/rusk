-- Phase 1: Series-Based Recurrence Architecture - Database Schema
-- Migration: 20250816000000_series_schema.up.sql
-- Purpose: Transform from template-instance to series-based recurrence model

-- Create the task_series table for managing recurring series
CREATE TABLE task_series (
    id TEXT PRIMARY KEY NOT NULL,                    -- UUIDv7, time-ordered for performance
    template_task_id TEXT NOT NULL UNIQUE,          -- Foreign key to template task (unique constraint)
    rrule TEXT NOT NULL,                             -- Canonical RFC 5545 recurrence rule with DTSTART
    dtstart TIMESTAMP NOT NULL,                      -- Series start time in UTC
    timezone TEXT NOT NULL DEFAULT 'UTC',           -- IANA timezone name (e.g., "America/New_York")
    active BOOLEAN NOT NULL DEFAULT TRUE,           -- Whether series is currently generating instances
    last_materialized_until TIMESTAMP,              -- Boundary for idempotent materialization
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,  -- Series creation timestamp
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,  -- Last modification timestamp
    FOREIGN KEY (template_task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- Create the series_exceptions table for handling deviations from series pattern
CREATE TABLE series_exceptions (
    series_id TEXT NOT NULL,                        -- Foreign key to task_series
    occurrence_dt TIMESTAMP NOT NULL,               -- Original scheduled occurrence time (UTC)
    exception_type TEXT NOT NULL,                   -- Type of exception (skip|override|move)
    exception_task_id TEXT,                         -- Reference to custom task (for override/move)
    notes TEXT,                                     -- Optional explanation for the exception
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,  -- Exception creation timestamp
    PRIMARY KEY (series_id, occurrence_dt),
    FOREIGN KEY (series_id) REFERENCES task_series(id) ON DELETE CASCADE,
    FOREIGN KEY (exception_task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    -- Constraint: exception_task_id required for override/move, forbidden for skip
    CHECK (
        (exception_type = 'skip' AND exception_task_id IS NULL) OR
        (exception_type IN ('override', 'move') AND exception_task_id IS NOT NULL)
    )
);

-- Remove legacy recurrence fields from tasks table and add series_id
-- First, drop the foreign key constraint for recurrence_template_id
PRAGMA foreign_keys = OFF;

-- Create new tasks table without legacy fields but with series_id
CREATE TABLE tasks_new (
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
    series_id TEXT,                                  -- New: Optional foreign key to task_series (for instances only)
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL,
    FOREIGN KEY (parent_id) REFERENCES tasks_new(id) ON DELETE CASCADE,
    FOREIGN KEY (series_id) REFERENCES task_series(id) ON DELETE SET NULL
);

-- Copy data from old tasks table to new one (excluding rrule and recurrence_template_id)
INSERT INTO tasks_new (
    id, name, description, status, priority, due_at, completed_at, 
    created_at, updated_at, project_id, parent_id, series_id
)
SELECT 
    id, name, description, status, priority, due_at, completed_at,
    created_at, updated_at, project_id, parent_id, NULL as series_id
FROM tasks;

-- Drop the old tasks table and rename the new one
DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

-- Re-enable foreign keys
PRAGMA foreign_keys = ON;

-- Create all necessary indexes for performance

-- Primary performance indexes for task_series
CREATE INDEX idx_task_series_template_task_id ON task_series(template_task_id);
CREATE INDEX idx_task_series_active ON task_series(active);
CREATE INDEX idx_task_series_materialized ON task_series(last_materialized_until);

-- Performance indexes for series_exceptions
CREATE INDEX idx_series_exceptions_series_id ON series_exceptions(series_id);
CREATE INDEX idx_series_exceptions_lookup ON series_exceptions(series_id, occurrence_dt);

-- Enhanced indexes for tasks table with series support
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_due_at ON tasks(due_at);
CREATE INDEX idx_tasks_project_id ON tasks(project_id);
CREATE INDEX idx_tasks_parent_id ON tasks(parent_id);
CREATE INDEX idx_tasks_series_id ON tasks(series_id);
CREATE INDEX idx_tasks_due_at_series ON tasks(due_at, series_id);
CREATE INDEX idx_tasks_status_due ON tasks(status, due_at);
CREATE INDEX idx_tasks_series_status ON tasks(series_id, status);