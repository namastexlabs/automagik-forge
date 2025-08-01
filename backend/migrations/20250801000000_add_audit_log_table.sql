-- Add audit log table for security event tracking
CREATE TABLE audit_log (
    id BLOB PRIMARY KEY,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'authentication', 'authorization', 'admin_action', 'user_management',
        'whitelist_change', 'token_access', 'security_violation',
        'config_change', 'data_access'
    )),
    user_id BLOB REFERENCES users(id),
    ip_address TEXT,
    user_agent TEXT,
    resource TEXT NOT NULL,
    action TEXT NOT NULL,
    result TEXT NOT NULL CHECK (result IN ('success', 'failure', 'error', 'blocked')),
    details TEXT, -- JSON serialized additional details
    severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    timestamp TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

-- Create indexes for efficient querying
CREATE INDEX idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_log_user_id ON audit_log(user_id);
CREATE INDEX idx_audit_log_event_type ON audit_log(event_type);
CREATE INDEX idx_audit_log_severity ON audit_log(severity);
CREATE INDEX idx_audit_log_result ON audit_log(result);

-- Create composite index for common query patterns
CREATE INDEX idx_audit_log_user_timestamp ON audit_log(user_id, timestamp);
CREATE INDEX idx_audit_log_type_timestamp ON audit_log(event_type, timestamp);