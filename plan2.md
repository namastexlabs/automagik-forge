# Comprehensive WhatsApp Notification Integration Plan

## Executive Summary

**Current System Analysis**
The automagik-forge notification system demonstrates excellent architectural maturity with:
- Clean service layer separation (NotificationService, Configuration, Execution Monitor)  
- Existing rmcp client library (v0.3.0) providing complete MCP integration capabilities
- Rich task execution context available in `finalize_task_completion()`
- Type-safe configuration management via ts-rs ensuring frontend/backend alignment
- Well-established environment variable patterns for secure credential management

**Implementation Assessment: LOW RISK, HIGH REWARD**
- All architectural prerequisites satisfied
- Clear integration pathway identified through existing MCP patterns
- Comprehensive error handling and logging already established
- Minimal disruption to existing functionality

---

## Strategic Findings (Ordered by Priority)

### 🔴 CRITICAL: Notification Service Extensibility

**Current Issue:** The `NotificationService` uses procedural conditional compilation for platform-specific notifications, which doesn't scale well for adding new notification *channels* like WhatsApp.

**Current Pattern:**
```rust
// backend/src/services/notification_service.rs (Lines 124-132)
if cfg!(target_os = "macos") {
    self.send_macos_notification(title, message).await;
} else if cfg!(target_os = "linux") && !crate::utils::is_wsl2() {
    self.send_linux_notification(title, message).await;
} else if cfg!(target_os = "windows") || (cfg!(target_os = "linux") && crate::utils::is_wsl2()) {
    self.send_windows_notification(title, message).await;
}
```

**Strategic Solution:** Implement trait-based notification strategy pattern.

```rust
#[async_trait::async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, payload: &NotificationPayload) -> anyhow::Result<()>;
}

pub struct WhatsAppNotifier {
    mcp_client: McpClient,
    config: WhatsAppConfig,
}

impl NotificationService {
    pub fn new(notifiers: Vec<Box<dyn Notifier>>) -> Self {
        // Configure with multiple notification channels
    }
}
```

### 🟡 HIGH: Secure Configuration Management

**Current Configuration Split:**
- User preferences: `backend/src/models/config.rs` (push_notifications: bool)
- Environment secrets: `.env.example` (operational configuration)

**WhatsApp Integration Requirements:**
```rust
// backend/src/models/config.rs - User-facing toggle
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
pub struct NotificationSettings {
    pub desktop: bool,
    pub whatsapp: bool,
}

// Environment-only configuration
#[derive(Debug, Clone)]
pub struct WhatsAppConfig {
    pub base_url: String,
    pub api_key: String,
    pub instance: String,
    pub fixed_recipient: Option<String>,
    pub timeout_ms: u64,
}
```

**Environment Variables:**
```bash
# .env.example additions
WHATSAPP_NOTIFICATIONS_ENABLED=true
EVOLUTION_API_BASE_URL=http://192.168.112.142:8080
EVOLUTION_API_API_KEY=BEE0266C2040-4D83-8FAA-A9A3EF89DDEF
EVOLUTION_API_INSTANCE=SofIA
EVOLUTION_API_FIXED_RECIPIENT=
WHATSAPP_MCP_SERVER_TIMEOUT=30000
```

### 🟡 HIGH: Enhanced Notification Content

**Current Limitation:** Basic string formatting in `execution_monitor.rs` doesn't support rich, channel-specific content.

**Strategic Solution:** Notification payload builder pattern.

```rust
#[derive(Debug, Clone)]
pub struct TaskCompletionPayload {
    pub task: Task,
    pub task_attempt: TaskAttempt,
    pub project: Project,
    pub success: bool,
    pub exit_code: Option<i64>,
    pub duration: Option<Duration>,
    pub error_summary: Option<String>,
}

impl TaskCompletionPayload {
    pub fn to_plain_text(&self) -> String {
        // Desktop notification format
    }
    
    pub fn to_whatsapp_markdown(&self) -> String {
        format!(
            "*{}* {}\n\n📋 *Task:* {}\n🔗 *Wish ID:* {}\n🌿 *Branch:* {}\n⚙️ *Executor:* {}\n⏱️ *Duration:* {}s\n\n🔗 [View Task]({})",
            if self.success { "✅ SUCCESS" } else { "❌ FAILED" },
            self.task.title,
            self.task.id,
            self.task.wish_id,
            self.task_attempt.branch,
            self.task_attempt.executor.as_deref().unwrap_or("default"),
            self.duration.map(|d| d.as_secs()).unwrap_or(0),
            self.generate_task_url()
        )
    }
    
    fn generate_task_url(&self) -> String {
        format!("http://localhost:3333/project/{}/task/{}", 
                self.project.id, self.task.id)
    }
}
```

