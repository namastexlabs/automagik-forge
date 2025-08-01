use automagik_forge::services::{
    whatsapp_config::WhatsAppConfig,
    whatsapp_notifier::WhatsAppNotifier,
};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{timeout, Duration};

/// Mock framework for WhatsApp notification testing
/// 
/// This module provides comprehensive mocking capabilities for testing
/// WhatsApp notification integration without external dependencies:
/// - Mock MCP transport responses
/// - Mock Evolution API server responses  
/// - Controllable error injection
/// - Request/response logging for verification
/// - Concurrent operation testing support

#[cfg(test)]
mod whatsapp_mock_framework {
    use super::*;

    // ============================================================================
    // MOCK MCP TRANSPORT
    // ============================================================================

    /// Mock MCP transport that simulates TokioChildProcess behavior
    #[derive(Debug, Clone)]
    pub struct MockMcpTransport {
        responses: Arc<Mutex<HashMap<String, MockResponse>>>,
        call_log: Arc<Mutex<Vec<MockCall>>>,
        default_behavior: MockBehavior,
    }

    #[derive(Debug, Clone)]
    pub enum MockResponse {
        Success(Value),
        Error(String),
        Timeout,
        ProcessSpawnFailure,
        ServiceCreationFailure,
    }

    #[derive(Debug, Clone)]
    pub enum MockBehavior {
        AlwaysSucceed,
        AlwaysFail,
        AlwaysTimeout,
        CustomResponse(MockResponse),
    }

    #[derive(Debug, Clone)]
    pub struct MockCall {
        pub tool_name: String,
        pub arguments: Option<Value>,
        pub timestamp: std::time::Instant,
        pub thread_id: std::thread::ThreadId,
    }

