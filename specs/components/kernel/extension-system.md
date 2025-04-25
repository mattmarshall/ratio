# Component Specification: Extension System

## Overview
The Extension System component provides a flexible architecture for extending the Ratio application's functionality through hooks and event listeners. It enables both native Rust extensions and Python modules via PyO3 integration, allowing for customization without modifying the core code.

## Responsibilities
- Define extension points for core system operations
- Provide hook interfaces for intercepting and modifying behavior
- Implement event system for asynchronous notifications
- Enable Python integration through PyO3
- Manage extension lifecycle (loading, execution, unloading)
- Provide security boundaries for extensions
- Support version compatibility checking

## Design
The Extension System follows a modular design with clear interfaces and a registry pattern for managing hooks and event listeners.

### Key Abstractions

#### Hook System

The hook system allows extensions to intercept and modify operations at well-defined points:

```rust
/// Generic hook trait that all specific hooks implement
pub trait Hook: Send + Sync {
    /// Get the unique ID of this hook
    fn id(&self) -> &str;
    
    /// Get the hook type
    fn hook_type(&self) -> &str;
    
    /// Check if hook is enabled
    fn is_enabled(&self) -> bool;
    
    /// Enable or disable the hook
    fn set_enabled(&mut self, enabled: bool);
}

/// Hook interface for the transaction creation lifecycle
pub trait TransactionHook: Hook + Send + Sync {
    /// Called before a transaction is created
    async fn before_create(&self, transaction: &mut NewTransaction) -> Result<(), Error>;
    
    /// Called after a transaction is created
    async fn after_create(&self, transaction: &Transaction) -> Result<(), Error>;
    
    /// Called before a transaction is posted
    async fn before_post(&self, transaction: &Transaction) -> Result<(), Error>;
    
    /// Called after a transaction is posted
    async fn after_post(&self, transaction: &Transaction) -> Result<(), Error>;
}

/// Hook interface for account operations
pub trait AccountHook: Hook + Send + Sync {
    /// Called before an account is created
    async fn before_account_create(&self, account: &mut NewAccount) -> Result<(), Error>;
    
    /// Called after an account is created
    async fn after_account_create(&self, account: &Account) -> Result<(), Error>;
    
    /// Called before an account balance is calculated
    async fn before_balance_calculation(&self, account_id: i64, as_of: Option<DateTime<Utc>>) -> Result<(), Error>;
    
    /// Called after an account balance is calculated
    async fn after_balance_calculation(&self, balance: &AccountBalance) -> Result<(), Error>;
}
```

#### Hook Registry

The registry manages registration and execution of hooks:

```rust
/// Hook registry for managing extension hooks
pub struct HookRegistry {
    transaction_hooks: Vec<Box<dyn TransactionHook>>,
    account_hooks: Vec<Box<dyn AccountHook>>,
    // Other hook types...
}

impl HookRegistry {
    /// Create a new hook registry
    pub fn new() -> Self {
        Self {
            transaction_hooks: Vec::new(),
            account_hooks: Vec::new(),
        }
    }
    
    /// Register a transaction hook
    pub fn register_transaction_hook(&mut self, hook: Box<dyn TransactionHook>) {
        self.transaction_hooks.push(hook);
    }
    
    /// Register an account hook
    pub fn register_account_hook(&mut self, hook: Box<dyn AccountHook>) {
        self.account_hooks.push(hook);
    }
    
    /// Run before transaction create hooks
    pub async fn run_before_create_hooks(&self, transaction: &mut NewTransaction) -> Result<(), Error> {
        for hook in &self.transaction_hooks {
            if hook.is_enabled() {
                hook.before_create(transaction).await?;
            }
        }
        Ok(())
    }
    
    /// Get all hooks of a specific type
    pub fn get_hooks<T: 'static>(&self) -> Vec<&Box<dyn T>> {
        match std::any::type_name::<T>() {
            "TransactionHook" => self.transaction_hooks.iter()
                .map(|h| h as &Box<dyn TransactionHook>)
                .collect(),
            "AccountHook" => self.account_hooks.iter()
                .map(|h| h as &Box<dyn AccountHook>)
                .collect(),
            _ => Vec::new(),
        }
    }
    
    // Other hook execution methods...
}
```

