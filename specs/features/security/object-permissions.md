# Object-Level Permissions Feature Specification

## Overview
This document outlines the object-level permissions feature for Ratio, which provides fine-grained access control at the resource level (books, accounts, transactions, etc.). The feature enables resource owners to control who can access their data and what operations they can perform, while enforcing security boundaries through PostgreSQL's row-level security.

## Goals
- Implement fine-grained access control at the object level
- Ensure data isolation between users and resources
- Provide intuitive interfaces for managing object permissions
- Enforce security through database row-level security
- Support collaborative workflows while maintaining security
- Create a consistent permission model across all object types

## User Stories

### Book Owner Stories
1. As a book owner, I want to control who can view my financial book so that I can maintain privacy
2. As a book owner, I want to grant specific permissions on my book to collaborators so that we can work together
3. As a book owner, I want to see everyone who has access to my book so that I can manage access effectively
4. As a book owner, I want to revoke access to my book when needed so that I can remove unneeded access
5. As a book owner, I want different collaborators to have different levels of access so that I can limit sensitive operations

### Account Owner Stories
1. As an account owner, I want to restrict account visibility within a book so that sensitive accounts remain private
2. As an account owner, I want to delegate account management to trusted users so that they can help maintain it
3. As an account owner, I want to limit transaction creation rights on my accounts so that only authorized users can add transactions
4. As an account owner, I want to set read-only access for certain users so they can view but not modify accounts
5. As an account owner, I want account permissions to be intuitive and easy to manage so I can maintain proper security

### Collaborator Stories
1. As a collaborator, I want to easily see which books and accounts I have access to so that I can find my work
2. As a collaborator, I want to understand what actions I can perform on each resource so that I know my boundaries
3. As a collaborator, I want to request additional permissions when needed so that I can complete my tasks
4. As a collaborator, I want consistent permissions across similar resources so that I have a predictable experience
5. As a collaborator, I want to understand why I was denied access to a resource so that I can request appropriate permissions

## Feature Requirements

### Resource Ownership Model

#### Primary Ownership
- Every resource has a single primary owner
- Ownership is established at resource creation time
- Ownership can be transferred to another user
- Owners have full control over their resources
- Ownership information is used for row-level security enforcement

#### Ownership Transfer
- Allow transferring ownership to another user
- Require confirmation from the new owner
- Maintain previous owner as a collaborator by default
- Log ownership transfers for audit
- Ensure seamless transfer without permissions disruption

#### Shared Ownership (Co-owners)
- Allow designation of co-owners with nearly full access
- Co-owners can manage permissions but cannot transfer primary ownership
- Co-owners have access to sensitive operations
- Co-owners can be added or removed by the primary owner
- Co-ownership is prominently displayed in interfaces

### Permission Levels

#### Book Permission Levels
- **Owner**: Full control of the book and all contained objects
- **Administrator**: Can manage book settings and user permissions
- **Editor**: Can create, update transactions and accounts
- **Contributor**: Can create transactions but not modify structure
- **Viewer**: Read-only access to the book and its data
- **Custom**: Tailored permission sets for specific requirements

#### Account Permission Levels
- **Manager**: Full control of the account and its transactions
- **Contributor**: Can add and edit transactions in the account
- **Viewer**: Read-only access to the account and transactions
- **No Access**: Account is hidden from the user's view

#### Transaction Permission Levels
- **Editor**: Can create, edit, and delete transactions
- **Viewer**: Can view transaction details
- **No Access**: Transaction is hidden from the user's view

### Inheritance & Propagation

#### Permission Hierarchy
- Permissions cascade from books to accounts to transactions
- Higher level permission grants implicitly provide lower level access
- More permissive access at a higher level overrides restrictions at lower levels
- Explicit denials can override inherited permissions
- Permission inheritance follows the resource containment hierarchy

#### Effective Permissions
- Calculate effective permissions based on direct and inherited grants
- Respect the most permissive applicable permission
- Apply security principle of least-privilege for conflicts
- Provide clear visibility into how effective permissions are derived
- Cache effective permissions for performance

#### Scope Limitations
- Allow setting permissions that apply only to specific resource subsets
- Support time-based permission scopes (temporary access)
- Implement purpose-based permission grants (specific task access)
- Enable conditional permissions based on resource state
- Provide permission templates for common collaboration patterns