---

## Step-by-Step Implementation Plan

### Phase 1: Backend Core Implementation

#### Step 1.1: Update Configuration Models
```rust
// backend/src/models/config.rs
// Replace: pub push_notifications: bool,
#[serde(default)]
pub notifications: NotificationSettings,

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct NotificationSettings {
    pub desktop: bool,
    pub whatsapp: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            desktop: true,
            whatsapp: false,
        }
    }
}
```

#### Step 1.2: Create WhatsApp Configuration
```rust
// backend/src/services/whatsapp_config.rs
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone)]
pub struct WhatsAppConfig {
    pub base_url: String,
    pub api_key: String,
    pub instance: String,
    pub fixed_recipient: Option<String>,
    pub timeout_ms: u64,
    pub include_task_url: bool,
}

impl WhatsAppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            base_url: env::var("EVOLUTION_API_BASE_URL")?,
            api_key: env::var("EVOLUTION_API_API_KEY")?,
            instance: env::var("EVOLUTION_API_INSTANCE")?,
            fixed_recipient: env::var("EVOLUTION_API_FIXED_RECIPIENT").ok(),
            timeout_ms: env::var("WHATSAPP_MCP_SERVER_TIMEOUT")
                .unwrap_or_else(|_| "30000".to_string())
                .parse()?,
            include_task_url: env::var("WHATSAPP_NOTIFICATION_INCLUDE_URL")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
        })
    }
}
```

#### Step 1.3: Implement MCP Client Integration
```rust
// backend/src/services/whatsapp_notifier.rs
use rmcp::client::{Client, Transport};
use serde_json::json;

pub struct WhatsAppNotifier {
    client: Client,
    config: WhatsAppConfig,
}

impl WhatsAppNotifier {
    pub async fn new(config: WhatsAppConfig) -> anyhow::Result<Self> {
        let transport = Transport::sse(config.base_url.clone())
            .with_timeout(Duration::from_millis(config.timeout_ms));
            
        let client = Client::new(transport).await?;
        
        Ok(Self { client, config })
    }
}

#[async_trait::async_trait]
impl Notifier for WhatsAppNotifier {
    async fn notify(&self, payload: &TaskCompletionPayload) -> anyhow::Result<()> {
        let message = payload.to_whatsapp_markdown();
        
        let params = json!({
            "instance": self.config.instance,
            "message": message,
            "number": self.config.fixed_recipient,
            "linkPreview": true,
            "delay": 0
        });
        
        let result = self.client.call_tool(
            "send_text_message",
            Some(params)
        ).await?;
        
        tracing::info!("WhatsApp notification sent: {:?}", result);
        Ok(())
    }
}
```

#### Step 1.4: Update AppState Getters
```rust
// backend/src/app_state.rs
// Replace get_push_notifications_enabled with:
pub async fn get_notification_settings(&self) -> NotificationSettings {
    let config = self.config.read().await;
    config.notifications.clone()
}

pub async fn get_whatsapp_notifications_enabled(&self) -> bool {
    let config = self.config.read().await;
    config.notifications.whatsapp
}
```

### Phase 2: Environment Configuration

