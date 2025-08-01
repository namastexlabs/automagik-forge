# Automagik-Forge Production Deployment Guide

This guide covers deploying automagik-forge securely in production environments with comprehensive security hardening.

## Table of Contents

1. [Production Environment Configuration](#production-environment-configuration)
2. [Security Hardening](#security-hardening)
3. [Database Security](#database-security)
4. [Deployment Methods](#deployment-methods)
5. [Monitoring and Alerting](#monitoring-and-alerting)
6. [Backup and Recovery](#backup-and-recovery)
7. [Incident Response](#incident-response)

## Production Environment Configuration

### Required Environment Variables

```bash
# Core Application Settings
NODE_ENV=production
RUST_LOG=info
DISABLE_TELEMETRY=false  # Set to true to disable analytics

# Server Configuration
HOST=0.0.0.0
PORT=3001
BASE_URL=https://your-domain.com
MCP_SSE_PORT=8889

# Security Configuration
JWT_SECRET=your-super-secure-jwt-secret-minimum-32-characters
GITHUB_TOKEN_ENCRYPTION_KEY=base64-encoded-32-byte-key
HTTPS=true

# Database Configuration
DATABASE_URL=sqlite:///app/data/db.sqlite
DATABASE_MAX_CONNECTIONS=10
DATABASE_CONNECTION_TIMEOUT=30

# GitHub OAuth Configuration
GITHUB_CLIENT_ID=your-production-github-oauth-client-id
GITHUB_CLIENT_SECRET=your-production-github-oauth-client-secret
OAUTH_CALLBACK_URL=https://your-domain.com/auth/github/callback

# CORS and Network Security
CORS_ORIGINS=https://your-domain.com,https://www.your-domain.com
ALLOWED_HOSTS=your-domain.com,www.your-domain.com

# Rate Limiting Configuration
RATE_LIMIT_ENABLED=true
RATE_LIMIT_PER_MINUTE=60
RATE_LIMIT_BURST=10
RATE_LIMIT_REDIS_URL=redis://redis:6379  # Optional: Redis for distributed rate limiting

# Audit and Monitoring
AUDIT_LOG_LEVEL=INFO
AUDIT_LOG_RETENTION_DAYS=90
SECURITY_ALERT_EMAIL=admin@your-domain.com

# Sentry Configuration (Optional but recommended)
SENTRY_DSN=https://your-sentry-dsn@sentry.io/project-id
SENTRY_ENVIRONMENT=production

# SSL/TLS Configuration
SSL_CERT_PATH=/etc/ssl/certs/your-domain.crt
SSL_KEY_PATH=/etc/ssl/private/your-domain.key
```

### Key Generation

Generate secure keys for production:

```bash
# Generate JWT Secret (32+ characters)
openssl rand -base64 32

# Generate GitHub Token Encryption Key
openssl rand -base64 32

# Generate Session Secret
openssl rand -base64 32
```

## Security Hardening

### 1. Network Security

#### Reverse Proxy Configuration (Nginx)

```nginx
server {
    listen 443 ssl http2;
    server_name your-domain.com;

    # SSL Configuration
    ssl_certificate /etc/ssl/certs/your-domain.crt;
    ssl_certificate_key /etc/ssl/private/your-domain.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-RSA-AES256-GCM-SHA384:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 1d;

    # Security Headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;
    add_header X-Frame-Options "DENY" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    add_header Content-Security-Policy "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; connect-src 'self' wss: https:; font-src 'self' data:; frame-ancestors 'none';" always;

    # Rate Limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
    limit_req_zone $binary_remote_addr zone=auth:10m rate=1r/s;

    # Main application
    location / {
        limit_req zone=api burst=20 nodelay;
        proxy_pass http://127.0.0.1:3001;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
    }

    # Authentication endpoints with stricter rate limiting
    location ~* ^/(api/auth|oauth) {
        limit_req zone=auth burst=5 nodelay;
        proxy_pass http://127.0.0.1:3001;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # MCP SSE endpoint
    location /mcp/sse {
        proxy_pass http://127.0.0.1:8889;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$server_name$request_uri;
}
```

#### Firewall Configuration

```bash
# UFW (Ubuntu)
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow ssh
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable

# iptables (Alternative)
iptables -A INPUT -p tcp --dport 22 -j ACCEPT
iptables -A INPUT -p tcp --dport 80 -j ACCEPT
iptables -A INPUT -p tcp --dport 443 -j ACCEPT
iptables -A INPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
iptables -A INPUT -j DROP
```

### 2. Application Security

#### Docker Security Configuration

```dockerfile
# Use minimal base image
FROM node:18-alpine AS builder

# Create non-root user
RUN addgroup -g 1001 -S automagik && \
    adduser -S automagik -u 1001

# Set working directory
WORKDIR /app

# Copy and build application
COPY package*.json ./
RUN npm ci --only=production && npm cache clean --force

COPY . .
RUN npm run build

# Production image
FROM node:18-alpine

# Install security updates
RUN apk update && apk upgrade && \
    apk add --no-cache dumb-init

# Create non-root user
RUN addgroup -g 1001 -S automagik && \
    adduser -S automagik -u 1001

# Set up directories
WORKDIR /app
RUN mkdir -p /app/data && \
    chown -R automagik:automagik /app

# Copy built application
COPY --from=builder --chown=automagik:automagik /app .

# Switch to non-root user
USER automagik

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3001/api/health || exit 1

# Use dumb-init as PID 1
ENTRYPOINT ["dumb-init", "--"]
CMD ["node", "dist/main.js"]
```

## Database Security

### SQLite Security Hardening

```sql
-- Enable foreign key constraints
PRAGMA foreign_keys = ON;

-- Set secure journal mode
PRAGMA journal_mode = WAL;

-- Enable full synchronous mode
PRAGMA synchronous = FULL;

-- Set reasonable cache size
PRAGMA cache_size = 10000;

-- Enable query planner stability
PRAGMA optimize;
```

### Database Backup Configuration

```bash
#!/bin/bash
# /etc/cron.hourly/backup-automagik-forge

# Configuration
DB_PATH="/app/data/db.sqlite"
BACKUP_DIR="/app/backups"
RETENTION_DAYS=30
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Create backup with integrity check
sqlite3 "$DB_PATH" ".backup $BACKUP_DIR/backup_$DATE.sqlite"

# Verify backup integrity
if sqlite3 "$BACKUP_DIR/backup_$DATE.sqlite" "PRAGMA integrity_check;" | grep -q "ok"; then
    echo "Backup $DATE completed successfully"
    
    # Compress backup
    gzip "$BACKUP_DIR/backup_$DATE.sqlite"
    
    # Clean old backups
    find "$BACKUP_DIR" -name "backup_*.sqlite.gz" -mtime +$RETENTION_DAYS -delete
else
    echo "Backup $DATE failed integrity check" >&2
    rm -f "$BACKUP_DIR/backup_$DATE.sqlite"
    exit 1
fi
```

## Deployment Methods

### 1. Docker Compose Deployment

```yaml
version: '3.8'

services:
  automagik-forge:
    build: .
    ports:
      - "3001:3001"
      - "8889:8889"
    environment:
      - NODE_ENV=production
      - DATABASE_URL=sqlite:///app/data/db.sqlite
      - JWT_SECRET=${JWT_SECRET}
      - GITHUB_TOKEN_ENCRYPTION_KEY=${GITHUB_TOKEN_ENCRYPTION_KEY}
      - GITHUB_CLIENT_ID=${GITHUB_CLIENT_ID}
      - GITHUB_CLIENT_SECRET=${GITHUB_CLIENT_SECRET}
      - BASE_URL=${BASE_URL}
      - CORS_ORIGINS=${CORS_ORIGINS}
    volumes:
      - ./data:/app/data
      - ./backups:/app/backups
    restart: unless-stopped
    security_opt:
      - no-new-privileges:true
    read_only: true
    tmpfs:
      - /tmp
    cap_drop:
      - ALL
    cap_add:
      - CHOWN
      - SETGID
      - SETUID

  redis:
    image: redis:7-alpine
    ports:
      - "127.0.0.1:6379:6379"
    volumes:
      - redis-data:/data
    restart: unless-stopped
    command: redis-server --appendonly yes --requirepass ${REDIS_PASSWORD}

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./ssl:/etc/ssl:ro
    depends_on:
      - automagik-forge
    restart: unless-stopped

volumes:
  redis-data:
```

### 2. Systemd Service Deployment

```ini
# /etc/systemd/system/automagik-forge.service
[Unit]
Description=Automagik Forge - Task Management Platform
After=network.target
Wants=network.target

[Service]
Type=simple
User=automagik
Group=automagik
WorkingDirectory=/opt/automagik-forge
ExecStart=/opt/automagik-forge/automagik-forge
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/automagik-forge/data
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
AmbientCapabilities=CAP_NET_BIND_SERVICE

# Environment
Environment=NODE_ENV=production
Environment=RUST_LOG=info
EnvironmentFile=/etc/automagik-forge/environment

# Limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

## Monitoring and Alerting

### 1. Health Check Endpoint

The application provides a comprehensive health check endpoint at `/api/health`:

```json
{
  "status": "healthy",
  "database": "connected",
  "authentication": "operational",
  "mcp_server": "running",
  "version": "0.2.3",
  "uptime": 3600,
  "active_sessions": 5,
  "rate_limit_status": "normal"
}
```

### 2. Monitoring Configuration

#### Prometheus Metrics

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'automagik-forge'
    static_configs:
      - targets: ['localhost:3001']
    metrics_path: '/metrics'
    scrape_interval: 10s
```

#### Grafana Dashboard

Create dashboards for:
- Request rate and response times
- Authentication success/failure rates
- Active sessions and user activity
- Rate limiting violations
- Security events from audit log
- Database performance metrics

### 3. Log Aggregation

#### Structured Logging Configuration

```json
{
  "version": 1,
  "disable_existing_loggers": false,
  "formatters": {
    "json": {
      "format": "%(asctime)s %(name)s %(levelname)s %(message)s",
      "class": "pythonjsonlogger.jsonlogger.JsonFormatter"
    }
  },
  "handlers": {
    "file": {
      "class": "logging.handlers.RotatingFileHandler",
      "filename": "/var/log/automagik-forge/app.log",
      "maxBytes": 104857600,
      "backupCount": 10,
      "formatter": "json"
    },
    "security": {
      "class": "logging.handlers.RotatingFileHandler",
      "filename": "/var/log/automagik-forge/security.log",
      "maxBytes": 104857600,
      "backupCount": 30,
      "formatter": "json"
    }
  },
  "loggers": {
    "automagik_forge.security": {
      "handlers": ["security"],
      "level": "INFO",
      "propagate": false
    }
  },
  "root": {
    "handlers": ["file"],
    "level": "INFO"
  }
}
```

## Backup and Recovery

### 1. Automated Backup Strategy

```bash
#!/bin/bash
# /usr/local/bin/automagik-forge-backup.sh

set -euo pipefail

# Configuration
DB_PATH="${DATABASE_PATH:-/app/data/db.sqlite}"
BACKUP_DIR="${BACKUP_DIR:-/app/backups}"
S3_BUCKET="${S3_BACKUP_BUCKET:-}"
RETENTION_LOCAL_DAYS=7
RETENTION_S3_DAYS=90
DATE=$(date +%Y%m%d_%H%M%S)

# Create local backup
echo "Creating backup: $DATE"
mkdir -p "$BACKUP_DIR"

# Stop application temporarily for consistent backup
systemctl stop automagik-forge

# Create backup
sqlite3 "$DB_PATH" ".backup $BACKUP_DIR/backup_$DATE.sqlite"

# Restart application
systemctl start automagik-forge

# Verify backup integrity
if sqlite3 "$BACKUP_DIR/backup_$DATE.sqlite" "PRAGMA integrity_check;" | grep -q "ok"; then
    echo "Backup $DATE integrity verified"
    
    # Compress backup
    gzip "$BACKUP_DIR/backup_$DATE.sqlite"
    
    # Upload to S3 if configured
    if [[ -n "$S3_BUCKET" ]]; then
        aws s3 cp "$BACKUP_DIR/backup_$DATE.sqlite.gz" "s3://$S3_BUCKET/backups/"
        echo "Backup uploaded to S3"
    fi
    
    # Clean old local backups
    find "$BACKUP_DIR" -name "backup_*.sqlite.gz" -mtime +$RETENTION_LOCAL_DAYS -delete
    
    # Clean old S3 backups
    if [[ -n "$S3_BUCKET" ]]; then
        aws s3 ls "s3://$S3_BUCKET/backups/" | while read -r line; do
            file_date=$(echo "$line" | awk '{print $1}')
            file_name=$(echo "$line" | awk '{print $4}')
            if [[ $(date -d "$file_date" +%s) -lt $(date -d "$RETENTION_S3_DAYS days ago" +%s) ]]; then
                aws s3 rm "s3://$S3_BUCKET/backups/$file_name"
            fi
        done
    fi
else
    echo "Backup $DATE failed integrity check" >&2
    rm -f "$BACKUP_DIR/backup_$DATE.sqlite"
    exit 1
fi
```

### 2. Recovery Procedures

#### Database Recovery

```bash
#!/bin/bash
# Recovery script

BACKUP_FILE="$1"
DB_PATH="${DATABASE_PATH:-/app/data/db.sqlite}"

if [[ -z "$BACKUP_FILE" ]]; then
    echo "Usage: $0 <backup_file>"
    exit 1
fi

echo "Stopping application..."
systemctl stop automagik-forge

echo "Creating recovery backup..."
cp "$DB_PATH" "$DB_PATH.recovery.$(date +%Y%m%d_%H%M%S)"

echo "Restoring from backup: $BACKUP_FILE"
if [[ "$BACKUP_FILE" == *.gz ]]; then
    gunzip -c "$BACKUP_FILE" > "$DB_PATH"
else
    cp "$BACKUP_FILE" "$DB_PATH"
fi

echo "Verifying restored database..."
if sqlite3 "$DB_PATH" "PRAGMA integrity_check;" | grep -q "ok"; then
    echo "Database restored successfully"
    systemctl start automagik-forge
else
    echo "Database restoration failed" >&2
    exit 1
fi
```

## Incident Response

### 1. Security Incident Response Plan

#### Immediate Response (0-1 hour)
1. **Identify and contain** the security incident
2. **Revoke compromised sessions** using admin panel or API
3. **Enable additional logging** if not already active
4. **Document** the incident timeline

#### Short-term Response (1-24 hours)
1. **Analyze audit logs** for scope of compromise
2. **Reset authentication credentials** if necessary
3. **Apply security patches** if vulnerability is identified
4. **Notify affected users** if data may be compromised

#### Long-term Response (1-7 days)
1. **Conduct thorough security review**
2. **Update security procedures** based on lessons learned
3. **Implement additional monitoring** to prevent similar incidents
4. **File incident report** with stakeholders

### 2. Emergency Procedures

#### Force Logout All Users

```bash
# Using direct database access
sqlite3 /app/data/db.sqlite "DELETE FROM user_sessions WHERE 1=1;"

# Using API (requires admin access)
curl -X POST https://your-domain.com/api/admin/sessions/revoke-all \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"reason": "security_incident"}'
```

#### Disable User Access

```bash
# Disable specific user
curl -X PATCH https://your-domain.com/api/admin/users/$USER_ID \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"is_whitelisted": false}'
```

#### Emergency Maintenance Mode

```bash
# Enable maintenance mode (returns 503 for all requests except admin)
export MAINTENANCE_MODE=true
systemctl restart automagik-forge
```

## Security Checklist

### Pre-Deployment Security Checklist

- [ ] **Environment Variables**: All sensitive data in environment variables
- [ ] **Encryption Keys**: Generated and securely stored
- [ ] **Database**: Properly configured with foreign key constraints
- [ ] **HTTPS**: Valid SSL certificate configured
- [ ] **CORS**: Restricted to specific domains
- [ ] **Rate Limiting**: Enabled and configured
- [ ] **Security Headers**: All security headers configured
- [ ] **Firewall**: Only necessary ports open
- [ ] **User Permissions**: Application runs as non-root user
- [ ] **Backups**: Automated backup system configured
- [ ] **Monitoring**: Health checks and alerting configured
- [ ] **Logging**: Structured logging with appropriate levels
- [ ] **Updates**: System packages up to date

### Post-Deployment Security Checklist

- [ ] **Health Check**: Application responding correctly
- [ ] **Authentication**: GitHub OAuth working properly
- [ ] **Rate Limiting**: Confirmed working with test requests
- [ ] **Audit Logging**: Security events being logged
- [ ] **Backups**: First backup completed successfully
- [ ] **Monitoring**: Metrics being collected correctly
- [ ] **SSL**: SSL configuration tested (SSLLabs A+ rating)
- [ ] **Security Scan**: Vulnerability scan completed
- [ ] **Penetration Test**: Basic security testing completed
- [ ] **Documentation**: Incident response procedures documented

## Troubleshooting

### Common Issues

#### High Memory Usage
- Check for memory leaks in long-running processes
- Monitor database connection pool usage
- Review audit log retention settings

#### Authentication Issues
- Verify GitHub OAuth configuration
- Check JWT secret configuration
- Review CORS settings for frontend domain

#### Rate Limiting False Positives
- Review rate limit thresholds
- Check for misconfigured proxy headers
- Verify user identification logic

#### Database Performance
- Run `PRAGMA optimize;` regularly
- Monitor database file size and WAL mode
- Check for missing indexes on frequently queried columns

### Performance Tuning

#### Database Optimization
```sql
-- Regular maintenance
PRAGMA optimize;
PRAGMA wal_checkpoint(TRUNCATE);
VACUUM;

-- Performance settings
PRAGMA cache_size = 20000;
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 268435456;
```

#### Application Tuning
- Configure appropriate worker thread counts
- Tune database connection pool size
- Optimize static file serving (consider CDN)
- Enable gzip compression in reverse proxy

This production deployment guide ensures a secure, scalable, and maintainable deployment of automagik-forge with comprehensive security measures and operational procedures.