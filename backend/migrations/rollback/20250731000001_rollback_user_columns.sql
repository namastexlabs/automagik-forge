PRAGMA foreign_keys = ON;

-- Remove performance indexes for user queries
DROP INDEX IF EXISTS idx_task_attempts_created_by;
DROP INDEX IF EXISTS idx_tasks_assigned_to;
DROP INDEX IF EXISTS idx_tasks_created_by;
DROP INDEX IF EXISTS idx_projects_created_by;

-- Remove user tracking columns from existing tables
-- Note: SQLite doesn't support DROP COLUMN directly
-- These would need to be done via table recreation if rollback is needed

-- Alternative approach: Create new tables without user columns and copy data
-- For safety, we'll document the process but not implement the destructive changes

-- To rollback completely, you would need to:
-- 1. CREATE TABLE projects_backup AS SELECT id, name, git_repo_path, setup_script, dev_script, cleanup_script, created_at, updated_at FROM projects;
-- 2. DROP TABLE projects;
-- 3. ALTER TABLE projects_backup RENAME TO projects;
-- 4. Repeat for tasks and task_attempts tables

-- WARNING: This rollback is destructive and will lose user assignment data
-- Only use if absolutely necessary and with proper data backup