#### Step 2.1: Update .env.example
```bash
# Add to .env.example after line 38:

# WhatsApp Notification Configuration
# Enable/disable handled via UI settings - these are backend credentials only
EVOLUTION_API_BASE_URL=http://192.168.112.142:8080
EVOLUTION_API_API_KEY=BEE0266C2040-4D83-8FAA-A9A3EF89DDEF
EVOLUTION_API_INSTANCE=SofIA
# Optional: Fixed recipient for all notifications (leave empty for user-configured)
EVOLUTION_API_FIXED_RECIPIENT=
# MCP client timeout in milliseconds
WHATSAPP_MCP_SERVER_TIMEOUT=30000
# Include direct task URLs in notifications
WHATSAPP_NOTIFICATION_INCLUDE_URL=true
```

#### Step 2.2: Update Cargo.toml Dependencies
```toml
# backend/Cargo.toml - Add rmcp client dependency
[dependencies]
rmcp = "0.3.0"
```

#### Step 2.3: Update Initialization in main.rs
```rust
// backend/src/main.rs - in main() function
// Initialize notification service with multiple channels
let notification_service = {
    let mut notifiers: Vec<Box<dyn Notifier>> = vec![];
    
    // Always include desktop notifier
    notifiers.push(Box::new(DesktopNotifier::new()));
    
    // Add WhatsApp notifier if configured
    if let Ok(whatsapp_config) = WhatsAppConfig::from_env() {
        match WhatsAppNotifier::new(whatsapp_config).await {
            Ok(notifier) => {
                notifiers.push(Box::new(notifier));
                tracing::info!("WhatsApp notifications enabled");
            }
            Err(e) => {
                tracing::warn!("WhatsApp notifications disabled: {}", e);
            }
        }
    } else {
        tracing::info!("WhatsApp notifications not configured");
    }
    
    NotificationService::new(notifiers)
};
```

### Phase 3: Notification Content & Integration

#### Step 3.1: Update finalize_task_completion()
```rust
// backend/src/execution_monitor.rs - in finalize_task_completion()
// Replace current notification logic (lines 982-1010) with:
let payload = TaskCompletionPayload {
    task: task.clone(),
    task_attempt: task_attempt.clone(),
    project: project.clone(),
    success,
    exit_code: Some(exit_code),
    duration: start_time.map(|start| Instant::now() - start),
    error_summary: if !success { 
        Some(format!("Process exited with code {}", exit_code)) 
    } else { 
        None 
    },
};

// Send notifications through all configured channels
if let Err(e) = notification_service.notify_all(&payload).await {
    tracing::error!("Failed to send notifications: {}", e);
}
```

#### Step 3.2: Enhanced NotificationService
```rust
// backend/src/services/notification_service.rs
impl NotificationService {
    pub async fn notify_all(&self, payload: &TaskCompletionPayload) -> anyhow::Result<()> {
        let results = futures::future::join_all(
            self.notifiers.iter().map(|notifier| notifier.notify(payload))
        ).await;
        
        for (i, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                tracing::error!("Notifier {} failed: {}", i, e);
            }
        }
        
        Ok(())
    }
}
```

### Phase 4: Frontend Integration

#### Step 4.1: Regenerate Types
```bash
npm run generate-types
```

#### Step 4.2: Update Settings UI
```typescript
// frontend/src/pages/Settings.tsx - Update notification section
const { notifications } = config;

return (
  <div className="space-y-4">
    <div className="flex items-center justify-between">
      <Label htmlFor="desktop-notifications">Desktop Notifications</Label>
      <Switch
        id="desktop-notifications"
        checked={notifications.desktop}
        onCheckedChange={(checked) => 
          updateConfig({ 
            notifications: { ...notifications, desktop: checked } 
          })
        }
      />
    </div>
    
    <div className="flex items-center justify-between">
      <Label htmlFor="whatsapp-notifications">WhatsApp Notifications</Label>
      <Switch
        id="whatsapp-notifications"
        checked={notifications.whatsapp}
        onCheckedChange={(checked) => 
          updateConfig({ 
            notifications: { ...notifications, whatsapp: checked } 
          })
        }
      />
    </div>
  </div>
);
```

---

## File Changes Summary

### New Files to Create:
1. `backend/src/services/whatsapp_config.rs` - WhatsApp configuration management
2. `backend/src/services/whatsapp_notifier.rs` - MCP client integration
3. `backend/src/services/notification_payload.rs` - Rich notification content
4. `backend/tests/whatsapp_notifier_tests.rs` - Unit tests
5. `backend/tests/notification_integration_tests.rs` - Integration tests

