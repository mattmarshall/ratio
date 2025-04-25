# Authentication Component Specification

## Overview
This document outlines the authentication component for Ratio, which provides user identity verification and session management. The component handles credential validation, token management, and multi-factor authentication.

## Goals
- Provide secure user authentication
- Manage JWT token lifecycle
- Support multi-factor authentication
- Ensure secure credential storage
- Maintain session state

## Dependencies
- Argon2id library for password hashing
- JWT library for token generation and validation
- TOTP library for multi-factor authentication
- PostgreSQL for user and token storage

## Component Design

### Architecture

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│                 │      │                 │      │                 │
│  Authentication │─────►│   Token         │─────►│  Session        │
│  Service        │      │   Manager       │      │  Manager        │
│                 │      │                 │      │                 │
└─────────────────┘      └─────────────────┘      └─────────────────┘
       ▲                        ▲                        ▲
       │                        │                        │
       ▼                        ▼                        ▼
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│                 │      │                 │      │                 │
│  Password       │      │  MFA            │      │  User           │
│  Manager        │      │  Provider       │      │  Repository     │
│                 │      │                 │      │                 │
└─────────────────┘      └─────────────────┘      └─────────────────┘
```

### Core Components

#### Authentication Service
The main entry point for authentication operations.

```rust
pub struct AuthenticationService {
    user_repository: Arc<dyn UserRepository>,
    password_manager: PasswordManager,
    token_manager: TokenManager,
    mfa_provider: Option<MfaProvider>,
    session_manager: SessionManager,
}

impl AuthenticationService {
    pub async fn authenticate(
        &self, 
        username: &str, 
        password: &str
    ) -> Result<AuthenticationResult, AuthError>;
    
    pub async fn verify_mfa(
        &self, 
        user_id: i64, 
        token: &str
    ) -> Result<AuthenticationResult, AuthError>;
    
    pub async fn refresh_token(
        &self, 
        refresh_token: &str
    ) -> Result<TokenPair, AuthError>;
    
    pub async fn logout(
        &self, 
        refresh_token: &str
    ) -> Result<(), AuthError>;
    
    pub async fn validate_token(
        &self, 
        token: &str
    ) -> Result<TokenValidationResult, AuthError>;
}
```

#### Password Manager
Handles secure password operations.

```rust
pub struct PasswordManager {
    pepper: String,
    hash_memory_cost: u32,
    hash_time_cost: u32,
    hash_parallelism: u32,
}

impl PasswordManager {
    pub fn hash_password(&self, password: &str) -> Result<String, PasswordError>;
    pub fn verify_password(&self, hash: &str, password: &str) -> Result<bool, PasswordError>;
    pub fn needs_rehash(&self, hash: &str) -> bool;
}
```

#### Token Manager
Manages JWT token lifecycle.

```rust
pub struct TokenManager {
    private_key: Arc<RsaPrivateKey>,
    public_key: Arc<RsaPublicKey>,
    token_repository: Arc<dyn TokenRepository>,
    access_token_expiry: Duration,
    refresh_token_expiry: Duration,
}

impl TokenManager {
    pub async fn generate_token_pair(
        &self, 
        user_id: i64,
        roles: Vec<String>,
        permissions: Vec<String>
    ) -> Result<TokenPair, TokenError>;
    
    pub async fn validate_access_token(
        &self, 
        token: &str
    ) -> Result<TokenValidationResult, TokenError>;
    
    pub async fn refresh_tokens(
        &self, 
        refresh_token: &str
    ) -> Result<TokenPair, TokenError>;
    
    pub async fn revoke_token(
        &self, 
        token_id: &str
    ) -> Result<(), TokenError>;
    
    pub async fn revoke_all_user_tokens(
        &self, 
        user_id: i64
    ) -> Result<(), TokenError>;
}
```

#### MFA Provider
Handles multi-factor authentication options.

```rust
pub enum MfaMethod {
    Totp,
    RecoveryCode,
}

pub struct MfaProvider {
    totp_secret_key: String,
    totp_digits: u32,
    totp_period: u32,
    recovery_code_count: u32,
    recovery_code_length: u32,
}

impl MfaProvider {
    pub fn generate_totp_secret() -> String;
    pub fn generate_recovery_codes() -> Vec<String>;
    pub fn verify_totp(&self, secret: &str, token: &str) -> bool;
    pub async fn verify_recovery_code(
        &self, 
        user_id: i64, 
        code: &str
    ) -> Result<bool, MfaError>;
    pub fn generate_totp_uri(&self, secret: &str, username: &str) -> String;
}
```

#### Session Manager
Manages active sessions and authentication state.

```rust
pub struct SessionManager {
    token_manager: Arc<TokenManager>,
    session_repository: Arc<dyn SessionRepository>,
    max_sessions_per_user: u32,
}

