-- Update executor references in projects table
UPDATE projects 
SET executor = 'opencode-ai' 
WHERE executor = 'charm-opencode';

-- Update executor references in tasks table  
UPDATE tasks 
SET executor_type = 'opencode-ai'
WHERE executor_type = 'charm-opencode';

-- Update executor references in task_attempts table
UPDATE task_attempts 
SET executor_type = 'opencode-ai'
WHERE executor_type = 'charm-opencode';