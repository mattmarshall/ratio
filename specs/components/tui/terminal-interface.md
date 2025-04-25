# Component Specification: Terminal User Interface

## Overview
The Terminal User Interface (TUI) component provides a responsive, keyboard-driven interface for Ratio in terminal environments. It offers an efficient way to interact with the accounting system, optimized for power users who prefer keyboard-centric workflows and fast navigation without sacrificing visual clarity or functionality.

## Responsibilities
- Provide a complete user interface for all Ratio functionality in a terminal environment
- Implement responsive, keyboard-driven navigation and data entry
- Display financial data in a clear, readable format
- Support both simple views for beginners and advanced views for power users
- Ensure high performance even when displaying large datasets
- Support cross-platform terminal compatibility (Linux, macOS, Windows)
- Facilitate efficient financial data entry and management workflows
- Support both mouse and keyboard interaction where appropriate

## Design
The TUI is built using a Rust-based terminal UI framework (either tui-rs or cursive) with a component-based architecture.

### Architecture Pattern
- **Model-View-Controller (MVC)**: Separation of data, presentation, and control logic
- **Component-Based**: Modular, reusable UI components
- **Event-Driven**: UI updates in response to user input and system events
- **State Management**: Centralized state management with local component state where appropriate

### Framework Selection

#### Option 1: tui-rs with crossterm
```rust
// Advantages:
// - Low-level control for custom widgets
// - High performance for large datasets
// - Flexible layout system
// - Good community support

// Disadvantages:
// - More boilerplate code required
// - Limited built-in widgets
```

#### Option 2: cursive
```rust
// Advantages:
// - Higher-level abstraction
// - Rich set of pre-built widgets
// - Built-in theming support
// - Dialog and form utilities

// Disadvantages:
// - Less control over rendering details
// - Potentially lower performance for complex views
```

Decision: **tui-rs with crossterm** will be used for its performance, flexibility, and lower-level control that better suits our custom financial interface needs.

### Key Components

#### Application Structure
```rust
/// The main application structure that manages the UI
pub struct App {
    /// Application state
    pub state: AppState,
    
    /// UI tabs
    pub tabs: Tabs,
    
    /// Active API clients
    pub clients: ApiClients,
    
    /// User preferences
    pub preferences: Preferences,
    
    /// Input mode (normal, insert)
    pub input_mode: InputMode,
    
    /// Command history
    pub command_history: Vec<String>,
    
    /// Current status message
    pub status: Option<StatusMessage>,
}

/// The application state
pub struct AppState {
    /// Currently selected book
    pub current_book: Option<Book>,
    
    /// Currently active view
    pub active_view: ActiveView,
    
    /// Currently selected account
    pub selected_account: Option<Account>,
    
    /// Currently selected transaction
    pub selected_transaction: Option<Transaction>,
    
    /// Search filters
    pub filters: Filters,
    
    // Other state...
}

/// The available views in the application
pub enum ActiveView {
    Dashboard,
    Accounts,
    Transactions,
    Scheduled,
    Reports,
    Settings,
}
```

#### UI Layout
```rust
fn render(&mut self, frame: &mut Frame) {
    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // Header
            Constraint::Min(0),      // Main content
            Constraint::Length(1),   // Status line
        ])
        .split(frame.size());
        
    // Render the header
    self.render_header(frame, chunks[0]);
    
    // Render the main content based on the active view
    match self.state.active_view {
        ActiveView::Dashboard => self.render_dashboard(frame, chunks[1]),
        ActiveView::Accounts => self.render_accounts(frame, chunks[1]),
        ActiveView::Transactions => self.render_transactions(frame, chunks[1]),
        ActiveView::Scheduled => self.render_scheduled(frame, chunks[1]),
        ActiveView::Reports => self.render_reports(frame, chunks[1]),
        ActiveView::Settings => self.render_settings(frame, chunks[1]),
    }
    
    // Render the status line
    self.render_status_line(frame, chunks[2]);
}
```

#### Custom Widgets
The TUI will include several custom widgets tailored for financial data:

```rust
/// Widget for displaying account lists with balances
pub struct AccountList<'a> {
    accounts: &'a [Account],
    balances: &'a HashMap<i64, Decimal>,
    selected_index: Option<usize>,
    highlight_style: Style,
}

/// Widget for displaying transaction registers
pub struct TransactionRegister<'a> {
    transactions: &'a [Transaction],
    selected_index: Option<usize>,
    show_splits: bool,
    highlight_style: Style,
}

/// Widget for displaying financial charts
pub struct FinancialChart<'a> {
    data_points: &'a [DataPoint],
    chart_type: ChartType,
    title: &'a str,
    x_axis_title: &'a str,
    y_axis_title: &'a str,
}

/// Widget for displaying a double-entry transaction form
pub struct TransactionForm<'a> {
    transaction: &'a mut Transaction,
    accounts: &'a [Account],
    focused_field: Field,
    validation_errors: HashMap<Field, String>,
}
```

