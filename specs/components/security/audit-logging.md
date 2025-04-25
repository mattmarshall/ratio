# Audit Logging Component Specification

## Overview
This document outlines the audit logging component for Ratio, which records security-relevant events and operations for accountability, compliance, and forensic analysis. The component provides a comprehensive audit trail of user actions, system events, and security changes.

## Goals
- Record all security-relevant events in the system
- Provide tamper-evident logging for critical operations
- Support compliance requirements for financial applications
- Enable forensic analysis of security incidents
- Facilitate operational troubleshooting and user activity tracking

## Dependencies
- Authentication component for user identity information
- Authorization component for permission context
- PostgreSQL for log storage
- Optional external logging service for secure external log archiving

## Component Design

### Architecture

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│                 │      │                 │      │                 │
│  Audit Logger   │─────►│   Event         │─────►│  Log            │
│  Service        │      │   Formatter     │      │  Dispatcher     │
│                 │      │                 │      │                 │
└─────────────────┘      └─────────────────┘      └─────────────────┘
       ▲                                                  ▲
       │                                                  │
       ▼                                                  ▼
┌─────────────────┐                             ┌─────────────────┐
│                 │                             │                 │
│  Context        │                             │  Storage        │
│  Collector      │                             │  Providers      │
│                 │                             │                 │
└─────────────────┘                             └─────────────────┘
```

### Core Components

#### Audit Logger Service
The main entry point for logging audit events.

```rust
pub struct AuditLoggerService {
    context_collector: Arc<ContextCollector>,
    event_formatter: Arc<EventFormatter>,
    log_dispatcher: Arc<LogDispatcher>,
    config: AuditLoggerConfig,
}

impl AuditLoggerService {
    pub async fn log_event(
        &self, 
        event_type: AuditEventType,
        resource_type: Option<&str>,
        resource_id: Option<i64>,
        action: &str,
        status: AuditEventStatus,
        context: Option<&HashMap<String, Value>>,
        user_id: Option<i64>
    ) -> Result<(), AuditError>;
    
    pub async fn log_authentication_event(
        &self, 
        user_id: Option<i64>,
        action: &str,
        status: AuditEventStatus,
        context: Option<&HashMap<String, Value>>
    ) -> Result<(), AuditError>;
    
    pub async fn log_authorization_event(
        &self, 
        user_id: i64,
        permission: &str,
        resource_type: &str,
        resource_id: Option<i64>,
        status: AuditEventStatus,
        context: Option<&HashMap<String, Value>>
    ) -> Result<(), AuditError>;
    
    pub async fn log_data_access(
        &self, 
        user_id: i64,
        resource_type: &str,
        resource_id: i64,
        action: &str,
        status: AuditEventStatus,
        context: Option<&HashMap<String, Value>>
    ) -> Result<(), AuditError>;
    
    pub async fn log_admin_action(
        &self, 
        admin_id: i64,
        action: &str,
        target_type: Option<&str>,
        target_id: Option<i64>,
        status: AuditEventStatus,
        context: Option<&HashMap<String, Value>>
    ) -> Result<(), AuditError>;
    
    pub async fn get_audit_logs(
        &self,
        filters: AuditLogFilters,
        pagination: Pagination
    ) -> Result<(Vec<AuditLog>, PaginationInfo), AuditError>;
    
    pub async fn export_audit_logs(
        &self,
        filters: AuditLogFilters,
        format: ExportFormat
    ) -> Result<Vec<u8>, AuditError>;
}
```

#### Context Collector
Collects contextual information for audit events.

```rust
pub struct ContextCollector {
    request_context_provider: Option<Arc<dyn RequestContextProvider>>,
    environment_context_provider: Arc<dyn EnvironmentContextProvider>,
}

impl ContextCollector {
    pub async fn collect_context(
        &self, 
        user_id: Option<i64>,
        additional_context: Option<&HashMap<String, Value>>
    ) -> Result<AuditContext, ContextError>;
    
    pub fn collect_request_context(
        &self, 
        request: Option<&HttpRequest>
    ) -> Result<RequestContext, ContextError>;
    