#### Event System

The event system provides asynchronous notification of system events:

```rust
/// Event types that can be emitted by the kernel
pub enum KernelEvent {
    TransactionCreated(Transaction),
    TransactionUpdated(Transaction),
    TransactionPosted(Transaction),
    TransactionVoided(Transaction),
    AccountCreated(Account),
    AccountUpdated(Account),
    BalanceCalculated(AccountBalance),
    // Other event types...
}

/// Event listener trait
pub trait EventListener: Send + Sync {
    /// Get the unique ID of this listener
    fn id(&self) -> &str;
    
    /// Get event types this listener is interested in
    fn event_types(&self) -> Vec<String>;
    
    /// Handle an event
    fn on_event(&self, event: &KernelEvent) -> Result<(), Error>;
}

/// Event bus for publishing and subscribing to kernel events
pub struct EventBus {
    listeners: Vec<Box<dyn EventListener>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }
    
    /// Register a new event listener
    pub fn register_listener(&mut self, listener: Box<dyn EventListener>) {
        self.listeners.push(listener);
    }
    
    /// Publish an event to all registered listeners
    pub async fn publish(&self, event: KernelEvent) -> Result<(), Error> {
        let event_type = match &event {
            KernelEvent::TransactionCreated(_) => "TransactionCreated",
            KernelEvent::TransactionUpdated(_) => "TransactionUpdated",
            // Other event type mappings...
            _ => "Unknown",
        };
        
        for listener in &self.listeners {
            if listener.event_types().contains(&event_type.to_string()) {
                listener.on_event(&event)?;
            }
        }
        Ok(())
    }
}
```

### Python Integration

The PyO3 integration allows Python modules to define hooks and event listeners:

```rust
/// Python extension bridge using PyO3
pub struct PythonExtensionBridge {
    py_hooks: HashMap<String, PyObject>,
    hook_registry: Arc<HookRegistry>,
    event_bus: Arc<EventBus>,
}

impl PythonExtensionBridge {
    /// Create a new Python extension bridge
    pub fn new(hook_registry: Arc<HookRegistry>, event_bus: Arc<EventBus>) -> Self {
        Self {
            py_hooks: HashMap::new(),
            hook_registry,
            event_bus,
        }
    }
    
    /// Load a Python extension module
    pub fn load_extension(&mut self, module_path: &str) -> Result<(), Error> {
        Python::with_gil(|py| {
            let extension_module = PyModule::import(py, module_path)?;
            
            // Check extension compatibility
            let version = extension_module.getattr("__version__")?.extract::<String>()?;
            self.check_compatibility(&version)?;
            
            // Register transaction hooks if implemented by the module
            if let Ok(hook_class) = extension_module.getattr("TransactionHook") {
                let instance = hook_class.call0()?;
                self.py_hooks.insert("transaction".to_string(), instance.into());
                
                // Create a Rust wrapper for the Python hook and register it
                let py_transaction_hook = PyTransactionHook {
                    py_hook: instance.into(),
                };
                self.hook_registry.register_transaction_hook(Box::new(py_transaction_hook));
            }
            
            // Register account hooks if implemented by the module
            if let Ok(hook_class) = extension_module.getattr("AccountHook") {
                let instance = hook_class.call0()?;
                self.py_hooks.insert("account".to_string(), instance.into());
                
                // Create a Rust wrapper for the Python hook and register it
                let py_account_hook = PyAccountHook {
                    py_hook: instance.into(),
                };
                self.hook_registry.register_account_hook(Box::new(py_account_hook));
            }
            
            // Register event listeners if implemented by the module
            if let Ok(listener_class) = extension_module.getattr("EventListener") {
                let instance = listener_class.call0()?;
                self.py_hooks.insert("event_listener".to_string(), instance.into());
                
                // Create a Rust wrapper for the Python event listener and register it
                let py_event_listener = PyEventListener {
                    py_listener: instance.into(),
                };
                self.event_bus.register_listener(Box::new(py_event_listener));
            }
            
            Ok(())
        })
    }
    
    /// Check if the extension is compatible with the current application version
    fn check_compatibility(&self, extension_version: &str) -> Result<(), Error> {
        // Implementation of version compatibility check
        Ok(())
    }
}

/// Python transaction hook wrapper
struct PyTransactionHook {
    py_hook: PyObject,
}

impl Hook for PyTransactionHook {
    fn id(&self) -> &str {
        "python_transaction_hook"
    }
    
    fn hook_type(&self) -> &str {
        "TransactionHook"
    }
    
    fn is_enabled(&self) -> bool {
        true
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        // Implementation for enabling/disabling
    }
}

impl TransactionHook for PyTransactionHook {
    async fn before_create(&self, transaction: &mut NewTransaction) -> Result<(), Error> {
        Python::with_gil(|py| {
            // Convert transaction to Python object
            let py_transaction = transaction_to_py(py, transaction)?;
            
            // Call the Python hook
            let result = self.py_hook.call_method(py, "before_create", (py_transaction,), None)?;
            
            // Apply any changes from Python back to the transaction
            update_transaction_from_py(transaction, py_transaction)?;
            
            Ok(())
        })
    }
    
    // Other method implementations...
}
```