### Screen Layouts

#### Dashboard Layout
The dashboard provides an overview of the user's financial situation:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Ratio Financial Manager                                      User: jdoe     │
│ [Dashboard] Accounts Transactions Scheduled Reports Settings                │
├─────────────────────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐ ┌─────────────────────────────────────────┐ │
│ │ Net Worth: $52,500.00       │ │ Upcoming Scheduled Transactions         │ │
│ │                             │ │ 04/26: Salary           +$2,000.00      │ │
│ │ Assets:     $85,000.00      │ │ 04/28: Rent             -$1,200.00      │ │
│ │ Liabilities: $32,500.00     │ │ 05/01: Car Payment      -$350.00        │ │
│ │                             │ │ 05/03: Electric Bill    -$85.00         │ │
│ └─────────────────────────────┘ └─────────────────────────────────────────┘ │
│ ┌─────────────────────────────┐ ┌─────────────────────────────────────────┐ │
│ │ Account Balances            │ │ Monthly Spending by Category            │ │
│ │ Checking:     $2,500.00     │ │ ████████████ Housing      $1,500.00     │ │
│ │ Savings:      $7,500.00     │ │ ████████     Food         $600.00       │ │
│ │ Investments: $75,000.00     │ │ ███          Transport    $350.00       │ │
│ │ Credit Card: -$1,200.00     │ │ ██           Utilities    $200.00       │ │
│ │ Mortgage:   -$31,300.00     │ │ █            Other        $100.00       │ │
│ └─────────────────────────────┘ └─────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────────────────┤
│ Press ? for help                                                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Transaction Register Layout
The transaction register provides a view of all transactions for an account:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Ratio Financial Manager                                      User: jdoe     │
│ Dashboard Accounts [Transactions] Scheduled Reports Settings                │
├─────────────────────────────────────────────────────────────────────────────┤
│ Account: Checking                                      Balance: $2,500.00   │
│ ┌───────┬─────────────┬──────────────────────────┬──────────┬─────────────┐ │
│ │ Date  │ Description │ Category                 │ Amount   │ Balance     │ │
│ ├───────┼─────────────┼──────────────────────────┼──────────┼─────────────┤ │
│ │ 04/20 │ Grocery     │ Expenses:Food:Groceries  │ -$85.50  │ $2,500.00   │ │
│ │ 04/19 │ Gas         │ Expenses:Transport:Fuel  │ -$45.00  │ $2,585.50   │ │
│ │ 04/15 │ Salary      │ Income:Salary            │ +$2,000.00│ $2,630.50  │ │
│ │ 04/10 │ Restaurant  │ Expenses:Food:Dining     │ -$65.25  │ $630.50     │ │
│ │ 04/05 │ Transfer    │ [Transfer to Savings]    │ -$500.00 │ $695.75     │ │
│ │ 04/01 │ Rent        │ Expenses:Housing:Rent    │ -$1,200.00│ $1,195.75  │ │
│ │ 03/30 │ Utilities   │ Expenses:Housing:Utilities│ -$95.00 │ $2,395.75   │ │
│ │ 03/28 │ Paycheck    │ Income:Salary            │ +$2,000.00│ $2,490.75  │ │
│ │       │             │                          │          │             │ │
│ │       │             │                          │          │             │ │
│ │       │             │                          │          │             │ │
│ │       │             │                          │          │             │ │
│ └───────┴─────────────┴──────────────────────────┴──────────┴─────────────┘ │
│ [n]ew transaction  [e]dit  [d]elete  [s]earch  [f]ilter  [r]econcile        │
├─────────────────────────────────────────────────────────────────────────────┤
│ Transaction: Grocery - From Checking to Expenses:Food:Groceries             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Transaction Entry Form
The transaction entry form allows for creating or editing transactions:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Ratio Financial Manager                                      User: jdoe     │
│ Dashboard Accounts [Transactions] Scheduled Reports Settings                │
├─────────────────────────────────────────────────────────────────────────────┤
│                          New Transaction                                    │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │ Date: [04/24/2025________]  Status: (●) Pending ( ) Posted              │ │
│ │                                                                         │ │
│ │ Description: [Grocery shopping_______________________________________]  │ │
│ │                                                                         │ │
│ │ Reference/Check #: [____________]                                       │ │
│ │                                                                         │ │
│ │ Splits:                                                                 │ │
│ │ ┌─────────┬──────────────────────────┬──────────┬───────┬────────────┐ │ │
│ │ │ Account │ Category                 │ Amount   │ D/C   │ Memo       │ │ │
│ │ ├─────────┼──────────────────────────┼──────────┼───────┼────────────┤ │ │
│ │ │ Checking│                          │ $85.50   │ Credit│            │ │ │
│ │ │[Expenses│ Food:Groceries           │ $85.50   │ Debit │ Weekly shop│>│ │
│ │ │         │                          │          │       │            │ │ │
│ │ │  [+]    │                          │          │       │            │ │ │
│ │ └─────────┴──────────────────────────┴──────────┴───────┴────────────┘ │ │
│ │                                                                         │ │
│ │ [Save Transaction]          [Cancel]                                    │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────────────────────┤
│ Tab: Next field  Shift+Tab: Previous field  Ctrl+S: Save  Esc: Cancel       │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Keyboard Navigation
The TUI will support an extensive set of keyboard shortcuts for efficient navigation and operation:

```rust
fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), Error> {
    match (self.input_mode, key_event.code) {
        // Global shortcuts
        (_, KeyCode::F(1)) => self.show_help(),
        (_, KeyCode::Char('q')) if key_event.modifiers.contains(KeyModifiers::CONTROL) => self.quit(),
        (_, KeyCode::Char('s')) if key_event.modifiers.contains(KeyModifiers::CONTROL) => self.save_current_item(),
        
        // Normal mode navigation
        (InputMode::Normal, KeyCode::Char('h')) => self.navigate_left(),
        (InputMode::Normal, KeyCode::Char('j')) => self.navigate_down(),
        (InputMode::Normal, KeyCode::Char('k')) => self.navigate_up(),
        (InputMode::Normal, KeyCode::Char('l')) => self.navigate_right(),
        (InputMode::Normal, KeyCode::Tab) => self.next_tab(),
        (InputMode::Normal, KeyCode::BackTab) => self.previous_tab(),
        
        // View-specific shortcuts in normal mode
        (InputMode::Normal, KeyCode::Char('n')) => self.new_item(),
        (InputMode::Normal, KeyCode::Char('e')) => self.edit_selected_item(),
        (InputMode::Normal, KeyCode::Char('d')) => self.delete_selected_item(),
        (InputMode::Normal, KeyCode::Char('f')) => self.toggle_filter_panel(),
        (InputMode::Normal, KeyCode::Char('/')) => self.start_search(),
        
        // Insert mode for text inputs
        (InputMode::Insert, KeyCode::Enter) => self.confirm_input(),
        (InputMode::Insert, KeyCode::Esc) => self.cancel_input(),
        (InputMode::Insert, KeyCode::Tab) => self.next_field(),
        (InputMode::Insert, KeyCode::BackTab) => self.previous_field(),
        (InputMode::Insert, KeyCode::Char(c)) => self.add_char_to_input(c),
        (InputMode::Insert, KeyCode::Backspace) => self.delete_char_from_input(),
        
        // Form navigation
        (InputMode::Form, KeyCode::Tab) => self.next_form_field(),
        (InputMode::Form, KeyCode::BackTab) => self.previous_form_field(),
        (InputMode::Form, KeyCode::Enter) => self.submit_form(),
        (InputMode::Form, KeyCode::Esc) => self.cancel_form(),
        
        // Default case - ignore or handle as appropriate
        _ => Ok(()),
    }
}
```

### Theming System
The TUI will support a theming system to customize the appearance:

```rust
/// Application theme
pub struct Theme {
    /// Base color palette
    pub colors: ColorPalette,
    
    /// Text styles
    pub styles: StylePalette,
    
    /// Layout settings
    pub layout: LayoutSettings,
}

/// Color palette for the theme
pub struct ColorPalette {
    pub background: Color,
    pub foreground: Color,
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub success: Color,
}

/// Style palette for the theme
pub struct StylePalette {
    pub normal: Style,
    pub header: Style,
    pub selected: Style,
    pub highlighted: Style,
    pub error: Style,
    pub success: Style,
    pub label: Style,
    pub value: Style,
}
```

### Data Visualization
The TUI will include several types of data visualization for financial reports:

```rust
/// Chart types for financial visualization
pub enum ChartType {
    /// Bar chart for category comparison
    BarChart,
    
    /// Line chart for trends over time
    LineChart,
    
    /// Pie chart for composition analysis
    PieChart,
    
    /// Sparkline for compact trend display
    Sparkline,
}

/// Data point for charts
pub struct DataPoint {
    pub label: String,
    pub value: Decimal,
    pub date: Option<NaiveDate>,
    pub color: Option<Color>,
}

impl FinancialChart<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        match self.chart_type {
            ChartType::BarChart => self.render_bar_chart(area, buf),
            ChartType::LineChart => self.render_line_chart(area, buf),
            ChartType::PieChart => self.render_pie_chart(area, buf),
            ChartType::Sparkline => self.render_sparkline(area, buf),
        }
    }
    
    // Implementation methods for each chart type...
}
```

## Dependencies
- **tui-rs**: For the terminal UI framework
- **crossterm**: For terminal manipulation and event handling
- **chrono**: For date and time handling
- **rust_decimal**: For precise decimal arithmetic
- **API Client**: For communication with the accounting kernel

## Performance Considerations
- **Efficient Rendering**: Minimize full screen redraws, use targeted updates
- **Pagination**: For large datasets, implement pagination to avoid loading everything
- **Async Loading**: Use background loading for data retrieval
- **Caching**: Cache frequently accessed data
- **Throttling**: Throttle rapid UI updates
- **Memory Management**: Be careful with large data structures

## Error Handling
The TUI will implement a comprehensive error handling strategy:

```rust
/// UI-specific error types
pub enum UiError {
    /// API client errors
    ApiError(ApiError),
    
    /// Terminal I/O errors
    IoError(std::io::Error),
    
    /// Validation errors for forms
    ValidationError(HashMap<String, String>),
    
    /// Layout errors
    LayoutError(String),
    
    /// Navigation errors
    NavigationError(String),
}

impl App {
    /// Handle and display errors in the UI
    fn handle_error(&mut self, error: UiError) {
        match error {
            UiError::ApiError(api_error) => {
                self.status = Some(StatusMessage::error(
                    format!("API Error: {}", api_error),
                    Duration::from_secs(5),
                ));
            },
            UiError::ValidationError(validation_errors) => {
                let message = validation_errors
                    .values()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                    
                self.status = Some(StatusMessage::error(
                    format!("Validation Error: {}", message),
                    Duration::from_secs(5),
                ));
                
                // Also update the form with error markers
                if let Some(form) = &mut self.current_form {
                    form.set_validation_errors(validation_errors);
                }
            },
            // Handle other error types...
        }
    }
}
```

## Testing Approach
The TUI component will be tested using:

- **Unit Tests**: For individual widgets and components
- **Integration Tests**: For screen flows and navigation
- **Mock API**: To test UI behavior without a real backend
- **Snapshot Testing**: To catch unexpected UI changes
- **Headless Testing**: For automated testing in CI environments

Example test:

```rust
#[test]
fn test_transaction_form_validation() {
    let mut app = App::new_with_mock_api();
    
    // Navigate to transactions and create a new transaction
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)).unwrap();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)).unwrap();
    
    // Try to save an empty form
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)).unwrap();
    
    // Check that validation errors were displayed
    assert!(app.status.is_some());
    assert!(app.status.as_ref().unwrap().is_error());
    
    // Check that the form shows validation errors
    let form = app.current_form.as_ref().unwrap();
    assert!(!form.validation_errors.is_empty());
    
    // Fill in required fields
    app.set_form_field("description", "Test Transaction");
    app.add_split("Checking", "Expenses:Food", "50.00", "Credit", "Debit");
    
    // Try to save again
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)).unwrap();
    
    // Check that the form was saved without errors
    assert!(app.current_form.is_none());
    assert!(app.status.is_some());
    assert!(app.status.as_ref().unwrap().is_success());
}
```

## Accessibility Considerations
- **Color Contrast**: Ensure sufficient contrast for readability
- **Screen Reader Support**: Where possible, include hints for screen readers
- **Keyboard Navigation**: All features must be accessible via keyboard
- **Customizable Colors**: Support for high contrast themes
- **Font Size Options**: For users with visual impairments

## Future Enhancements
- **Mouse Support**: Add optional mouse interaction for all operations
- **Touch Support**: For terminal emulators on touch devices
- **Advanced Filters**: More sophisticated filtering and sorting
- **Custom Reports**: User-defined report layouts
- **Macro Recording**: For automating repetitive tasks
- **Split Panes**: For viewing multiple datasets simultaneously
