# Security Architecture Specification

## Overview
This document outlines the security architecture for Ratio, addressing authentication, authorization, data protection, and audit logging. The security model is designed to protect sensitive financial data while providing flexible access control for multi-user scenarios.

## Security Principles

- **Defense in Depth**: Multiple security controls at different layers
- **Least Privilege**: Users have only the permissions they need
- **Secure by Default**: All resources require explicit permissions
- **Fail Secure**: Security failures result in denied access, not granted access
- **Auditability**: All security-relevant actions are logged

## Authentication Framework

### Authentication Methods

1. **Local Authentication**
   - Username/password with strong password policy
   - Password storage using Argon2id with proper salt and work factors
   - Support for MFA (TOTP) as a second factor

2. **Token-Based Authentication**
   - JWT tokens for API access
   - Short-lived access tokens (15 minutes)
   - Longer-lived refresh tokens (7 days) with secure storage
   - Token rotation on refresh with one-time use

### Token Structure

```
{
  "header": {
    "alg": "RS256",
    "typ": "JWT"
  },
  "payload": {
    "sub": "<user_id>",
    "iss": "ratio",
    "iat": <timestamp>,
    "exp": <timestamp>,
    "jti": "<unique_token_id>",
    "roles": ["role1", "role2"],
    "permissions": ["permission1", "permission2"]
  }
}
```

### Authentication Flow

1. User presents credentials
2. System validates credentials against stored hash
3. If valid, generate and return access token and refresh token
4. Client includes access token in subsequent requests
5. When access token expires, client uses refresh token to get new tokens

## Authorization Framework

### Multi-layered Authorization

Ratio implements authorization at three layers:

1. **API Layer**
   - Validates token permissions before processing requests
   - Enforces coarse-grained access control based on roles and permissions
   - Resolves polymorphic permissions (e.g., "owner" resolves to specific permissions)

2. **Service Layer**
   - Enforces business rules and complex permission logic
   - Validates cross-entity relationships and hierarchies
   - Manages derived permissions

3. **Database Layer**
   - PostgreSQL Row-Level Security (RLS) policies
   - Enforces fine-grained access control at the row level
   - Provides defense in depth for direct database access scenarios

### Role-Based Access Control

Predefined roles with associated permissions:

- **Admin**: Full system access
- **Manager**: Full access to assigned books and their children
- **User**: Standard access to owned and shared books
- **Viewer**: Read-only access to shared books
- **Service**: Limited API access for integrations

### Permission Model

Permissions follow the format: `<action>:<resource_type>[:resource_id]`

Examples:
- `read:book`: Can read any book
- `create:account:*`: Can create accounts in any book
- `update:transaction:123`: Can update transaction with ID 123

### Object Ownership

- Each object has an owner (user who created it)
- Ownership can be transferred
- Ownership grants implicit admin permissions on the object

### Access Control Lists (ACLs)

- Books, accounts, and other objects can have ACLs
- ACLs contain user-to-permission mappings for fine-grained control
- Permissions are inherited down the hierarchy (book → accounts → transactions)

## Row-Level Security Implementation

### Security Context

The PostgreSQL security context is set at the connection level:

```sql
-- Set current user context
SELECT set_config('app.current_user_id', '<user_id>', true);
-- Set current roles context
SELECT set_config('app.current_user_roles', '["role1","role2"]', true);
```

### Connection Management

1. **Context Lifecycle**
   - The API layer sets security context on the connection at the start of each request
   - Security context is cleared after the request completes
   - Connection pooling maintains security isolation between requests

2. **Connection Middleware**
   ```rust
   async fn set_security_context(
       conn: &mut PgConnection, 
       user_id: i64, 
       roles: &[String],
       permissions: &[String]
   ) -> Result<(), Error> {
       // Set user context
       conn.execute(
           "SELECT set_config('app.current_user_id', $1, true)", 
           &[&user_id.to_string()]
       ).await?;
       
       // Set roles and permissions
       conn.execute(
           "SELECT set_config('app.current_user_roles', $1, true)", 
           &[&serde_json::to_string(roles)?]
       ).await?;
       
       conn.execute(
           "SELECT set_config('app.current_user_permissions', $1, true)", 
           &[&serde_json::to_string(permissions)?]
       ).await?;
       
       Ok(())
   }
   ```

### Table-Level RLS Policies

Each table with sensitive data has RLS policies:

```sql
-- Enable RLS on books table
ALTER TABLE books ENABLE ROW LEVEL SECURITY;

-- Policy for book access (owner or explicitly granted access)
CREATE POLICY book_access_policy ON books
    FOR SELECT
    USING (
        created_by_user_id = current_setting('app.current_user_id')::bigint
        OR
        id IN (
            SELECT book_id 
            FROM book_access_grants 
            WHERE user_id = current_setting('app.current_user_id')::bigint
        )
        OR
        EXISTS (
            SELECT 1 
            FROM users_roles ur
            JOIN roles_permissions rp ON ur.role_id = rp.role_id
            WHERE ur.user_id = current_setting('app.current_user_id')::bigint
            AND rp.permission = 'read:book:*'
        )
    );

-- Policy for book modification (owner or users with explicit modify permission)
CREATE POLICY book_modify_policy ON books
    FOR UPDATE
    USING (
        created_by_user_id = current_setting('app.current_user_id')::bigint
        OR
        id IN (
            SELECT book_id 
            FROM book_access_grants 
            WHERE user_id = current_setting('app.current_user_id')::bigint
            AND permission = 'update:book'
        )
        OR
        EXISTS (
            SELECT 1 
            FROM users_roles ur
            JOIN roles_permissions rp ON ur.role_id = rp.role_id
            WHERE ur.user_id = current_setting('app.current_user_id')::bigint
            AND rp.permission = 'update:book:*'
        )
    );
```

### Administrative Override

For administrative functions:

```sql
-- Create bypass mechanism for administrative users
CREATE POLICY admin_bypass_policy ON books
    USING (
        EXISTS (
            SELECT 1 
            FROM users_roles ur
            WHERE ur.user_id = current_setting('app.current_user_id')::bigint
            AND ur.role_id = (SELECT id FROM roles WHERE name = 'ADMIN')
        )
    );
```

### Performance Considerations

- Indexes on foreign key columns used in RLS policies
- Periodic evaluation of RLS policy efficiency
- Selective use of security definer functions for critical operations
- Caching commonly used permission checks

## Data Protection

### Encryption at Rest

- Sensitive personal data encrypted at the column level
- AES-256-GCM encryption for sensitive fields
- Database backups encrypted using application-level encryption

### Encryption in Transit

- TLS 1.3+ for all API connections
- TLS for database connections
- Internal service communications encrypted

### Key Management

- Application master key securely stored in environment or key vault
- Per-user encryption keys for personal data
- Key rotation procedures for all keys

## Audit Logging

### Audit Events

All security-relevant events are logged:

- Authentication attempts (success/failure)
- Authorization decisions (access granted/denied)
- Permission changes
- Security configuration changes
- Object access and modifications

### Log Format

```json
{
  "timestamp": "ISO8601",
  "event_type": "PERMISSION_CHANGE",
  "user_id": "123",
  "resource_type": "book",
  "resource_id": "456",
  "action": "grant",
  "target_user_id": "789",
  "permission": "read:book",
  "status": "success",
  "context": {
    "source_ip": "192.168.1.1",
    "user_agent": "..."
  }
}
```

### Secure Logging

- Logs are integrity-protected
- Logs are retained according to compliance requirements
- Access to logs is restricted and audited
- External log shipping for critical events

## Security Boundaries and Trust Zones

1. **Public Zone**
   - Internet-facing API endpoints
   - Authentication enforced
   - Rate limiting and DDoS protection

2. **Application Zone**
   - API services and business logic
   - Service-to-service authentication
   - Authorization enforced

3. **Data Zone**
   - Database and data storage
   - Row-level security enforced
   - Encryption for sensitive data

## Threat Mitigation

| Threat | Mitigation |
|--------|------------|
| Unauthorized access | Authentication, authorization, RLS |
| Data leakage | Encryption, access controls, RLS |
| Password compromise | Argon2id, MFA, password policies |
| SQL injection | Parameterized queries, RLS |
| Privilege escalation | Least privilege, RLS, role separation |
| Session hijacking | Short-lived JWTs, secure cookie policies |
| API abuse | Rate limiting, input validation |

## Security Monitoring

- Failed authentication attempts monitoring
- Anomalous access pattern detection
- Permission change monitoring
- Privilege escalation attempts detection
- Regular security scanning and testing

## Compliance Considerations

- GDPR: Data subject rights, encryption, audit logs
- CCPA: User data access and deletion capabilities
- SOC 2: Control framework alignment
- Financial data protection standards