### Row-Level Security Integration

#### Security Context
- Set user context on database connections
- Include user ID, roles, and permissions in security context
- Maintain per-connection isolation of security context
- Clear security context after request completion
- Implement security definer functions for administrative operations

#### RLS Policy Implementation
- Create RLS policies for all sensitive tables
- Enforce ownership checks in RLS policies
- Implement permission-based access control in policies
- Create specialized policies for common query patterns
- Optimize policy expressions for performance

#### Multi-Tenant Security
- Ensure complete isolation between different user data
- Prevent access to unauthorized books and accounts
- Implement security boundaries at the database level
- Enforce strict validation of ownership and permissions
- Maintain audit trail of all access attempts

### Collaboration Workflows

#### Invitation Process
- Allow resource owners to invite users by email
- Send secure invitation links with expiration
- Present clear permission information during acceptance
- Support invitation to users not yet registered
- Track invitation status and send reminders

#### Permission Requests
- Enable users to request access to resources
- Notify owners of permission requests
- Provide context for why access is needed
- Allow approving or denying requests with comments
- Suggest appropriate permission levels based on request

#### Bulk Operations
- Support batch permission changes across multiple users
- Allow template-based permission assignments
- Provide permission cloning between similar resources
- Implement permission presets for common scenarios
- Enable bulk revocation for emergency access removal

## User Interfaces

### Book Permissions Interface

```
┌────────────────────────────────────────────────────────────────────┐
│ Book Permissions: Household Budget 2025                             │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ Current Access                                              │    │
│  ├───────────┬──────────────┬─────────────┬──────────────────┬┘    │
│  │ User      │ Access Level │ Granted By  │ Actions          │     │
│  ├───────────┼──────────────┼─────────────┼──────────────────┤     │
│  │ John Doe  │ Owner        │ Self        │ [Transfer]       │     │
│  │           │              │             │                  │     │
│  │ Alice S.  │ Administrator│ John Doe    │ [Edit] [Remove] │     │
│  │           │              │             │                  │     │
│  │ Bob J.    │ Editor       │ John Doe    │ [Edit] [Remove] │     │
│  │           │              │             │                  │     │
│  │ Charlie W.│ Viewer       │ Alice S.    │ [Edit] [Remove] │     │
│  └───────────┴──────────────┴─────────────┴──────────────────┘     │
│                                                                     │
│  [ Invite People ]  [ View Access Requests (2) ]                    │
│                                                                     │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ Advanced Permissions                                        │    │
│  ├────────────────────────────────────────────────────────────┤    │
│  │                                                             │    │
│  │  [ ] Apply permissions to all accounts in this book         │    │
│  │                                                             │    │
│  │  [ ] Allow editors to share with others                     │    │
│  │                                                             │    │
│  │  [ ] Restrict export and printing of data                   │    │
│  │                                                             │    │
│  │  [ ] Enable temporary access expiration                     │    │
│  │                                                             │    │
│  │  [Configure Defaults] [Configure Account-Specific Access]   │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                     │
│  [ Save Changes ]                                                   │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

### Account Permissions Interface

```
┌────────────────────────────────────────────────────────────────────┐
│ Account Permissions: Checking Account                               │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  This account is part of: Household Budget 2025                     │
│                                                                     │
│  ⓘ Users with access to the parent book automatically have         │
│    some level of access to this account.                            │
│                                                                     │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ Custom Account Access                                       │    │
│  ├───────────┬──────────────┬─────────────┬──────────────────┬┘    │
│  │ User      │ Access Level │ Source      │ Actions          │     │
│  ├───────────┼──────────────┼─────────────┼──────────────────┤     │
│  │ John Doe  │ Manager      │ Book Owner  │ [Default]        │     │
│  │           │              │             │                  │     │
│  │ Alice S.  │ Manager      │ Book Admin  │ [Default]        │     │
│  │           │              │             │                  │     │
│  │ Bob J.    │ No Access    │ Custom      │ [Edit] [Remove] │     │
│  │           │              │             │                  │     │
│  │ Charlie W.│ Viewer       │ Book Viewer │ [Default]        │     │
│  │           │              │             │                  │     │
│  │ Dana T.   │ Contributor  │ Custom      │ [Edit] [Remove] │     │
│  └───────────┴──────────────┴─────────────┴──────────────────┘     │
│                                                                     │
│  ⓘ "Default" means this access is inherited from book-level        │
│    permissions. You can override with custom account access.        │
│                                                                     │
│  [ Add Custom Access ]  [ Reset All to Book Defaults ]              │
│                                                                     │
│  [ Save Changes ]                                                   │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

