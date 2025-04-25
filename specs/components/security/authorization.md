# Authorization Component Specification

## Overview
This document outlines the authorization component for Ratio, which manages permissions and access control throughout the application. The component enforces a multi-layered authorization approach with role-based access control (RBAC), object-level permissions, and row-level security (RLS) in PostgreSQL.

## Goals
- Implement a flexible and granular permissions system
- Support role-based access control with hierarchical roles
- Enable object-level access control for fine-grained permissions
- Enforce permissions at multiple layers (API, service, database)
- Provide efficient permission checking with caching
- Support delegation and permission transfer

## Dependencies
- Authentication component for user identity
- Database with row-level security support
- Cache system for permission resolution optimization

## Component Design

### Architecture

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│                 │      │                 │      │                 │
│  Authorization  │─────►│   Policy        │─────►│  Role           │
│  Service        │      │   Enforcer      │      │  Manager        │
│                 │      │                 │      │                 │
└─────────────────┘      └─────────────────┘      └─────────────────┘
       ▲                        ▲                        ▲
       │                        │                        │
       ▼                        ▼                        ▼
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│                 │      │                 │      │                 │
│  Permission     │      │  Database       │      │  Permission     │
│  Manager        │      │  RLS            │      │  Cache          │
│                 │      │                 │      │                 │
└─────────────────┘      └─────────────────┘      └─────────────────┘
```

### Core Components

#### Authorization Service
The main entry point for authorization operations.

```rust
pub struct AuthorizationService {
    role_manager: Arc<RoleManager>,
    permission_manager: Arc<PermissionManager>,
    policy_enforcer: Arc<PolicyEnforcer>,
    permission_cache: Arc<PermissionCache>,
}

impl AuthorizationService {
    pub async fn check_permission(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>
    ) -> Result<bool, AuthzError>;
    
    pub async fn get_user_permissions(
        &self, 
        user_id: i64, 
        resource_type: Option<&str>,
        resource_id: Option<i64>
    ) -> Result<Vec<String>, AuthzError>;
    
    pub async fn get_user_roles(
        &self, 
        user_id: i64
    ) -> Result<Vec<Role>, AuthzError>;
    
    pub async fn add_permission(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>,
        granted_by: i64
    ) -> Result<(), AuthzError>;
    
    pub async fn revoke_permission(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>,
        revoked_by: i64
    ) -> Result<(), AuthzError>;
    
    pub async fn check_ownership(
        &self, 
        user_id: i64, 
        resource_type: &str,
        resource_id: i64
    ) -> Result<bool, AuthzError>;
}
```

#### Role Manager
Manages roles and their hierarchies.

```rust
pub struct RoleManager {
    role_repository: Arc<dyn RoleRepository>,
}

impl RoleManager {
    pub async fn get_role(
        &self, 
        role_id: i64
    ) -> Result<Role, RoleError>;
    
    pub async fn get_role_by_name(
        &self, 
        name: &str
    ) -> Result<Role, RoleError>;
    
    pub async fn create_role(
        &self, 
        name: &str, 
        description: &str,
        parent_role_id: Option<i64>
    ) -> Result<Role, RoleError>;
    
    pub async fn update_role(
        &self, 
        role_id: i64, 
        name: Option<&str>, 
        description: Option<&str>,
        parent_role_id: Option<i64>
    ) -> Result<Role, RoleError>;
    
    pub async fn delete_role(
        &self, 
        role_id: i64
    ) -> Result<(), RoleError>;
    
    pub async fn assign_role_to_user(
        &self, 
        user_id: i64, 
        role_id: i64,
        assigned_by: i64
    ) -> Result<(), RoleError>;
    
    pub async fn remove_role_from_user(
        &self, 
        user_id: i64, 
        role_id: i64,
        removed_by: i64
    ) -> Result<(), RoleError>;
    
    pub async fn get_user_roles(
        &self, 
        user_id: i64
    ) -> Result<Vec<Role>, RoleError>;
    
