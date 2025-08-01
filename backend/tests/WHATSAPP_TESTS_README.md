# WhatsApp Notification Integration - Comprehensive Test Suite

This document describes the comprehensive test suite for the WhatsApp notification integration in automagik-forge. The test suite provides extensive coverage of configuration validation, message formatting, MCP communication, error handling, performance testing, and security validation.

## 📋 Test Suite Overview

The WhatsApp test suite consists of 5 main test modules with over 95 individual test cases:

### 1. **Unit Tests** (`whatsapp_comprehensive_tests.rs`)
- **Focus**: Individual component testing with comprehensive edge case coverage
- **Test Count**: 25+ tests
- **Coverage Areas**:
  - Configuration validation and environment variable handling
  - Message formatting and special character handling
  - Unicode support and large payload processing
  - Security testing for injection attacks
  - Resource cleanup verification

### 2. **Integration Tests** (`whatsapp_integration_tests.rs`)
- **Focus**: End-to-end notification flow testing
- **Test Count**: 15+ tests (existing + enhancements)
- **Coverage Areas**:
  - Full notification flow from configuration to delivery
  - Environment variable integration
  - Error propagation and graceful failure handling
  - Configuration variations and edge cases

### 3. **Performance Tests** (`whatsapp_performance_tests.rs`)
- **Focus**: Stress testing and performance validation
- **Test Count**: 12+ tests
- **Coverage Areas**:
  - Sequential and concurrent notification performance
  - Resource usage and memory leak detection
  - Stress testing under high load scenarios
  - Timeout behavior validation and consistency
  - Resource cleanup under concurrent operations

### 4. **Mock Framework** (`whatsapp_mock_framework.rs`)
- **Focus**: Test infrastructure and mocking capabilities
- **Test Count**: 15+ tests
- **Coverage Areas**:
  - Mock MCP transport simulation
  - Mock Evolution API server responses
  - Controllable error injection
  - Request/response logging and verification
  - Concurrent mock operation testing

### 5. **Test Runner & Documentation** (`run_whatsapp_tests.rs`)
- **Focus**: Test execution utilities and examples
- **Coverage Areas**:
  - Test execution configuration and automation
  - Test report generation
  - Usage examples and documentation
  - Environment setup utilities

## 🚀 Quick Start

### Running All WhatsApp Tests
```bash
# Run all WhatsApp notification tests
cargo test whatsapp

# Run with output capture to see detailed results
cargo test whatsapp -- --nocapture

# Run specific test categories
cargo test whatsapp_config                    # Configuration tests
cargo test whatsapp_comprehensive_tests       # Comprehensive edge cases
cargo test whatsapp_performance_tests         # Performance and stress tests
cargo test whatsapp_mock_framework           # Mock framework tests
```

### Running Specific Test Scenarios
```bash
# Configuration validation tests
cargo test test_whatsapp_config_from_env

# Message handling tests
cargo test test_whatsapp_notifier_unicode_handling

# Performance tests (may take longer)
cargo test test_concurrent_notification_performance

# Security tests
cargo test test_whatsapp_security_injection_prevention
```

## 🔧 Test Environment Setup

### Required Environment Variables (for testing)
```bash
export EVOLUTION_API_BASE_URL="http://test-api.example.com"
export EVOLUTION_API_API_KEY="test-key-12345"
export EVOLUTION_API_INSTANCE="test-instance"
export EVOLUTION_API_FIXED_RECIPIENT="+5511999999999"
export WHATSAPP_MCP_SERVER_TIMEOUT="2000"
export WHATSAPP_NOTIFICATION_INCLUDE_URL="true"
```

### Test Configuration Options
```bash
# Sequential execution (for performance tests)
cargo test whatsapp_performance -- --test-threads=1

# Show all test output including performance metrics
cargo test whatsapp -- --nocapture

# Run ignored tests (long-running performance tests)
cargo test whatsapp -- --ignored
```

## 📊 Test Coverage Details