### Effective Permissions View

```
┌────────────────────────────────────────────────────────────────────┐
│ User Effective Permissions: Charlie Williams                        │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ Book Access                                                 │    │
│  ├────────────────┬──────────────┬─────────────────────────┬──┘    │
│  │ Resource       │ Access Level │ Source                  │       │
│  ├────────────────┼──────────────┼─────────────────────────┤       │
│  │ Household      │ Viewer       │ Direct Grant from       │       │
│  │ Budget 2025    │              │ Alice Smith on 4/20/25  │       │
│  └────────────────┴──────────────┴─────────────────────────┘       │
│                                                                     │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ Account Access                                              │    │
│  ├────────────────┬──────────────┬─────────────────────────┬──┘    │
│  │ Account        │ Access Level │ Source                  │       │
│  ├────────────────┼──────────────┼─────────────────────────┤       │
│  │ Checking       │ Viewer       │ Inherited from Book     │       │
│  │                │              │                         │       │
│  │ Savings        │ Viewer       │ Inherited from Book     │       │
│  │                │              │                         │       │
│  │ Credit Card    │ No Access    │ Custom Override by      │       │
│  │                │              │ John Doe on 4/22/25     │       │
│  │                │              │                         │       │
│  │ Investments    │ Contributor  │ Custom Grant from       │       │
│  │                │              │ John Doe on 4/21/25     │       │
│  └────────────────┴──────────────┴─────────────────────────┘       │
│                                                                     │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ Permission Details                                          │    │
│  ├────────────────────────────────────┬─────────────────────┬─┘    │
│  │ Permission                         │ Status              │      │
│  ├────────────────────────────────────┼─────────────────────┤      │
│  │ View book summary                  │ ✓ Allowed          │      │
│  │ View account balances              │ ✓ Allowed          │      │
│  │ View transactions                  │ ✓ Allowed          │      │
│  │ Create transactions                │ ✓ On Investments   │      │
│  │ Edit transactions                  │ ✓ Own transactions │      │
│  │ Delete transactions                │ ✗ Denied           │      │
│  │ Create accounts                    │ ✗ Denied           │      │
│  │ Edit account settings              │ ✗ Denied           │      │
│  │ Share book with others             │ ✗ Denied           │      │
│  └────────────────────────────────────┴─────────────────────┘      │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

### Permission Request Interface

```
┌────────────────────────────────────────────────────────────────────┐
│ Request Access                                                      │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  You currently don't have permission to edit "Credit Card" account  │
│  in "Household Budget 2025".                                        │
│                                                                     │
│  Request access from the account owner:                             │
│                                                                     │
│  Access needed: [Contributor (add/edit transactions)        (▼)]    │
│                                                                     │
│  Reason for request:                                                │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ I need to add my recent credit card transactions           │    │
│  │ for the month. I can only see the account now but          │    │
│  │ cannot add new transactions.                               │    │
│  │                                                            │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                     │
│  [ Cancel ]                           [ Send Request ]              │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

### Ownership Transfer Interface