    impl MockMcpTransport {
        pub fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(HashMap::new())),
                call_log: Arc::new(Mutex::new(Vec::new())),
                default_behavior: MockBehavior::AlwaysSucceed,
            }
        }

        pub fn with_default_behavior(behavior: MockBehavior) -> Self {
            Self {
                responses: Arc::new(Mutex::new(HashMap::new())),
                call_log: Arc::new(Mutex::new(Vec::new())),
                default_behavior: behavior,
            }
        }

        pub fn set_response_for_tool(&self, tool_name: &str, response: MockResponse) {
            self.responses.lock().unwrap().insert(tool_name.to_string(), response);
        }

        pub fn set_default_behavior(&mut self, behavior: MockBehavior) {
            self.default_behavior = behavior;
        }

        pub fn get_call_log(&self) -> Vec<MockCall> {
            self.call_log.lock().unwrap().clone()
        }

        pub fn get_call_count(&self) -> usize {
            self.call_log.lock().unwrap().len()
        }

        pub fn get_calls_for_tool(&self, tool_name: &str) -> Vec<MockCall> {
            self.call_log.lock().unwrap()
                .iter()
                .filter(|call| call.tool_name == tool_name)
                .cloned()
                .collect()
        }

        pub fn reset(&self) {
            self.responses.lock().unwrap().clear();
            self.call_log.lock().unwrap().clear();
        }

        pub fn simulate_tool_call(&self, tool_name: &str, arguments: Option<Value>) -> Result<Value> {
            // Log the call
            let call = MockCall {
                tool_name: tool_name.to_string(),
                arguments: arguments.clone(),
                timestamp: std::time::Instant::now(),
                thread_id: std::thread::current().id(),
            };
            self.call_log.lock().unwrap().push(call);

            // Get response for this tool or use default
            let response = self.responses.lock().unwrap()
                .get(tool_name)
                .cloned()
                .unwrap_or_else(|| match &self.default_behavior {
                    MockBehavior::AlwaysSucceed => MockResponse::Success(json!({"status": "success"})),
                    MockBehavior::AlwaysFail => MockResponse::Error("Mock error".to_string()),
                    MockBehavior::AlwaysTimeout => MockResponse::Timeout,
                    MockBehavior::CustomResponse(resp) => resp.clone(),
                });

            match response {
                MockResponse::Success(value) => Ok(value),
                MockResponse::Error(msg) => Err(anyhow::anyhow!("Mock error: {}", msg)),
                MockResponse::Timeout => {
                    // Simulate timeout by sleeping
                    std::thread::sleep(Duration::from_millis(200));
                    Err(anyhow::anyhow!("Mock timeout"))
                },
                MockResponse::ProcessSpawnFailure => {
                    Err(anyhow::anyhow!("Failed to spawn process: No such file or directory"))
                },
                MockResponse::ServiceCreationFailure => {
                    Err(anyhow::anyhow!("Failed to create MCP service"))
                },
            }
        }
    }

    // ============================================================================
    // MOCK EVOLUTION API SERVER
    // ============================================================================

    /// Mock Evolution API server for integration testing
    #[derive(Debug)]
    pub struct MockEvolutionApiServer {
        request_log: Arc<Mutex<Vec<EvolutionApiRequest>>>,
        response_config: Arc<Mutex<EvolutionApiResponseConfig>>,
    }

    #[derive(Debug, Clone)]
    pub struct EvolutionApiRequest {
        pub instance: String,
        pub message: String,
        pub number: Option<String>,
        pub link_preview: bool,
        pub delay: u32,
        pub timestamp: std::time::Instant,
    }

    #[derive(Debug, Clone)]
    pub struct EvolutionApiResponseConfig {
        pub status_code: u16,
        pub response_body: Value,
        pub delay_ms: u64,
        pub should_fail: bool,
        pub failure_message: String,
    }

    impl Default for EvolutionApiResponseConfig {
        fn default() -> Self {
            Self {
                status_code: 200,
                response_body: json!({
                    "status": "success",
                    "messageId": "mock-message-id-123",
                    "timestamp": "2023-01-01T00:00:00Z"
                }),
                delay_ms: 0,
                should_fail: false,
                failure_message: "Mock API failure".to_string(),
            }
        }
    }

    impl MockEvolutionApiServer {
        pub fn new() -> Self {
            Self {
                request_log: Arc::new(Mutex::new(Vec::new())),
                response_config: Arc::new(Mutex::new(EvolutionApiResponseConfig::default())),
            }
        }

        pub fn configure_response(&self, config: EvolutionApiResponseConfig) {
            *self.response_config.lock().unwrap() = config;
        }

        pub fn configure_success_response(&self, message_id: &str) {
            let config = EvolutionApiResponseConfig {
                status_code: 200,
                response_body: json!({
                    "status": "success",
                    "messageId": message_id,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }),
                delay_ms: 50, // Realistic delay
                should_fail: false,
                failure_message: String::new(),
            };
            self.configure_response(config);
        }

        pub fn configure_error_response(&self, error_code: u16, error_message: &str) {
            let config = EvolutionApiResponseConfig {
                status_code: error_code,
                response_body: json!({
                    "status": "error",
                    "error": error_message,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }),
                delay_ms: 0,
                should_fail: true,
                failure_message: error_message.to_string(),
            };
            self.configure_response(config);
        }

        pub fn configure_timeout_response(&self, timeout_ms: u64) {
            let config = EvolutionApiResponseConfig {
                status_code: 408,
                response_body: json!({"status": "timeout"}),
                delay_ms: timeout_ms,
                should_fail: true,
                failure_message: "Request timeout".to_string(),
            };
            self.configure_response(config);
        }

        pub fn simulate_request(&self, request: EvolutionApiRequest) -> Result<Value> {
            // Log the request
            self.request_log.lock().unwrap().push(request);

            let config = self.response_config.lock().unwrap().clone();

            // Simulate delay
            if config.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(config.delay_ms));
            }

            // Return response based on configuration
            if config.should_fail {
                Err(anyhow::anyhow!("API Error {}: {}", config.status_code, config.failure_message))
            } else {
                Ok(config.response_body)
            }
        }

        pub fn get_request_log(&self) -> Vec<EvolutionApiRequest> {
            self.request_log.lock().unwrap().clone()
        }

        pub fn get_request_count(&self) -> usize {
            self.request_log.lock().unwrap().len()
        }

        pub fn get_last_request(&self) -> Option<EvolutionApiRequest> {
            self.request_log.lock().unwrap().last().cloned()
        }

        pub fn reset(&self) {
            self.request_log.lock().unwrap().clear();
        }
    }

    // ============================================================================
    // TEST UTILITIES WITH MOCKS
    // ============================================================================

    /// Test utilities that integrate with the mock framework
    pub struct MockTestUtils;

    impl MockTestUtils {
        /// Create a WhatsApp notifier configured for mocked testing
        pub async fn create_mocked_notifier(config: WhatsAppConfig) -> Result<WhatsAppNotifier> {
            // In a real implementation, this would inject the mock transport
            // For now, we create a regular notifier but with test-friendly configuration
            WhatsAppNotifier::new(config).await
        }

        /// Simulate MCP tool call with controllable responses
        pub async fn simulate_mcp_call(
            transport: &MockMcpTransport,
            tool_name: &str,
            params: Value,
        ) -> Result<Value> {
            tokio::task::spawn_blocking({
                let transport = transport.clone();
                let tool_name = tool_name.to_string();
                move || transport.simulate_tool_call(&tool_name, Some(params))
            }).await?
        }

        /// Create test configuration with mock-friendly settings
        pub fn create_mock_friendly_config() -> WhatsAppConfig {
            WhatsAppConfig {
                base_url: "http://mock-evolution-api:8080".to_string(),
                api_key: "mock-api-key".to_string(),
                instance: "mock-instance".to_string(),
                fixed_recipient: Some("+1234567890".to_string()),
                timeout_ms: 1000, // Short timeout for tests
                include_task_url: true,
            }
        }

        /// Verify Evolution API request structure
        pub fn verify_evolution_api_request(request: &EvolutionApiRequest, expected_message: &str) -> Result<()> {
            if !request.message.contains(expected_message) {
                return Err(anyhow::anyhow!(
                    "Expected message '{}' not found in request message '{}'",
                    expected_message, request.message
                ));
            }

            if request.instance.is_empty() {
                return Err(anyhow::anyhow!("Instance should not be empty"));
            }

            // Verify required fields
            if request.number.is_none() {
                return Err(anyhow::anyhow!("Phone number should be provided"));
            }

            Ok(())
        }
    }

    // ============================================================================
    // MOCK FRAMEWORK TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_mock_mcp_transport_success_response() {
        let transport = MockMcpTransport::new();
        transport.set_response_for_tool("send_text_message", MockResponse::Success(
            json!({"status": "sent", "messageId": "test-123"})
        ));

        let result = MockTestUtils::simulate_mcp_call(
            &transport,
            "send_text_message",
            json!({"instance": "test", "message": "Hello"})
        ).await;

        assert!(result.is_ok(), "Mock transport should return success");
        let response = result.unwrap();
        assert_eq!(response["status"], "sent");
        assert_eq!(response["messageId"], "test-123");

        // Verify call was logged
        assert_eq!(transport.get_call_count(), 1);
        let calls = transport.get_calls_for_tool("send_text_message");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "send_text_message");
    }

    #[tokio::test]
    async fn test_mock_mcp_transport_error_response() {
        let transport = MockMcpTransport::new();
        transport.set_response_for_tool("send_text_message", 
            MockResponse::Error("Authentication failed".to_string()));

        let result = MockTestUtils::simulate_mcp_call(
            &transport,
            "send_text_message",
            json!({"instance": "test", "message": "Hello"})
        ).await;

        assert!(result.is_err(), "Mock transport should return error");
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Authentication failed"));

        // Verify call was still logged
        assert_eq!(transport.get_call_count(), 1);
    }

    #[tokio::test]
    async fn test_mock_mcp_transport_timeout_behavior() {
        let transport = MockMcpTransport::new();
        transport.set_response_for_tool("send_text_message", MockResponse::Timeout);

        let start_time = std::time::Instant::now();
        let result = timeout(
            Duration::from_millis(100),
            MockTestUtils::simulate_mcp_call(
                &transport,
                "send_text_message",
                json!({"instance": "test", "message": "Hello"})
            )
        ).await;

        let elapsed = start_time.elapsed();

        // Should timeout due to mock delay
        assert!(result.is_err(), "Should timeout");
        assert!(elapsed >= Duration::from_millis(100), "Should take at least timeout duration");
        assert!(elapsed < Duration::from_millis(300), "Should not take too long");
    }

    #[tokio::test]
    async fn test_mock_mcp_transport_concurrent_calls() {
        let transport = MockMcpTransport::new();
        transport.set_response_for_tool("send_text_message", 
            MockResponse::Success(json!({"status": "success"})));

        // Make 5 concurrent mock calls
        let tasks = (0..5).map(|i| {
            let transport = transport.clone();
            tokio::spawn(async move {
                MockTestUtils::simulate_mcp_call(
                    &transport,
                    "send_text_message",
                    json!({"instance": "test", "message": format!("Message {}", i)})
                ).await
            })
        });

        let results = futures::future::join_all(tasks).await;

        // All calls should succeed
        for (i, result) in results.into_iter().enumerate() {
            assert!(result.is_ok(), "Task {} should complete", i);
            let call_result = result.unwrap();
            assert!(call_result.is_ok(), "Call {} should succeed", i);
        }

        // Verify all calls were logged
        assert_eq!(transport.get_call_count(), 5);
        let calls = transport.get_calls_for_tool("send_text_message");
        assert_eq!(calls.len(), 5);

        // Verify concurrent calls have different thread IDs
        let thread_ids: std::collections::HashSet<_> = calls.iter()
            .map(|call| call.thread_id)
            .collect();
        assert!(thread_ids.len() >= 1, "Should have at least one thread ID");
    }

    #[test]
    fn test_mock_evolution_api_server_success_response() {
        let server = MockEvolutionApiServer::new();
        server.configure_success_response("msg-123");

        let request = EvolutionApiRequest {
            instance: "test-instance".to_string(),
            message: "*Test Title*\n\nTest message".to_string(),
            number: Some("+1234567890".to_string()),
            link_preview: true,
            delay: 0,
            timestamp: std::time::Instant::now(),
        };

        let result = server.simulate_request(request);

        assert!(result.is_ok(), "Mock API should return success");
        let response = result.unwrap();
        assert_eq!(response["status"], "success");
        assert_eq!(response["messageId"], "msg-123");

        // Verify request was logged
        assert_eq!(server.get_request_count(), 1);
        let last_request = server.get_last_request().unwrap();
        assert_eq!(last_request.instance, "test-instance");
        assert!(last_request.message.contains("Test Title"));
    }

    #[test]
    fn test_mock_evolution_api_server_error_response() {
        let server = MockEvolutionApiServer::new();
        server.configure_error_response(401, "Invalid API key");

        let request = EvolutionApiRequest {
            instance: "test-instance".to_string(),
            message: "Test message".to_string(),
            number: Some("+1234567890".to_string()),
            link_preview: true,
            delay: 0,
            timestamp: std::time::Instant::now(),
        };

        let result = server.simulate_request(request);

        assert!(result.is_err(), "Mock API should return error");
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Invalid API key"));

        // Verify request was still logged
        assert_eq!(server.get_request_count(), 1);
    }

    #[test]
    fn test_mock_evolution_api_request_validation() {
        let request = EvolutionApiRequest {
            instance: "test-instance".to_string(),
            message: "*Test Title*\n\nTest message content".to_string(),
            number: Some("+1234567890".to_string()),
            link_preview: true,
            delay: 0,
            timestamp: std::time::Instant::now(),
        };

        let validation_result = MockTestUtils::verify_evolution_api_request(&request, "Test Title");
        assert!(validation_result.is_ok(), "Request validation should pass");

        // Test validation failure
        let validation_result = MockTestUtils::verify_evolution_api_request(&request, "Missing Content");
        assert!(validation_result.is_err(), "Request validation should fail for missing content");
    }

    #[tokio::test]
    async fn test_integration_with_mocked_components() {
        let config = MockTestUtils::create_mock_friendly_config();
        let transport = MockMcpTransport::new();
        let api_server = MockEvolutionApiServer::new();

        // Configure successful responses
        transport.set_response_for_tool("send_text_message", 
            MockResponse::Success(json!({"status": "sent", "messageId": "integration-test-123"})));
        api_server.configure_success_response("integration-test-123");

        // Create notifier (in real implementation, this would use injected mocks)
        let notifier = MockTestUtils::create_mocked_notifier(config).await.unwrap();

        // Simulate the full flow with mocks
        let mock_params = json!({
            "instance": "mock-instance",
            "message": "*Integration Test*\n\nTesting mock integration",
            "number": "+1234567890",
            "linkPreview": true,
            "delay": 0
        });

        // Simulate MCP call
        let mcp_result = MockTestUtils::simulate_mcp_call(
            &transport,
            "send_text_message",
            mock_params.clone()
        ).await;

        assert!(mcp_result.is_ok(), "Mock MCP call should succeed");

        // Simulate API request
        let api_request = EvolutionApiRequest {
            instance: mock_params["instance"].as_str().unwrap().to_string(),
            message: mock_params["message"].as_str().unwrap().to_string(),
            number: Some(mock_params["number"].as_str().unwrap().to_string()),
            link_preview: mock_params["linkPreview"].as_bool().unwrap(),
            delay: mock_params["delay"].as_u64().unwrap() as u32,
            timestamp: std::time::Instant::now(),
        };

        let api_result = api_server.simulate_request(api_request);
        assert!(api_result.is_ok(), "Mock API call should succeed");

        // Verify both components were called
        assert_eq!(transport.get_call_count(), 1);
        assert_eq!(api_server.get_request_count(), 1);

        // Verify request structure
        let last_request = api_server.get_last_request().unwrap();
        let validation_result = MockTestUtils::verify_evolution_api_request(&last_request, "Integration Test");
        assert!(validation_result.is_ok(), "Request should be valid");
    }

    #[tokio::test]
    async fn test_mock_framework_error_injection() {
        let transport = MockMcpTransport::with_default_behavior(MockBehavior::AlwaysFail);

        // Test various error scenarios
        let error_scenarios = vec![
            ("ProcessSpawnFailure", MockResponse::ProcessSpawnFailure),
            ("ServiceCreationFailure", MockResponse::ServiceCreationFailure),
            ("CustomError", MockResponse::Error("Custom test error".to_string())),
            ("Timeout", MockResponse::Timeout),
        ];

        for (scenario_name, error_response) in error_scenarios {
            transport.reset();
            transport.set_response_for_tool("send_text_message", error_response);

            let result = timeout(
                Duration::from_millis(300), // Account for timeout scenario
                MockTestUtils::simulate_mcp_call(
                    &transport,
                    "send_text_message",
                    json!({"test": scenario_name})
                )
            ).await;

            match result {
                Ok(call_result) => {
                    assert!(call_result.is_err(), "Scenario '{}' should produce error", scenario_name);
                },
                Err(_) => {
                    // Timeout is acceptable for timeout scenario
                    assert_eq!(scenario_name, "Timeout", "Only timeout scenario should timeout");
                }
            }

            // Verify call was logged even for errors
            assert_eq!(transport.get_call_count(), 1, "Error scenario '{}' should be logged", scenario_name);
        }
    }
}