    pub async fn get_role_permissions(
        &self, 
        role_id: i64
    ) -> Result<Vec<Permission>, RoleError>;
    
    pub async fn add_permission_to_role(
        &self, 
        role_id: i64, 
        permission: &str
    ) -> Result<(), RoleError>;
    
    pub async fn remove_permission_from_role(
        &self, 
        role_id: i64, 
        permission: &str
    ) -> Result<(), RoleError>;
}
```

#### Permission Manager
Handles permission operations and object-level access control.

```rust
pub struct PermissionManager {
    permission_repository: Arc<dyn PermissionRepository>,
    resource_repository: Arc<dyn ResourceRepository>,
}

impl PermissionManager {
    pub async fn add_permission(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>,
        granted_by: i64
    ) -> Result<(), PermissionError>;
    
    pub async fn revoke_permission(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>,
        revoked_by: i64
    ) -> Result<(), PermissionError>;
    
    pub async fn get_user_permissions(
        &self, 
        user_id: i64, 
        resource_type: Option<&str>,
        resource_id: Option<i64>
    ) -> Result<Vec<Permission>, PermissionError>;
    
    pub async fn check_ownership(
        &self, 
        user_id: i64, 
        resource_type: &str,
        resource_id: i64
    ) -> Result<bool, PermissionError>;
    
    pub async fn transfer_ownership(
        &self, 
        resource_type: &str,
        resource_id: i64,
        from_user_id: i64,
        to_user_id: i64,
        transferred_by: i64
    ) -> Result<(), PermissionError>;
}
```

#### Policy Enforcer
Enforces authorization policies at different layers.

```rust
pub struct PolicyEnforcer {
    role_manager: Arc<RoleManager>,
    permission_manager: Arc<PermissionManager>,
    permission_cache: Arc<PermissionCache>,
}

impl PolicyEnforcer {
    pub async fn can_access(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>
    ) -> Result<bool, PolicyError>;
    
    pub async fn enforce_access(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>
    ) -> Result<(), PolicyError>;
    
    pub async fn filter_accessible_resources<T>(
        &self, 
        user_id: i64, 
        permission: &str,
        resources: Vec<T>,
        id_extractor: impl Fn(&T) -> i64
    ) -> Result<Vec<T>, PolicyError>;
    
    pub async fn get_accessible_resource_ids(
        &self, 
        user_id: i64, 
        permission: &str,
        resource_type: &str
    ) -> Result<Vec<i64>, PolicyError>;
}
```

#### Database RLS Manager
Manages row-level security policies in PostgreSQL.

```rust
pub struct DatabaseRlsManager {
    db_pool: Arc<PgPool>,
}

impl DatabaseRlsManager {
    pub async fn set_security_context(
        &self, 
        conn: &mut PgConnection, 
        user_id: i64,
        roles: &[String],
        permissions: &[String]
    ) -> Result<(), RlsError>;
    
    pub async fn clear_security_context(
        &self, 
        conn: &mut PgConnection
    ) -> Result<(), RlsError>;
    
    pub async fn create_rls_policy(
        &self, 
        table_name: &str,
        policy_name: &str,
        using_expression: &str,
        command: RlsCommand,
        with_check_expression: Option<&str>
    ) -> Result<(), RlsError>;
    
    pub async fn drop_rls_policy(
        &self, 
        table_name: &str,
        policy_name: &str
    ) -> Result<(), RlsError>;
    
    pub async fn enable_rls(
        &self, 
        table_name: &str
    ) -> Result<(), RlsError>;
    
    pub async fn disable_rls(
        &self, 
        table_name: &str
    ) -> Result<(), RlsError>;
}

pub enum RlsCommand {
    All,
    Select,
    Insert,
    Update,
    Delete,
}
```

#### Permission Cache
Caches permission checks for better performance.

```rust
pub struct PermissionCache {
    cache: Arc<Cache<PermissionCacheKey, bool>>,
    ttl: Duration,
}

impl PermissionCache {
    pub async fn get(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>
    ) -> Option<bool>;
    