```
┌────────────────────────────────────────────────────────────────────┐
│ Transfer Ownership                                                  │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  You're about to transfer ownership of "Household Budget 2025".     │
│                                                                     │
│  Current owner: John Doe (you)                                      │
│                                                                     │
│  New owner: [Alice Smith                                    (▼)]    │
│                                                                     │
│  After transfer:                                                    │
│                                                                     │
│  - Alice Smith will become the primary owner                        │
│  - Alice Smith will have full control over the book                 │
│  - You will retain Administrator access                             │
│  - You can be removed by the new owner                              │
│                                                                     │
│  This action cannot be undone automatically. The new owner          │
│  would need to transfer ownership back to you.                      │
│                                                                     │
│  [ Cancel ]                           [ Transfer Ownership ]        │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

## Workflows

### Object Permission Assignment Workflow

1. Resource owner navigates to resource permissions interface
2. Owner views current access list for the resource
3. Owner selects "Add Custom Access" or "Invite People"
4. Owner selects a user and assigns appropriate permission level
5. System validates that owner has authority to grant permissions
6. System creates resource access grant record
7. System logs permission change for audit
8. Target user receives notification of new access
9. User can now access the resource with granted permissions

### Permission Inheritance Override Workflow

1. Resource owner navigates to account permissions interface
2. System shows inherited permissions from parent book
3. Owner selects a user with inherited permissions
4. Owner chooses "Override" and selects custom permission level
5. System explains the implications of permission override
6. Owner confirms the override
7. System creates custom permission record
8. System logs the permission change
9. User's effective permissions are updated
10. User is notified of permission change

### Permission Request and Approval Workflow

1. User attempts to access or modify a resource
2. System denies access due to insufficient permissions
3. User initiates permission request with justification
4. System routes request to resource owner
5. Owner receives notification of permission request
6. Owner reviews request details and justification
7. Owner approves or denies the request
8. If approved, system grants requested permission
9. System logs the request and decision
10. User receives notification of request outcome
11. If approved, user can now perform the requested action

### Ownership Transfer Workflow

1. Current owner initiates ownership transfer
2. Owner selects new owner from eligible users
3. System displays confirmation with implications
4. Current owner confirms transfer
5. System notifies new owner of pending transfer
6. New owner accepts or rejects the transfer
7. If accepted, system updates ownership records
8. System maintains current owner as Administrator
9. System logs ownership transfer for audit
10. Both users receive confirmation of completed transfer

### Object Visibility Determination Workflow

1. User accesses a list of books or accounts
2. System retrieves all resources from database
3. PostgreSQL RLS automatically filters resources based on user context
4. System determines effective permission level for each visible resource
5. System displays resources with appropriate actions based on permissions
6. User sees only resources they have permission to access
7. Restricted resources are completely hidden from user's view

## RLS Implementation Details

### Book-Level RLS Policies

```sql
-- Enable RLS on books table
ALTER TABLE books ENABLE ROW LEVEL SECURITY;

-- Policy for viewing books
CREATE POLICY books_select_policy ON books
    FOR SELECT
    USING (
        -- Owner can view
        book_owner_id = current_setting('app.current_user_id')::bigint
        OR
        -- Users with explicit grants can view
        EXISTS (
            SELECT 1 FROM book_permissions
            WHERE book_id = books.id
            AND user_id = current_setting('app.current_user_id')::bigint
            AND permission_level IN ('VIEWER', 'CONTRIBUTOR', 'EDITOR', 'ADMINISTRATOR')
        )
        OR
        -- Users with admin role can view all books
        current_setting('app.current_user_roles')::jsonb ? 'ADMIN'
    );

-- Policy for updating books
CREATE POLICY books_update_policy ON books
    FOR UPDATE
    USING (
        -- Owner can update
        book_owner_id = current_setting('app.current_user_id')::bigint
        OR
        -- Admins and editors can update
        EXISTS (
            SELECT 1 FROM book_permissions
            WHERE book_id = books.id
            AND user_id = current_setting('app.current_user_id')::bigint
            AND permission_level IN ('EDITOR', 'ADMINISTRATOR')
        )
        OR
        -- Users with admin role can update all books
        current_setting('app.current_user_roles')::jsonb ? 'ADMIN'
    );

-- Policy for deleting books
CREATE POLICY books_delete_policy ON books
    FOR DELETE
    USING (
        -- Only owner can delete
        book_owner_id = current_setting('app.current_user_id')::bigint
        OR
        -- Users with admin role can delete all books
        current_setting('app.current_user_roles')::jsonb ? 'ADMIN'
    );
```

### Account-Level RLS Policies

```sql
-- Enable RLS on accounts table
ALTER TABLE accounts ENABLE ROW LEVEL SECURITY;