### Extension Manager

The Extension Manager provides a central point for loading and managing extensions:

```rust
/// Manages the lifecycle of extensions
pub struct ExtensionManager {
    hook_registry: Arc<HookRegistry>,
    event_bus: Arc<EventBus>,
    python_bridge: PythonExtensionBridge,
    rust_extensions: Vec<Box<dyn Extension>>,
    config: ExtensionConfig,
}

/// Configuration for extensions
pub struct ExtensionConfig {
    extension_dir: PathBuf,
    enabled_extensions: HashSet<String>,
    security_level: SecurityLevel,
}

/// Security level for extensions
pub enum SecurityLevel {
    /// Extensions have minimal capabilities
    Restricted,
    
    /// Extensions have standard capabilities
    Standard,
    
    /// Extensions have full access (dangerous)
    Unrestricted,
}

/// Native Rust extension trait
pub trait Extension: Send + Sync {
    /// Get the unique ID of this extension
    fn id(&self) -> &str;
    
    /// Get the extension version
    fn version(&self) -> &str;
    
    /// Initialize the extension
    fn init(&self, hook_registry: Arc<HookRegistry>, event_bus: Arc<EventBus>) -> Result<(), Error>;
    
    /// Shutdown the extension
    fn shutdown(&self) -> Result<(), Error>;
}

impl ExtensionManager {
    /// Create a new extension manager
    pub fn new(config: ExtensionConfig) -> Self {
        let hook_registry = Arc::new(HookRegistry::new());
        let event_bus = Arc::new(EventBus::new());
        
        Self {
            hook_registry: Arc::clone(&hook_registry),
            event_bus: Arc::clone(&event_bus),
            python_bridge: PythonExtensionBridge::new(
                Arc::clone(&hook_registry),
                Arc::clone(&event_bus)
            ),
            rust_extensions: Vec::new(),
            config,
        }
    }
    
    /// Load all enabled extensions
    pub fn load_all(&mut self) -> Result<(), Error> {
        // Load Rust extensions
        self.load_rust_extensions()?;
        
        // Load Python extensions
        self.load_python_extensions()?;
        
        Ok(())
    }
    
    /// Load Rust extensions
    fn load_rust_extensions(&mut self) -> Result<(), Error> {
        // Implementation details for loading native Rust extensions
        Ok(())
    }
    
    /// Load Python extensions
    fn load_python_extensions(&mut self) -> Result<(), Error> {
        for entry in fs::read_dir(&self.config.extension_dir)? {
            let path = entry?.path();
            
            if path.is_dir() && path.join("__init__.py").exists() {
                let module_name = path.file_name()
                    .ok_or_else(|| Error::ExtensionError("Invalid module name".to_string()))?
                    .to_string_lossy()
                    .to_string();
                
                if self.config.enabled_extensions.contains(&module_name) {
                    self.python_bridge.load_extension(&module_name)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Get the hook registry
    pub fn hook_registry(&self) -> Arc<HookRegistry> {
        Arc::clone(&self.hook_registry)
    }
    
    /// Get the event bus
    pub fn event_bus(&self) -> Arc<EventBus> {
        Arc::clone(&self.event_bus)
    }
}
```

