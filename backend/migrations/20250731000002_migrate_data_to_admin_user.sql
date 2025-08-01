PRAGMA foreign_keys = ON;

-- This migration creates an initial admin user and assigns existing data to them
-- It reads from the config.json to extract GitHub information if available

-- Create initial admin user (placeholder values that will be updated by code)
-- Note: This will be updated by the migration code to use actual config.json values
INSERT INTO users (
    id, 
    github_id, 
    username, 
    email, 
    display_name, 
    is_admin, 
    is_whitelisted, 
    created_at, 
    updated_at
) VALUES (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || 
    substr(lower(hex(randomblob(2))), 2) || '-' || 
    substr('89ab', abs(random()) % 4 + 1, 1) || 
    substr(lower(hex(randomblob(2))), 2) || '-' || 
    lower(hex(randomblob(6))),
    0, -- Will be updated by migration code
    'admin', -- Will be updated by migration code  
    'admin@localhost', -- Will be updated by migration code
    'System Administrator', -- Will be updated by migration code
    TRUE,
    TRUE,
    datetime('now', 'subsec'),
    datetime('now', 'subsec')
);

-- Update existing projects to be owned by the admin user
UPDATE projects 
SET created_by = (SELECT id FROM users WHERE is_admin = TRUE LIMIT 1)
WHERE created_by IS NULL;

-- Update existing tasks to be created by the admin user
UPDATE tasks 
SET created_by = (SELECT id FROM users WHERE is_admin = TRUE LIMIT 1)
WHERE created_by IS NULL;

-- Update existing task_attempts to be created by the admin user
UPDATE task_attempts 
SET created_by = (SELECT id FROM users WHERE is_admin = TRUE LIMIT 1)
WHERE created_by IS NULL;