-- Policy for viewing accounts
CREATE POLICY accounts_select_policy ON accounts
    FOR SELECT
    USING (
        -- Users can view accounts in books they have access to, unless overridden
        (
            EXISTS (
                SELECT 1 FROM books b
                WHERE b.id = accounts.book_id
                AND (
                    -- Book owner
                    b.book_owner_id = current_setting('app.current_user_id')::bigint
                    OR
                    -- Book permission
                    EXISTS (
                        SELECT 1 FROM book_permissions bp
                        WHERE bp.book_id = b.id
                        AND bp.user_id = current_setting('app.current_user_id')::bigint
                        AND bp.permission_level IN ('VIEWER', 'CONTRIBUTOR', 'EDITOR', 'ADMINISTRATOR')
                    )
                )
            )
            AND
            -- Not explicitly denied at account level
            NOT EXISTS (
                SELECT 1 FROM account_permissions ap
                WHERE ap.account_id = accounts.id
                AND ap.user_id = current_setting('app.current_user_id')::bigint
                AND ap.permission_level = 'NO_ACCESS'
            )
        )
        OR
        -- Explicit account permission
        EXISTS (
            SELECT 1 FROM account_permissions ap
            WHERE ap.account_id = accounts.id
            AND ap.user_id = current_setting('app.current_user_id')::bigint
            AND ap.permission_level IN ('VIEWER', 'CONTRIBUTOR', 'MANAGER')
        )
        OR
        -- Admin role
        current_setting('app.current_user_roles')::jsonb ? 'ADMIN'
    );

-- Policy for updating accounts
CREATE POLICY accounts_update_policy ON accounts
    FOR UPDATE
    USING (
        -- Book owner can update
        EXISTS (
            SELECT 1 FROM books b
            WHERE b.id = accounts.book_id
            AND b.book_owner_id = current_setting('app.current_user_id')::bigint
        )
        OR
        -- Book administrator can update
        EXISTS (
            SELECT 1 FROM book_permissions bp
            WHERE bp.book_id = accounts.book_id
            AND bp.user_id = current_setting('app.current_user_id')::bigint
            AND bp.permission_level = 'ADMINISTRATOR'
        )
        OR
        -- Book editor can update unless explicitly restricted
        (
            EXISTS (
                SELECT 1 FROM book_permissions bp
                WHERE bp.book_id = accounts.book_id
                AND bp.user_id = current_setting('app.current_user_id')::bigint
                AND bp.permission_level = 'EDITOR'
            )
            AND
            NOT EXISTS (
                SELECT 1 FROM account_permissions ap
                WHERE ap.account_id = accounts.id
                AND ap.user_id = current_setting('app.current_user_id')::bigint
                AND ap.permission_level IN ('VIEWER', 'NO_ACCESS')
            )
        )
        OR
        -- Account manager can update
        EXISTS (
            SELECT 1 FROM account_permissions ap
            WHERE ap.account_id = accounts.id
            AND ap.user_id = current_setting('app.current_user_id')::bigint
            AND ap.permission_level = 'MANAGER'
        )
        OR
        -- Admin role
        current_setting('app.current_user_roles')::jsonb ? 'ADMIN'
    );
```

### Transaction-Level RLS Policies

```sql
-- Enable RLS on transactions table
ALTER TABLE transactions ENABLE ROW LEVEL SECURITY;

-- Policy for viewing transactions
CREATE POLICY transactions_select_policy ON transactions
    FOR SELECT
    USING (
        -- Users can view transactions in accounts they have access to
        EXISTS (
            SELECT 1 FROM accounts a
            WHERE a.id = transactions.account_id
            AND (
                -- Book owner
                EXISTS (
                    SELECT 1 FROM books b
                    WHERE b.id = a.book_id
                    AND b.book_owner_id = current_setting('app.current_user_id')::bigint
                )
                OR
                -- Book permission
                EXISTS (
                    SELECT 1 FROM book_permissions bp
                    JOIN books b ON bp.book_id = b.id
                    WHERE b.id = a.book_id
                    AND bp.user_id = current_setting('app.current_user_id')::bigint
                    AND bp.permission_level IN ('VIEWER', 'CONTRIBUTOR', 'EDITOR', 'ADMINISTRATOR')
                    AND NOT EXISTS (
                        -- Not explicitly denied at account level
                        SELECT 1 FROM account_permissions ap
                        WHERE ap.account_id = a.id
                        AND ap.user_id = current_setting('app.current_user_id')::bigint
                        AND ap.permission_level = 'NO_ACCESS'
                    )
                )
                OR
                -- Account permission
                EXISTS (
                    SELECT 1 FROM account_permissions ap
                    WHERE ap.account_id = a.id
                    AND ap.user_id = current_setting('app.current_user_id')::bigint
                    AND ap.permission_level IN ('VIEWER', 'CONTRIBUTOR', 'MANAGER')
                )
            )
        )
        OR
        -- Transaction created by user
        transactions.created_by_user_id = current_setting('app.current_user_id')::bigint
        OR
        -- Admin role
        current_setting('app.current_user_roles')::jsonb ? 'ADMIN'
    );