impl SessionManager {
    pub async fn create_session(
        &self, 
        user_id: i64, 
        token_id: &str, 
        device_info: &DeviceInfo
    ) -> Result<Session, SessionError>;
    
    pub async fn validate_session(
        &self, 
        token_id: &str
    ) -> Result<Session, SessionError>;
    
    pub async fn end_session(
        &self, 
        session_id: i64
    ) -> Result<(), SessionError>;
    
    pub async fn end_all_user_sessions(
        &self, 
        user_id: i64
    ) -> Result<(), SessionError>;
    
    pub async fn list_active_sessions(
        &self, 
        user_id: i64
    ) -> Result<Vec<Session>, SessionError>;
}
```

### Data Models

#### User Authentication Data

```rust
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub is_active: bool,
    pub failed_login_attempts: u32,
    pub locked_until: Option<DateTime<Utc>>,
    pub password_changed_at: DateTime<Utc>,
    pub mfa_enabled: bool,
    pub mfa_secret: Option<String>,
    pub recovery_codes: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

#### Token Data

```rust
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

pub struct TokenValidationResult {
    pub user_id: i64,
    pub token_id: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub struct RefreshToken {
    pub id: String,
    pub user_id: i64,
    pub token_family: String,
    pub is_used: bool,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
```

#### Session Data

```rust
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub token_id: String,
    pub ip_address: String,
    pub user_agent: String,
    pub device_type: String,
    pub location: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

pub struct DeviceInfo {
    pub ip_address: String,
    pub user_agent: String,
    pub device_type: String,
    pub location: Option<String>,
}
```

## Database Schema

The following tables are required for the authentication component:

### Users Table

```sql
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    failed_login_attempts INT NOT NULL DEFAULT 0,
    locked_until TIMESTAMP WITH TIME ZONE,
    password_changed_at TIMESTAMP WITH TIME ZONE NOT NULL,
    mfa_enabled BOOLEAN NOT NULL DEFAULT false,
    mfa_secret VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_is_active ON users(is_active);
```

### Recovery Codes Table

```sql
CREATE TABLE recovery_codes (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    code_hash VARCHAR(255) NOT NULL,
    is_used BOOLEAN NOT NULL DEFAULT false,
    used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_recovery_codes_user_id ON recovery_codes(user_id);
```

### Refresh Tokens Table

```sql
CREATE TABLE refresh_tokens (
    id VARCHAR(64) PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    token_family VARCHAR(64) NOT NULL,
    is_used BOOLEAN NOT NULL DEFAULT false,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_token_family ON refresh_tokens(token_family);
CREATE INDEX idx_refresh_tokens_expires_at ON refresh_tokens(expires_at);
```

### Sessions Table

```sql
CREATE TABLE sessions (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    token_id VARCHAR(64) NOT NULL,
    ip_address VARCHAR(45) NOT NULL,
    user_agent TEXT NOT NULL,
    device_type VARCHAR(50) NOT NULL,
    location VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_token_id ON sessions(token_id);
CREATE INDEX idx_sessions_last_active_at ON sessions(last_active_at);
```

## Authentication Flows

### Registration Flow

1. User provides username, email, and password
2. System validates input (username/email uniqueness, password strength)
3. Password is hashed using Argon2id with salt and pepper
4. User record is created with default role
5. Welcome email is sent with confirmation link
6. User activates account via confirmation link (optional)

### Login Flow

1. User provides username/email and password
2. System verifies credentials against stored hash
3. If MFA is enabled, prompt for verification code
4. Upon successful verification:
   - Generate JWT access and refresh tokens
   - Create new session record
   - Return tokens to client
5. If authentication fails:
   - Increment failed login attempts
   - Lock account after threshold (e.g., 5 attempts)
   - Log authentication failure

### Token Refresh Flow

1. Client sends refresh token
2. System validates refresh token:
   - Verify signature
   - Check expiration
   - Ensure token is not used or revoked
3. If valid:
   - Generate new access and refresh tokens
   - Mark old refresh token as used
   - Return new token pair
4. If invalid:
   - Revoke all tokens in the family (potential token theft)
   - Force re-authentication

### Password Reset Flow

1. User requests password reset via email
2. System generates time-limited reset token
3. Reset link is sent to user's email
4. User submits new password with reset token
5. System validates token and updates password
6. All existing sessions and refresh tokens are revoked
7. User is notified of password change

### MFA Setup Flow

1. User initiates MFA setup from account settings
2. System generates TOTP secret and recovery codes
3. QR code is displayed for TOTP app scanning
4. User verifies by entering a valid TOTP code
5. MFA is enabled on the account
6. User is shown recovery codes to save

## Security Considerations

### Password Storage

- Argon2id hashing algorithm with individual salts
- Application-level pepper added before hashing
- Configurable memory and time cost parameters
- Regular reassessment of hashing parameters

### Token Security