## Interfaces

### Extension System API

```rust
/// Extension system service API
pub trait ExtensionService {
    /// Load an extension by ID
    fn load_extension(&mut self, id: &str) -> Result<(), Error>;
    
    /// Unload an extension by ID
    fn unload_extension(&mut self, id: &str) -> Result<(), Error>;
    
    /// List all loaded extensions
    fn list_extensions(&self) -> Vec<ExtensionInfo>;
    
    /// Enable an extension
    fn enable_extension(&mut self, id: &str) -> Result<(), Error>;
    
    /// Disable an extension
    fn disable_extension(&mut self, id: &str) -> Result<(), Error>;
    
    /// Get hook registry
    fn hook_registry(&self) -> Arc<HookRegistry>;
    
    /// Get event bus
    fn event_bus(&self) -> Arc<EventBus>;
}

/// Information about an extension
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub extension_type: ExtensionType,
}

/// Type of extension
pub enum ExtensionType {
    Rust,
    Python,
}
```

## Dependencies
- **PyO3**: For Python integration
- **tokio**: For async support in hooks and events
- **Accounting Kernel**: For core accounting types and operations
- **File System Access**: For loading extension modules

## Performance Considerations
- Hook execution should be optimized to minimize performance impact
- Events should be processed asynchronously to avoid blocking
- Python integration should minimize GIL contention
- Extension loading should be lazy where possible

## Error Handling
- Extensions should be isolated to prevent failures from affecting the core system
- All hook errors should be properly logged and reported
- Version incompatibilities should be clearly identified
- Malformed extensions should be gracefully handled

## Testing Approach
- **Unit Testing**: Test hook and event mechanics in isolation
- **Integration Testing**: Test extension loading and execution
- **Mocking**: Use mock hooks and listeners for testing
- **Security Testing**: Verify extension sandboxing works correctly

Example test:

```rust
#[tokio::test]
async fn test_transaction_hook() {
    let mut registry = HookRegistry::new();
    
    // Create a test hook
    struct TestHook;
    
    impl Hook for TestHook {
        fn id(&self) -> &str { "test_hook" }
        fn hook_type(&self) -> &str { "TransactionHook" }
        fn is_enabled(&self) -> bool { true }
        fn set_enabled(&mut self, _: bool) {}
    }
    
    impl TransactionHook for TestHook {
        async fn before_create(&self, transaction: &mut NewTransaction) -> Result<(), Error> {
            // Modify the transaction description
            transaction.description = format!("Modified: {}", transaction.description);
            Ok(())
        }
        
        async fn after_create(&self, _: &Transaction) -> Result<(), Error> {
            Ok(())
        }
        
        async fn before_post(&self, _: &Transaction) -> Result<(), Error> {
            Ok(())
        }
        
        async fn after_post(&self, _: &Transaction) -> Result<(), Error> {
            Ok(())
        }
    }
    
    // Register the hook
    registry.register_transaction_hook(Box::new(TestHook));
    
    // Create a test transaction
    let mut transaction = NewTransaction {
        book_id: 1,
        description: "Test Transaction".to_string(),
        transaction_date: Utc::now().date_naive(),
        // Other fields...
    };
    
    // Run the hooks
    registry.run_before_create_hooks(&mut transaction).await.unwrap();
    
    // Verify the hook modified the transaction
    assert_eq!(transaction.description, "Modified: Test Transaction");
}
```

## Security Considerations
- Extensions run with restricted permissions by default
- File system access from extensions should be limited to specific directories
- Network access from extensions should be controlled
- Database access from extensions should be mediated through the core API
- User should be informed of extension capabilities before enabling