-- Policy for updating transactions
CREATE POLICY transactions_update_policy ON transactions
    FOR UPDATE
    USING (
        -- Users can update transactions they created
        transactions.created_by_user_id = current_setting('app.current_user_id')::bigint
        OR
        -- Book owner
        EXISTS (
            SELECT 1 FROM accounts a
            JOIN books b ON a.book_id = b.id
            WHERE a.id = transactions.account_id
            AND b.book_owner_id = current_setting('app.current_user_id')::bigint
        )
        OR
        -- Book administrator or editor
        EXISTS (
            SELECT 1 FROM accounts a
            JOIN book_permissions bp ON a.book_id = bp.book_id
            WHERE a.id = transactions.account_id
            AND bp.user_id = current_setting('app.current_user_id')::bigint
            AND bp.permission_level IN ('ADMINISTRATOR', 'EDITOR')
        )
        OR
        -- Account manager or contributor
        EXISTS (
            SELECT 1 FROM account_permissions ap
            WHERE ap.account_id = transactions.account_id
            AND ap.user_id = current_setting('app.current_user_id')::bigint
            AND ap.permission_level IN ('MANAGER', 'CONTRIBUTOR')
        )
        OR
        -- Admin role
        current_setting('app.current_user_roles')::jsonb ? 'ADMIN'
    );
```

## Technical Implementation Considerations

### Security Context Management

```rust
// Database Connection Middleware
async fn set_security_context(
    conn: &mut PgConnection,
    auth_context: &AuthContext,
) -> Result<(), DbError> {
    // Set user ID
    sqlx::query("SELECT set_config('app.current_user_id', $1, true)")
        .bind(auth_context.user_id.to_string())
        .execute(conn)
        .await?;
    
    // Set user roles
    let roles_json = serde_json::to_string(&auth_context.roles)?;
    sqlx::query("SELECT set_config('app.current_user_roles', $1, true)")
        .bind(roles_json)
        .execute(conn)
        .await?;
    
    // Set user permissions
    let permissions_json = serde_json::to_string(&auth_context.permissions)?;
    sqlx::query("SELECT set_config('app.current_user_permissions', $1, true)")
        .bind(permissions_json)
        .execute(conn)
        .await?;
    
    Ok(())
}

// Clear security context when done
async fn clear_security_context(conn: &mut PgConnection) -> Result<(), DbError> {
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

### Effective Permission Computation

```rust
// Calculate effective permissions for a user on a book
async fn calculate_book_effective_permissions(
    user_id: i64,
    book_id: i64,
    conn: &mut PgConnection
) -> Result<EffectivePermissions, PermissionError> {
    // Check if user is owner
    let is_owner = sqlx::query_scalar!(
        "SELECT book_owner_id = $1 FROM books WHERE id = $2",
        user_id,
        book_id
    )
    .fetch_one(conn)
    .await?;
    
    if is_owner {
        return Ok(EffectivePermissions {
            resource_type: "book".to_string(),
            resource_id: book_id,
            permission_level: "OWNER".to_string(),
            source: "ownership".to_string(),
            permissions: get_owner_permissions(),
        });
    }
    
    // Check for direct book permissions
    let book_permission = sqlx::query!(
        "SELECT permission_level FROM book_permissions 
         WHERE book_id = $1 AND user_id = $2",
        book_id,
        user_id
    )
    .fetch_optional(conn)
    .await?;
    
    if let Some(perm) = book_permission {
        return Ok(EffectivePermissions {
            resource_type: "book".to_string(),
            resource_id: book_id,
            permission_level: perm.permission_level,
            source: "direct_grant".to_string(),
            permissions: get_permissions_for_level("book", &perm.permission_level),
        });
    }
