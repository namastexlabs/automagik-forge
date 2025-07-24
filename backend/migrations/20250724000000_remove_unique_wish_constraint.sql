PRAGMA foreign_keys = ON;

-- Remove the unique constraint index for wish_id that was incorrectly added
-- wish_id is meant for grouping tasks, not uniqueness
DROP INDEX IF EXISTS unique_wish_per_project;