### Files to Modify:
1. `backend/src/models/config.rs` - Replace push_notifications with notifications object
2. `backend/src/services/notification_service.rs` - Refactor to trait-based architecture  
3. `backend/src/app_state.rs` - Update notification getters
4. `backend/src/execution_monitor.rs` - Enhanced notification payload
5. `backend/src/main.rs` - Initialize WhatsApp notifier
6. `backend/Cargo.toml` - Add rmcp dependency
7. `.env.example` - Add WhatsApp environment variables
8. `frontend/src/pages/Settings.tsx` - Update UI for new notification settings

---

## Testing Strategy

### Unit Tests
```rust
// backend/tests/whatsapp_notifier_tests.rs
#[tokio::test]
async fn test_whatsapp_notification_formatting() {
    let payload = create_test_payload();
    let content = payload.to_whatsapp_markdown();
    
    assert!(content.contains("✅ SUCCESS"));
    assert!(content.contains("View Task"));
    assert!(content.contains("test-wish-id"));
}

#[tokio::test]
async fn test_whatsapp_config_from_env() {
    env::set_var("EVOLUTION_API_BASE_URL", "http://test.com");
    env::set_var("EVOLUTION_API_API_KEY", "test-key");
    env::set_var("EVOLUTION_API_INSTANCE", "test-instance");
    
    let config = WhatsAppConfig::from_env().unwrap();
    assert_eq!(config.base_url, "http://test.com");
}
```

### Integration Tests
```rust
// backend/tests/notification_integration_tests.rs
#[tokio::test]
async fn test_notification_service_with_multiple_channels() {
    let mock_notifiers = vec![
        Box::new(MockDesktopNotifier::new()),
        Box::new(MockWhatsAppNotifier::new()),
    ];
    
    let service = NotificationService::new(mock_notifiers);
    let payload = create_test_payload();
    
    assert!(service.notify_all(&payload).await.is_ok());
}
```

---

## Risk Assessment & Mitigation

### Low Risk Factors ✅
- **Existing MCP Infrastructure**: rmcp client library integration required
- **Clear Integration Points**: Well-defined notification trigger in `finalize_task_completion()`
- **Type Safety**: ts-rs ensures frontend/backend consistency
- **Graceful Degradation**: Notifications can fail without affecting core functionality

### Mitigation Strategies
1. **Configuration Validation**: Fail gracefully if WhatsApp credentials missing
2. **Timeout Handling**: 30-second timeout prevents hanging notifications
3. **Error Logging**: Comprehensive logging for troubleshooting
4. **Backward Compatibility**: Desktop notifications remain default/fallback

---

## Deployment Checklist

### Environment Setup
- [ ] Add WhatsApp credentials to production environment
- [ ] Configure Evolution API MCP server accessibility
- [ ] Verify MCP server endpoint reachability
- [ ] Test notification delivery in staging environment

### Application Changes
- [ ] Deploy backend with new notification service
- [ ] Verify type generation and frontend compatibility
- [ ] Update user documentation for WhatsApp notifications
- [ ] Monitor notification delivery success rates

### Rollback Plan
- [ ] Disable WhatsApp notifications via environment variables
- [ ] Revert to desktop-only notifications
- [ ] Monitor application stability

---

## Sample WhatsApp Notification Format

```
✅ SUCCESS Implement user authentication system

📋 Task: auth-task-001
🔗 Wish ID: user-auth-wish-123
🌿 Branch: feature/auth-system
⚙️ Executor: claude-code
⏱️ Duration: 45s

🔗 [View Task](http://localhost:3333/project/proj-001/task/auth-task-001)

Final Output:
Successfully implemented OAuth2 authentication with GitHub integration. Added secure session management and user authorization middleware.
```

---

This comprehensive plan provides a strategic, low-risk approach to integrating WhatsApp notifications while maintaining the architectural integrity of automagik-forge. The implementation leverages existing patterns and follows established conventions for maximum reliability and maintainability.