    pub fn collect_environment_context(&self) -> Result<EnvironmentContext, ContextError>;
}
```

#### Event Formatter
Formats audit events into structured log entries.

```rust
pub struct EventFormatter {
    config: FormatterConfig,
}

impl EventFormatter {
    pub fn format_event(
        &self, 
        event_type: AuditEventType,
        resource_type: Option<&str>,
        resource_id: Option<i64>,
        action: &str,
        status: AuditEventStatus,
        user_id: Option<i64>,
        context: &AuditContext
    ) -> Result<AuditLog, FormatterError>;
    
    pub fn mask_sensitive_data(
        &self, 
        data: &Value, 
        field_path: &str
    ) -> Result<Value, FormatterError>;
    
    pub fn generate_event_id(&self) -> String;
}
```

#### Log Dispatcher
Dispatches log entries to various storage providers.

```rust
pub struct LogDispatcher {
    storage_providers: Vec<Arc<dyn StorageProvider>>,
    config: DispatcherConfig,
}

impl LogDispatcher {
    pub async fn dispatch(
        &self, 
        log_entry: &AuditLog
    ) -> Result<(), DispatcherError>;
    
    pub async fn dispatch_batch(
        &self, 
        log_entries: &[AuditLog]
    ) -> Result<(), DispatcherError>;
    
    pub async fn query_logs(
        &self,
        filters: &AuditLogFilters,
        pagination: &Pagination
    ) -> Result<(Vec<AuditLog>, PaginationInfo), DispatcherError>;
}
```

#### Storage Providers
Store audit logs in various destinations.

```rust
pub trait StorageProvider: Send + Sync {
    async fn store(&self, log_entry: &AuditLog) -> Result<(), StorageError>;
    async fn store_batch(&self, log_entries: &[AuditLog]) -> Result<(), StorageError>;
    async fn query(&self, filters: &AuditLogFilters, pagination: &Pagination) 
        -> Result<(Vec<AuditLog>, PaginationInfo), StorageError>;
}

pub struct DatabaseStorageProvider {
    db_pool: Arc<PgPool>,
    table_name: String,
}

pub struct FileStorageProvider {
    log_directory: PathBuf,
    file_rotation_config: FileRotationConfig,
}

pub struct ExternalServiceProvider {
    client: Arc<dyn ExternalLogClient>,
    retry_config: RetryConfig,
}
```

### Data Models

#### Audit Event Types and Status

```rust
pub enum AuditEventType {
    Authentication,
    Authorization,
    DataAccess,
    DataModification,
    SystemConfiguration,
    UserManagement,
    SecurityConfiguration,
    Extension,
}

pub enum AuditEventStatus {
    Success,
    Failure,
    Denied,
    Error,
}
```

#### Audit Log Entry

```rust
pub struct AuditLog {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<i64>,
    pub action: String,
    pub status: AuditEventStatus,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
    pub context: Value,
    pub metadata: Value,
}
```

#### Audit Context

```rust
pub struct AuditContext {
    pub request: Option<RequestContext>,
    pub environment: EnvironmentContext,
    pub custom: Option<HashMap<String, Value>>,
}

pub struct RequestContext {
    pub request_id: String,
    pub ip_address: String,
    pub user_agent: String,
    pub path: String,
    pub method: String,
    pub headers: HashMap<String, String>,
}

pub struct EnvironmentContext {
    pub hostname: String,
    pub process_id: u32,
    pub application_version: String,
    pub environment: String,
}
```

#### Query Filters

```rust
pub struct AuditLogFilters {
    pub event_types: Option<Vec<AuditEventType>>,
    pub user_id: Option<i64>,
    pub resource_type: Option<String>,
    pub resource_id: Option<i64>,
    pub action: Option<String>,
    pub status: Option<AuditEventStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub ip_address: Option<String>,
    pub contains_context: Option<HashMap<String, Value>>,
}

pub struct Pagination {
    pub offset: u64,
    pub limit: u64,
    pub sort_by: String,
    pub sort_direction: SortDirection,
}

