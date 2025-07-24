PRAGMA foreign_keys = ON;

-- Add wish_id column to tasks table as required field
ALTER TABLE tasks ADD COLUMN wish_id TEXT NOT NULL DEFAULT '';

-- Create index for wish_id lookups
CREATE INDEX idx_tasks_wish_id ON tasks(wish_id);