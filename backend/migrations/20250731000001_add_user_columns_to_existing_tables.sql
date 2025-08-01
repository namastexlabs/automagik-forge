PRAGMA foreign_keys = ON;

-- Add user tracking to existing projects table
ALTER TABLE projects ADD COLUMN created_by BLOB REFERENCES users(id);

-- Add user tracking to existing tasks table
ALTER TABLE tasks ADD COLUMN created_by BLOB REFERENCES users(id);
ALTER TABLE tasks ADD COLUMN assigned_to BLOB REFERENCES users(id);

-- Add user tracking to existing task_attempts table
ALTER TABLE task_attempts ADD COLUMN created_by BLOB REFERENCES users(id);

-- Performance indexes for user queries
CREATE INDEX idx_projects_created_by ON projects(created_by);
CREATE INDEX idx_tasks_created_by ON tasks(created_by);
CREATE INDEX idx_tasks_assigned_to ON tasks(assigned_to);
CREATE INDEX idx_task_attempts_created_by ON task_attempts(created_by);