pub enum SortDirection {
    Ascending,
    Descending,
}

pub struct PaginationInfo {
    pub total_records: u64,
    pub page_count: u64,
    pub current_page: u64,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}
```

#### Export Format

```rust
pub enum ExportFormat {
    JSON,
    CSV,
    PDF,
}
```

## Database Schema

The following table is required for the audit logging component:

### Audit Logs Table

```sql
CREATE TABLE audit_logs (
    id VARCHAR(64) PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    user_id BIGINT,
    username VARCHAR(255),
    resource_type VARCHAR(50),
    resource_id BIGINT,
    action VARCHAR(100) NOT NULL,
    status VARCHAR(20) NOT NULL,
    ip_address VARCHAR(45),
    user_agent TEXT,
    request_id VARCHAR(64),
    context JSONB NOT NULL DEFAULT '{}',
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_timestamp ON audit_logs(timestamp);
CREATE INDEX idx_audit_logs_event_type ON audit_logs(event_type);
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_status ON audit_logs(status);
CREATE INDEX idx_audit_logs_request_id ON audit_logs(request_id);
CREATE INDEX idx_audit_logs_context ON audit_logs USING GIN (context);
```

## Logging Workflows

### Authentication Event Logging

```rust
// Log a successful authentication
audit_logger.log_authentication_event(
    Some(user_id),
    "login",
    AuditEventStatus::Success,
    Some(context_map)
).await?;

// Log a failed authentication
audit_logger.log_authentication_event(
    None,  // User ID might not be known for failed logins
    "login",
    AuditEventStatus::Failure,
    Some(context_map)
).await?;
```

### Authorization Event Logging

```rust
// Log a permission check
audit_logger.log_authorization_event(
    user_id,
    "read:book",
    "book",
    Some(book_id),
    result ? AuditEventStatus::Success : AuditEventStatus::Denied,
    Some(context_map)
).await?;
```

### Data Access Logging

```rust
// Log data access event
audit_logger.log_data_access(
    user_id,
    "transaction",
    transaction_id,
    "view",
    AuditEventStatus::Success,
    Some(context_map)
).await?;
```

### Data Modification Logging

```rust
// Log data modification
audit_logger.log_event(
    AuditEventType::DataModification,
    Some("account"),
    Some(account_id),
    "update_balance",
    AuditEventStatus::Success,
    Some(context_map),
    Some(user_id)
).await?;
```

### Admin Action Logging

```rust
// Log admin action
audit_logger.log_admin_action(
    admin_id,
    "configure_system_setting",
    Some("security_setting"),
    None,
    AuditEventStatus::Success,
    Some(context_map)
).await?;
```

## Integration with API Layer

The audit logging component can be integrated with the API layer through middleware:

```rust
pub async fn audit_log_middleware<B>(
    req: Request<B>,
    audit_logger: Arc<AuditLoggerService>,
    next: Next<B>
) -> Result<Response, StatusCode> {
    // Extract authentication context
    let auth_ctx = req.extensions()
        .get::<AuthContext>()
        .cloned();
    
    // Collect request context
    let request_context = collect_request_context(&req);
    
    // Determine resource info from request path
    let (resource_type, resource_id) = extract_resource_info(&req);
    
    // Determine action from HTTP method and path
    let action = determine_action(&req);
    
    // Process the request
    let response = next.run(req).await;
    
    // Determine status from response
    let status = match response.status().as_u16() {
        200..=299 => AuditEventStatus::Success,
        401 => AuditEventStatus::Denied,
        403 => AuditEventStatus::Denied,
        _ => AuditEventStatus::Error,
    };
    
    // Log the event
    if let Some(auth_ctx) = auth_ctx {
        audit_logger.log_event(
            AuditEventType::DataAccess,
            resource_type.as_deref(),
            resource_id,
            &action,
            status,
            Some(&request_context),
            Some(auth_ctx.user_id)
        ).await.ok();  // Non-blocking logging
    } else {
        audit_logger.log_event(
            AuditEventType::DataAccess,
            resource_type.as_deref(),
            resource_id,
            &action,
            status,
            Some(&request_context),
            None
        ).await.ok();  // Non-blocking logging
    }
    
    Ok(response)
}
```

## Security Considerations

### Log Integrity

To ensure log integrity:

1. Each log entry has a unique ID
2. Logs are stored with timestamps from a reliable time source
3. Database constraints prevent modification of existing logs
4. Optional cryptographic signatures can be added to log entries
5. Log verification mechanisms can detect tampering

### Sensitive Data Handling

To protect sensitive information:

1. PII and sensitive data are masked or redacted in logs
2. Financial amounts are logged without revealing full details
3. Password and authentication data are never logged
4. Configurable field-level masking rules control data visibility

### Access Control

Access to audit logs is strictly controlled:

1. Only authorized users can query audit logs
2. Audit log access is itself logged as an audit event
3. Row-level security restricts which logs can be viewed
4. Export operations are tracked and restricted

### Log Resilience

To ensure logs are not lost:

1. Asynchronous logging with retry mechanisms
2. Multiple storage providers for redundancy
3. Buffer overflow protection
4. Failover mechanisms for logging components

## Performance Considerations

### Logging Efficiency

To minimize performance impact:

1. Asynchronous logging to avoid blocking operations
2. Batch processing for high-volume events
3. Connection pooling for database operations
4. Optional in-memory buffering with periodic flushing

### Query Performance

For efficient log querying:

1. Indexes on commonly queried fields
2. Partitioning for time-based queries
3. JSONB indexes for context queries
4. Pagination for large result sets

### Storage Management

For efficient storage:

1. Log rotation and archiving policies
2. Compression for archived logs
3. Retention policies based on log types
4. Automatic purging of old logs based on compliance requirements

## Compliance Support

The audit logging component supports various compliance requirements:

1. **GDPR**: Logging of all data access and user consent actions
2. **SOX**: Tracking of financial data changes and approvals
3. **PCI-DSS**: Monitoring of access to payment information
4. **HIPAA**: Logging of access to protected health information (if applicable)

## Configuration Parameters

The audit logging component should be configurable with:

```toml
[audit_logging]
# General settings
enabled = true
log_level = "INFO"
include_context = true

# Storage settings
primary_storage = "database"
secondary_storage = "file"
retention_days = 365

# Database settings
db_table = "audit_logs"
db_batch_size = 100
db_max_pool_size = 5

# File settings
file_path = "/var/log/ratio/audit"
file_rotation = "daily"
file_max_size_mb = 100

# External service settings
external_service_url = "https://logging.example.com/api/logs"
external_service_token = "${ENV_LOG_TOKEN}"  # Environment variable
external_service_retry_count = 3

# Masking settings
mask_patterns = [
    "creditCard:.*?(\\d{4}-\\d{4}-\\d{4}-)(\\d{4}).*?:$1XXXX",
    "ssn:.*?(\\d{3}-\\d{2}-)(\\d{4}).*?:$1XXXX",
    "password:.*:REDACTED"
]

# Query settings
max_query_days = 90
max_results_per_page = 1000
```

## Testing Strategy

The audit logging component should be tested with:

1. **Unit Tests**:
   - Test log formatting and filtering
   - Test context collection
   - Test log dispatching

2. **Integration Tests**:
   - Test database storage and retrieval
   - Test file storage and rotation
   - Test integration with authentication and authorization

3. **Performance Tests**:
   - Measure logging overhead
   - Test high-volume logging scenarios
   - Test query performance with large log volumes

4. **Security Tests**:
   - Verify log integrity mechanisms
   - Test access control to logs
   - Verify sensitive data masking

## Monitoring and Metrics

The following metrics should be tracked:

1. **Logging Metrics**:
   - Events logged per minute
   - Logging latency
   - Logging errors
   - Storage consumption rate

2. **Query Metrics**:
   - Query response time
   - Query volume
   - Export operations

3. **Security Metrics**:
   - Log access attempts
   - Suspicious patterns in logs
   - Failed operations rate