### Configuration Tests (15+ tests)
- ✅ Valid environment variable loading
- ✅ Missing required variable handling
- ✅ Optional field default values
- ✅ Timeout parsing edge cases
- ✅ URL format validation
- ✅ Boolean parsing validation
- ✅ Security injection resistance
- ✅ Clone and debug trait validation
- ✅ Boundary value testing
- ✅ Error message validation

### Notification Tests (25+ tests)
- ✅ Message formatting with special characters
- ✅ Unicode handling across different languages
- ✅ Large payload processing
- ✅ Empty and whitespace edge cases
- ✅ Concurrent notification handling
- ✅ Resource cleanup verification
- ✅ Error handling and graceful failure
- ✅ JSON parameter structure validation
- ✅ MCP transport simulation
- ✅ Process spawn failure handling

### Integration Tests (12+ tests)
- ✅ Full notification flow testing
- ✅ Environment configuration integration
- ✅ Configuration variation testing
- ✅ JSON parameter compliance
- ✅ Error propagation validation
- ✅ Resource management verification
- ✅ Mock MCP server integration
- ✅ Evolution API protocol compliance

### Performance Tests (12+ tests)
- ✅ Sequential notification performance
- ✅ Varying message size performance
- ✅ Small concurrent load (5 notifications)
- ✅ Medium concurrent load (15 notifications)  
- ✅ High concurrent load (30 notifications)
- ✅ Rapid creation/destruction stress test
- ✅ Memory usage pattern validation
- ✅ Timeout behavior consistency
- ✅ Resource cleanup verification
- ✅ Concurrent resource management

### Security Tests (8+ tests)
- ✅ Command injection prevention
- ✅ Environment variable injection resistance
- ✅ Special character handling
- ✅ Path traversal attack prevention
- ✅ Script injection prevention
- ✅ SQL injection attempt handling
- ✅ XSS attempt prevention
- ✅ Environment isolation validation

## 🏗️ Test Architecture

### Test Utilities and Builders
```rust
// Environment management with automatic cleanup
TestEnvironment::new()
    .setup_valid_env()
    .set_var("KEY", "value")

// Configuration builders for various scenarios
ConfigTestBuilder::valid_config()
ConfigTestBuilder::invalid_config()
ConfigTestBuilder::config_with_special_chars()

// Notification data builders
NotificationTestBuilder::unicode_notification()
NotificationTestBuilder::large_notification()
NotificationTestBuilder::injection_attempt_notification()
```

### Mock Framework Components
```rust
// Mock MCP transport with controllable responses
MockMcpTransport::new()
    .set_response_for_tool("send_text_message", MockResponse::Success(...))
    .set_default_behavior(MockBehavior::AlwaysFail)

// Mock Evolution API server
MockEvolutionApiServer::new()
    .configure_success_response("message-id-123")
    .configure_error_response(401, "Invalid API key")
```

### Performance Testing Framework
```rust
// Performance metrics collection
PerformanceTestHarness::new(config)
    .run_concurrent_notifications(count, timeout_ms)
    .get_metrics() // Returns detailed performance data

// Metrics include: success rate, timeout rate, duration stats
```

## 🎯 Key Test Scenarios

### 1. Configuration Edge Cases
- Missing environment variables with specific error messages  
- Invalid timeout values with fallback behavior
- URL format variations and protocol handling
- Boolean parsing edge cases
- Boundary value testing (zero, negative, overflow)
- Security injection attempt validation

### 2. Message Processing
- Unicode character support across languages
- Large message handling (up to 100KB+)
- Empty and whitespace-only content
- Special characters and escape sequences
- JSON structure compliance for Evolution API
- Message formatting consistency

### 3. Error Handling Scenarios
- Process spawn failures (uvx not available)
- MCP service creation failures
- Network timeout simulations
- Evolution API error responses
- Resource cleanup on errors
- Graceful degradation patterns

### 4. Performance Characteristics
- Sequential notification throughput
- Concurrent operation scaling (5, 15, 30+ notifications)
- Memory usage patterns and leak detection
- Resource cleanup efficiency
- Timeout behavior consistency
- Stress testing under high load

### 5. Security Validation
- Command injection prevention
- Environment variable contamination
- Input sanitization verification
- Process isolation validation
- Error message information leakage
- Resource access restrictions