- RS256 asymmetric signing for JWTs
- Short-lived access tokens (15 minutes)
- Refresh tokens with one-time use policy
- Token rotation on refresh for token theft detection
- Secure token storage in HTTP-only, secure cookies or secure device storage

### Rate Limiting

- Limit authentication attempts by IP and username/email
- Exponential backoff for repeated failures
- Account lockout after threshold with notification
- Delayed responses for failed attempts to prevent timing attacks

### Audit and Monitoring

- Log all authentication events (success, failure, lockout)
- Monitor for suspicious patterns
- Alert on unusual activity (e.g., multiple lockouts, geographic anomalies)
- Regular security review of authentication logs

## Implementation Guidelines

### Authentication Service Implementation

```rust
// Example implementation skeleton
impl AuthenticationService {
    pub async fn authenticate(
        &self, 
        username: &str, 
        password: &str
    ) -> Result<AuthenticationResult, AuthError> {
        // Find user by username
        let user = self.user_repository.find_by_username(username).await?;
        
        // Check if account is locked
        if let Some(locked_until) = user.locked_until {
            if locked_until > Utc::now() {
                return Err(AuthError::AccountLocked);
            }
        }
        
        // Verify password
        if !self.password_manager.verify_password(&user.password_hash, password)? {
            // Handle failed login
            self.handle_failed_login(&user).await?;
            return Err(AuthError::InvalidCredentials);
        }
        
        // Reset failed login attempts
        if user.failed_login_attempts > 0 {
            self.user_repository.reset_failed_attempts(user.id).await?;
        }
        
        // Check if password needs rehashing
        if self.password_manager.needs_rehash(&user.password_hash) {
            let new_hash = self.password_manager.hash_password(password)?;
            self.user_repository.update_password_hash(user.id, &new_hash).await?;
        }
        
        // Check if MFA is required
        if user.mfa_enabled {
            return Ok(AuthenticationResult::MfaRequired { user_id: user.id });
        }
        
        // Generate tokens
        let token_pair = self.token_manager.generate_token_pair(
            user.id,
            self.get_user_roles(user.id).await?,
            self.get_user_permissions(user.id).await?
        ).await?;
        
        // Create session
        // ... session creation code ...
        
        Ok(AuthenticationResult::Success { 
            user_id: user.id,
            token_pair,
        })
    }
    
    // Other method implementations...
}
```

### Token Validation Middleware

```rust
pub async fn auth_middleware<B>(
    req: Request<B>,
    token_manager: Arc<TokenManager>,
    next: Next<B>
) -> Result<Response, StatusCode> {
    // Extract token from header
    let auth_header = req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|value| {
            if value.starts_with("Bearer ") {
                Some(value[7..].to_string())
            } else {
                None
            }
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Validate token
    let validation_result = token_manager
        .validate_access_token(&auth_header)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    // Set authentication context in request extensions
    let mut req = req;
    req.extensions_mut().insert(AuthContext {
        user_id: validation_result.user_id,
        roles: validation_result.roles,
        permissions: validation_result.permissions,
    });
    
    // Continue to the next middleware/handler
    Ok(next.run(req).await)
}
```

## Testing Strategy

The authentication component should be thoroughly tested with:

1. **Unit Tests**:
   - Test each component in isolation with mocked dependencies
   - Validate all authentication flows
   - Test edge cases and error conditions

2. **Integration Tests**:
   - Test authentication API endpoints
   - Validate token generation and validation
   - Test authentication middleware

3. **Security Tests**:
   - Test rate limiting and account lockout
   - Test token expiration and refresh
   - Test against common attacks (brute force, timing attacks)

4. **Performance Tests**:
   - Benchmark password hashing parameters
   - Test performance under load

5. **Compliance Tests**:
   - Validate against security requirements
   - Test data protection and privacy features

## Monitoring and Metrics

The following metrics should be tracked:

1. **Authentication Metrics**:
   - Authentication success/failure rate
   - Number of locked accounts
   - MFA usage statistics
   - Password reset frequency

2. **Performance Metrics**:
   - Authentication response time
   - Token validation time
   - Password hash time

3. **Security Metrics**:
   - Failed login attempts
   - Token refresh rate
   - Session duration distribution
   - Geographic distribution of logins

## Configuration Parameters

The authentication component should be configurable with:

```toml
[authentication]
# Password hashing
argon2_memory_cost = 65536
argon2_time_cost = 3
argon2_parallelism = 4
pepper = "${ENV_PEPPER}"  # Environment variable

# Token settings
access_token_expiry_minutes = 15
refresh_token_expiry_days = 7
token_signing_algorithm = "RS256"
token_private_key_path = "/path/to/private.key"
token_public_key_path = "/path/to/public.key"

# MFA settings
totp_digits = 6
totp_period = 30
recovery_code_count = 10
recovery_code_length = 8

# Security settings
max_failed_attempts = 5
account_lockout_minutes = 30
max_sessions_per_user = 5