    pub async fn set(
        &self, 
        user_id: i64, 
        permission: &str, 
        resource_type: &str,
        resource_id: Option<i64>,
        allowed: bool
    ) -> Result<(), CacheError>;
    
    pub async fn invalidate(
        &self, 
        user_id: i64, 
        permission: Option<&str>, 
        resource_type: Option<&str>,
        resource_id: Option<i64>
    ) -> Result<(), CacheError>;
    
    pub async fn clear_all(&self) -> Result<(), CacheError>;
}
```

### Data Models

#### Role Data

```rust
pub struct Role {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub parent_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct UserRole {
    pub id: i64,
    pub user_id: i64,
    pub role_id: i64,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: i64,
    pub created_at: DateTime<Utc>,
}

pub struct RolePermission {
    pub id: i64,
    pub role_id: i64,
    pub permission: String,
    pub created_at: DateTime<Utc>,
}
```

#### Permission Data

```rust
pub struct Permission {
    pub id: i64,
    pub user_id: i64,
    pub resource_type: String,
    pub resource_id: Option<i64>,
    pub permission: String,
    pub granted_at: DateTime<Utc>,
    pub granted_by: i64,
    pub created_at: DateTime<Utc>,
}

pub struct Resource {
    pub resource_type: String,
    pub resource_id: i64,
    pub owner_id: i64,
    pub created_at: DateTime<Utc>,
    pub created_by: i64,
}
```

#### Permission Cache Key

```rust
pub struct PermissionCacheKey {
    pub user_id: i64,
    pub permission: String,
    pub resource_type: String,
    pub resource_id: Option<i64>,
}
```

## Database Schema

The following tables are required for the authorization component:

### Roles Table

```sql
CREATE TABLE roles (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    parent_id BIGINT REFERENCES roles(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_roles_parent_id ON roles(parent_id);
```

### Users-Roles Table

```sql
CREATE TABLE users_roles (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    role_id BIGINT NOT NULL REFERENCES roles(id),
    assigned_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    assigned_by BIGINT NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, role_id)
);

CREATE INDEX idx_users_roles_user_id ON users_roles(user_id);
CREATE INDEX idx_users_roles_role_id ON users_roles(role_id);
```

### Roles-Permissions Table

```sql
CREATE TABLE roles_permissions (
    id BIGSERIAL PRIMARY KEY,
    role_id BIGINT NOT NULL REFERENCES roles(id),
    permission VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(role_id, permission)
);

CREATE INDEX idx_roles_permissions_role_id ON roles_permissions(role_id);
CREATE INDEX idx_roles_permissions_permission ON roles_permissions(permission);
```

### User-Permissions Table

```sql
CREATE TABLE users_permissions (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    resource_type VARCHAR(255) NOT NULL,
    resource_id BIGINT,
    permission VARCHAR(255) NOT NULL,
    granted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    granted_by BIGINT NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, resource_type, resource_id, permission)
);

CREATE INDEX idx_users_permissions_user_id ON users_permissions(user_id);
CREATE INDEX idx_users_permissions_resource ON users_permissions(resource_type, resource_id);
```

### Resource-Ownership Table

```sql
CREATE TABLE resource_ownership (
    id BIGSERIAL PRIMARY KEY,
    resource_type VARCHAR(255) NOT NULL,
    resource_id BIGINT NOT NULL,
    owner_id BIGINT NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(resource_type, resource_id)
);

CREATE INDEX idx_resource_ownership_owner_id ON resource_ownership(owner_id);
CREATE INDEX idx_resource_ownership_resource ON resource_ownership(resource_type, resource_id);
```

### Object-Level Access Control Tables

For specific entities like books, accounts, etc., additional ACL tables can be created:

```sql
CREATE TABLE book_access_grants (
    id BIGSERIAL PRIMARY KEY,
    book_id BIGINT NOT NULL REFERENCES books(id),
    user_id BIGINT NOT NULL REFERENCES users(id),
    permission VARCHAR(50) NOT NULL,
    granted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    granted_by BIGINT NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(book_id, user_id, permission)
);

CREATE INDEX idx_book_access_grants_book_id ON book_access_grants(book_id);
CREATE INDEX idx_book_access_grants_user_id ON book_access_grants(user_id);
```

## RLS Policy Implementation

### Creating RLS Policies for Tables

For each table with sensitive data, RLS policies need to be created:

```sql
-- Enable RLS on a table
ALTER TABLE books ENABLE ROW LEVEL SECURITY;

-- Policy for select operations
CREATE POLICY books_select_policy ON books
    FOR SELECT
    USING (
        -- Owner access
        EXISTS (
            SELECT 1 FROM resource_ownership
            WHERE resource_type = 'book'
            AND resource_id = books.id
            AND owner_id = current_setting('app.current_user_id')::bigint
        )
        OR
        -- Explicit granted access
        EXISTS (
            SELECT 1 FROM book_access_grants
            WHERE book_id = books.id
            AND user_id = current_setting('app.current_user_id')::bigint
            AND permission = 'read:book'
        )
        OR
        -- Role-based access
        EXISTS (
            SELECT 1 
            FROM users_roles ur
            JOIN roles_permissions rp ON ur.role_id = rp.role_id
            WHERE ur.user_id = current_setting('app.current_user_id')::bigint
            AND (
                rp.permission = 'read:book:*'
                OR rp.permission = 'read:*'
            )
        )
    );

-- Policy for update operations
CREATE POLICY books_update_policy ON books
    FOR UPDATE
    USING (
        -- Owner access
        EXISTS (
            SELECT 1 FROM resource_ownership
            WHERE resource_type = 'book'
            AND resource_id = books.id
            AND owner_id = current_setting('app.current_user_id')::bigint
        )
        OR
        -- Explicit granted access
        EXISTS (
            SELECT 1 FROM book_access_grants
            WHERE book_id = books.id
            AND user_id = current_setting('app.current_user_id')::bigint
            AND permission = 'update:book'
        )
        OR
        -- Role-based access
        EXISTS (
            SELECT 1 
            FROM users_roles ur
            JOIN roles_permissions rp ON ur.role_id = rp.role_id
            WHERE ur.user_id = current_setting('app.current_user_id')::bigint
            AND (
                rp.permission = 'update:book:*'
                OR rp.permission = 'update:*'
            )
        )
    );

-- Policy for delete operations
CREATE POLICY books_delete_policy ON books
    FOR DELETE
    USING (
        -- Owner access
        EXISTS (
            SELECT 1 FROM resource_ownership
            WHERE resource_type = 'book'
            AND resource_id = books.id
            AND owner_id = current_setting('app.current_user_id')::bigint
        )
        OR
        -- Explicit granted access
        EXISTS (
            SELECT 1 FROM book_access_grants
            WHERE book_id = books.id
            AND user_id = current_setting('app.current_user_id')::bigint
            AND permission = 'delete:book'
        )
        OR
        -- Role-based access
        EXISTS (
            SELECT 1 
            FROM users_roles ur
            JOIN roles_permissions rp ON ur.role_id = rp.role_id
            WHERE ur.user_id = current_setting('app.current_user_id')::bigint
            AND (
                rp.permission = 'delete:book:*'
                OR rp.permission = 'delete:*'
            )
        )
    );

-- Admin bypass policy
CREATE POLICY books_admin_policy ON books
    USING (
        EXISTS (
            SELECT 1 
            FROM users_roles ur
            WHERE ur.user_id = current_setting('app.current_user_id')::bigint
            AND ur.role_id = (SELECT id FROM roles WHERE name = 'ADMIN')
        )
    );
```

### RLS Security Context Management

Before executing database operations, the security context needs to be set:

```rust
async fn set_security_context(
    conn: &mut PgConnection, 
    user_id: i64,
    roles: &[String],
    permissions: &[String]
) -> Result<(), sqlx::Error> {
    // Set user ID
    sqlx::query("SELECT set_config('app.current_user_id', $1, true)")
        .bind(user_id.to_string())
        .execute(conn)
        .await?;
    
    // Set roles
    let roles_json = serde_json::to_string(roles).unwrap_or_else(|_| "[]".to_string());
    sqlx::query("SELECT set_config('app.current_user_roles', $1, true)")
        .bind(roles_json)
        .execute(conn)
        .await?;
    
    // Set permissions
    let permissions_json = serde_json::to_string(permissions).unwrap_or_else(|_| "[]".to_string());
    sqlx::query("SELECT set_config('app.current_user_permissions', $1, true)")
        .bind(permissions_json)
        .execute(conn)
        .await?;
    
    Ok(())
}

async fn clear_security_context(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('app.current_user_id', '', true)")
        .execute(conn)
        .await?;
    sqlx::query("SELECT set_config('app.current_user_roles', '[]', true)")
        .execute(conn)
        .await?;
    sqlx::query("SELECT set_config('app.current_user_permissions', '[]', true)")
        .execute(conn)
        .await?;
    
    Ok(())
}
```

## Authorization Flows

### Permission Check Flow

1. Check if result is in cache
2. If not, check if user is resource owner
3. If not, check explicit user permissions
4. If not, check permissions granted by roles
5. Cache result
6. Return permission decision

```rust
async fn check_permission(
    user_id: i64, 
    permission: &str, 
    resource_type: &str,
    resource_id: Option<i64>
) -> Result<bool, AuthzError> {
    // Check cache
    if let Some(cached) = permission_cache.get(user_id, permission, resource_type, resource_id).await {
        return Ok(cached);
    }
    
    // Check ownership if resource_id is provided
    if let Some(resource_id) = resource_id {
        let is_owner = permission_manager.check_ownership(user_id, resource_type, resource_id).await?;
        if is_owner {
            // Cache result
            permission_cache.set(user_id, permission, resource_type, Some(resource_id), true).await?;
            return Ok(true);
        }
    }
    
    // Check explicit permissions
    let user_permissions = permission_manager.get_user_permissions(
        user_id, Some(resource_type), resource_id
    ).await?;
    
    if user_permissions.iter().any(|p| p.permission == permission) {
        // Cache result
        permission_cache.set(user_id, permission, resource_type, resource_id, true).await?;
        return Ok(true);
    }
    
    // Check role-based permissions
    let user_roles = role_manager.get_user_roles(user_id).await?;
    
    for role in user_roles {
        let role_permissions = role_manager.get_role_permissions(role.id).await?;
        
        // Check if role has the requested permission
        let has_permission = role_permissions.iter().any(|p| {
            // Exact permission match
            p.permission == permission ||
            // Wildcard match for type
            p.permission == format!("{}:*", permission.split(':').next().unwrap_or("")) ||
            // Wildcard match for all
            p.permission == "*"
        });
        
        if has_permission {
            // Cache result
            permission_cache.set(user_id, permission, resource_type, resource_id, true).await?;
            return Ok(true);
        }
    }
    
    // No permission found
    permission_cache.set(user_id, permission, resource_type, resource_id, false).await?;
    Ok(false)
}
```

### Role Assignment Flow

1. Admin or manager requests to assign a role to a user
2. System verifies admin/manager has permission to assign roles
3. System assigns the role to the user
4. System logs the role assignment with the assigner's ID
5. Permission cache for the user is invalidated

### Resource Access Grant Flow

1. Resource owner initiates permission grant
2. System verifies granter is owner or has grant permission
3. System creates resource access grant record
4. System logs the grant operation
5. Permission cache for the user is invalidated

### Ownership Transfer Flow

1. Current owner initiates ownership transfer
2. System verifies current owner has ownership
3. System updates ownership record
4. System grants previous owner appropriate access rights
5. System logs the ownership transfer
6. Permission caches for both users are invalidated

## Security Considerations

### Permission Design

- Use a consistent permission naming format: `action:resource_type[:resource_id]`
- Implement least privilege principle by default
- Use wildcards sparingly and only for admin roles
- Regularly audit and review role permissions

### Performance Optimization

- Cache common permission checks
- Optimize RLS policies with appropriate indexes
- Use prepared statements for permission queries
- Consider materialized views for complex permission relationships
- Implement batch permission checking for UI screens

### Secure Implementation

- Validate all inputs in permission checks
- Prevent permission escalation attacks
- Log all permission changes for audit
- Regularly review role assignments
- Implement time-limited permission grants for temporary access

## Implementation Guidelines

### API Layer Authorization

```rust
pub async fn authorize_api_request<B>(
    req: Request<B>,
    auth_service: Arc<AuthorizationService>,
    required_permission: &'static str,
    next: Next<B>
) -> Result<Response, StatusCode> {
    // Get user from authentication context
    let auth_ctx = req.extensions()
        .get::<AuthContext>()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Determine resource type and ID from request
    let (resource_type, resource_id) = extract_resource_info(&req);
    
    // Check permission
    let authorized = auth_service
        .check_permission(
            auth_ctx.user_id, 
            required_permission, 
            resource_type, 
            resource_id
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if !authorized {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Continue to the next middleware/handler
    Ok(next.run(req).await)
}
```

### Service Layer Authorization

```rust
impl BookService {
    pub async fn get_book(&self, user_id: i64, book_id: i64) -> Result<Book, ServiceError> {
        // Check permission
        let authorized = self.auth_service
            .check_permission(user_id, "read:book", "book", Some(book_id))
            .await?;
        
        if !authorized {
            return Err(ServiceError::Forbidden);
        }
        
        // Proceed with getting the book
        let book = self.book_repository.find_by_id(book_id).await?;
        Ok(book)
    }
    
    pub async fn update_book(
        &self, 
        user_id: i64, 
        book_id: i64, 
        data: BookUpdateData
    ) -> Result<Book, ServiceError> {
        // Check permission
        let authorized = self.auth_service
            .check_permission(user_id, "update:book", "book", Some(book_id))
            .await?;
        
        if !authorized {
            return Err(ServiceError::Forbidden);
        }
        
        // Proceed with updating the book
        let book = self.book_repository.update(book_id, data).await?;
        Ok(book)
    }
}
```

### Database Layer Authorization (RLS)

For each database operation, ensure the security context is set:

```rust
impl BookRepository {
    pub async fn find_by_id(&self, book_id: i64) -> Result<Book, RepositoryError> {
        // Get connection from pool
        let mut conn = self.pool.acquire().await?;
        
        // Security context is already set by middleware
        // RLS policies will automatically filter results
        
        let book = sqlx::query_as::<_, Book>("SELECT * FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_optional(&mut conn)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        
        Ok(book)
    }
    
    pub async fn find_all(&self) -> Result<Vec<Book>, RepositoryError> {
        // Get connection from pool
        let mut conn = self.pool.acquire().await?;
        
        // Security context is already set by middleware
        // RLS policies will automatically filter results
        
        let books = sqlx::query_as::<_, Book>("SELECT * FROM books")
            .fetch_all(&mut conn)
            .await?;
        
        Ok(books)
    }
}
```

### Middleware for Setting Security Context

```rust
pub async fn security_context_middleware<B>(
    req: Request<B>,
    db_pool: Arc<PgPool>,
    next: Next<B>
) -> Result<Response, StatusCode> {
    // Get authentication context
    let auth_ctx = req.extensions()
        .get::<AuthContext>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Set security context for this request
    let mut conn = db_pool.acquire().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    DatabaseRlsManager::set_security_context(
        &mut conn,
        auth_ctx.user_id,
        &auth_ctx.roles,
        &auth_ctx.permissions
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Store connection in request extensions
    let mut req = req;
    req.extensions_mut().insert(DbConnection(conn));
    
    // Continue with request
    let response = next.run(req).await;
    
    // Clear security context after request is complete
    // This is handled in a separate middleware that runs after the response is generate