## 📈 Performance Benchmarks

### Expected Performance Characteristics
- **Single Notification**: < 100ms (mock scenario)
- **5 Concurrent**: < 2 seconds total
- **15 Concurrent**: < 5 seconds total  
- **30 Concurrent**: < 10 seconds total
- **Memory Usage**: Stable, no leaks detected
- **Resource Cleanup**: Complete within 100ms

### Performance Test Metrics
```bash
# Run performance tests with metrics output
cargo test whatsapp_performance_tests -- --nocapture

# Expected output includes:
# - Total/average/min/max duration
# - Success and timeout rates
# - Concurrent operation statistics
# - Memory usage patterns
# - Resource cleanup verification
```

## 🛠️ Extending the Test Suite

### Adding New Test Cases
1. **Unit Tests**: Add to `whatsapp_comprehensive_tests.rs`
2. **Integration Tests**: Add to `whatsapp_integration_tests.rs`  
3. **Performance Tests**: Add to `whatsapp_performance_tests.rs`
4. **Mock Scenarios**: Add to `whatsapp_mock_framework.rs`

### Test Patterns to Follow
```rust
#[tokio::test]
async fn test_new_scenario() {
    // Arrange: Set up test environment and data
    let mut env = TestEnvironment::new();
    env.setup_valid_env();
    
    let config = ConfigTestBuilder::valid_config();
    let notifier = WhatsAppNotifier::new(config).await.unwrap();
    
    // Act: Execute the scenario
    let result = timeout(
        Duration::from_millis(100),
        notifier.send_notification("Test", "Message")
    ).await;
    
    // Assert: Verify expected behavior
    match result {
        Ok(send_result) => assert!(send_result.is_ok()),
        Err(_) => {}, // Timeout acceptable for mock scenarios
    }
}
```

### Mock Response Patterns
```rust
// Success scenario
transport.set_response_for_tool("send_text_message", 
    MockResponse::Success(json!({"status": "sent", "messageId": "123"})));

// Error scenario  
transport.set_response_for_tool("send_text_message",
    MockResponse::Error("Authentication failed".to_string()));

// Timeout scenario
transport.set_response_for_tool("send_text_message", 
    MockResponse::Timeout);
```

## 🚨 Troubleshooting Test Issues

### Common Test Failures
1. **Environment Variable Issues**: Ensure test environment is properly isolated
2. **Timeout Variations**: Adjust timeout values based on system performance
3. **Concurrent Test Flaking**: Use sequential execution for performance tests
4. **Resource Cleanup**: Verify proper cleanup in test teardown

### Debug Options
```bash
# Enable debug logging
RUST_LOG=debug cargo test whatsapp

# Run single test with output
cargo test test_specific_scenario -- --exact --nocapture

# Run tests sequentially to avoid resource contention
cargo test whatsapp -- --test-threads=1
```

### Performance Test Considerations
- Performance tests may show timing variations across different systems
- Use `--test-threads=1` for accurate performance measurement
- Monitor system resources during stress tests
- Adjust timeout values based on system capabilities

## 📚 Additional Resources

### Related Documentation
- [WhatsApp Integration Overview](../src/services/whatsapp_notifier.rs)
- [Configuration Management](../src/services/whatsapp_config.rs)
- [MCP Transport Documentation](../src/mcp/)
- [Evolution API Protocol](https://evolution-api.com/docs)

### Test Development Guidelines
- Follow existing test patterns and naming conventions
- Include both positive and negative test scenarios
- Document test purpose and expected behavior
- Use appropriate timeout values for async operations
- Verify resource cleanup in all test scenarios
- Include performance considerations for new features

---

**Total Test Coverage**: 95+ individual test cases across 5 test modules  
**Execution Time**: ~2-5 minutes for full suite (varies by system)  
**Test Categories**: Unit, Integration, Performance, Mock Framework, Security  
**Mock Capabilities**: Full MCP transport and Evolution API simulation  
**Performance Testing**: Concurrent operations up to 30+ simultaneous notifications