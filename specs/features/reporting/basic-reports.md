# Basic Reports Feature Specification

## Overview
This document outlines the basic reporting capabilities for Ratio, providing users with essential financial insights through standardized reports. These reports build on the double-entry bookkeeping system to present accurate financial statements and help users understand their financial position and performance.

## Goals
- Implement essential financial reporting capabilities
- Provide accurate balance sheet and income statement reports
- Enable filtering and customization of report parameters
- Support report exports in various formats
- Create visually clear presentation of financial data
- Provide consistent reporting capabilities across the application

## User Stories

### Financial Reporting Stories
1. As a user, I want to generate a balance sheet so that I can see my financial position at a point in time
2. As a user, I want to create an income statement so that I can track my income and expenses over a period
3. As a user, I want to compare reports across different time periods so that I can identify trends
4. As a user, I want to filter reports by account or category so that I can focus on specific areas
5. As a user, I want to export reports in different formats so that I can use them in other applications

### Reporting Customization Stories
1. As a user, I want to select custom date ranges for reports so that I can analyze specific periods
2. As a user, I want to choose the level of detail in my reports so that I can view summary or detailed information
3. As a user, I want to save report configurations so that I can quickly run the same report in the future
4. As a user, I want to set a default currency for reports so that multi-currency data is consistently presented
5. As a user, I want to include or exclude specific accounts so that I can customize my reports

## Feature Requirements

### Core Report Types

#### Balance Sheet 
- Shows financial position at a specific point in time
- Follows the accounting equation: Assets = Liabilities + Equity
- Includes all balance sheet accounts (Assets, Liabilities, Equity)
- Presents accounts in hierarchical structure
- Provides subtotals for account groups
- Supports different date points (current, month-end, year-end)

#### Income Statement (Profit & Loss)
- Shows financial performance over a period of time
- Includes all income and expense accounts
- Calculates net income or loss
- Supports comparison across different periods
- Provides percentage of total calculations
- Allows for monthly or quarterly breakdowns
- Includes year-to-date calculations

### Report Customization

#### Date Range Selection
- Point-in-time selection for balance sheet
- Period selection for income statement
- Support for standard periods (current month, quarter, year)
- Support for custom date ranges
- Fiscal year awareness
- Support for comparative periods (year-over-year, month-over-month)

#### Filtering Options
- Filter by account type
- Filter by account group or individual accounts
- Include/exclude inactive accounts
- Filter by tags or categories
- Filter by currency

#### Appearance Options
- Detail level (summary, detailed, all accounts)
- Account number display options
- Zero balance account display options
- Negative number display format
- Percentage calculation options
- Currency display preferences

### Report Formatting

#### Layout and Structure
- Clear hierarchical account presentation
- Consistent indentation for account levels
- Column alignment for numbers
- Proper decimal alignment
- Header and footer customization
- Report title and description

#### Visual Elements
- Styling for different account levels
- Emphasis for totals and subtotals
- Optional gridlines and borders
- Font and text size options
- Header and report branding

### Report Actions

#### Export Capabilities
- Export to CSV for spreadsheet analysis
- Export to PDF for formal presentation
- Export to JSON for programmatic use
- Print-friendly formatting
- Attachment of reports to emails

#### Saving and Sharing
- Save report configurations
- Schedule recurring report generation
- Share reports with other users (permission controlled)
- Annotate reports with notes and comments
- Compare saved reports

## User Interfaces

### Balance Sheet Report

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Balance Sheet - April 24, 2025                                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ASSETS                                                                  │
│    Current Assets                                                        │
│      Cash and Cash Equivalents                                           │
│        Checking Account                            $    2,450.67         │
│        Savings Account                             $    5,500.00         │
│        Petty Cash                                  $      150.00         │
│      Total Cash and Cash Equivalents               $    8,100.67         │
│                                                                          │
│      Accounts Receivable                           $    1,200.00         │
│      Investments                                   $   10,000.00         │
│    Total Current Assets                            $   19,300.67         │
│                                                                          │
│    Fixed Assets                                                          │
│      Equipment                                     $    2,500.00         │
│      Less: Accumulated Depreciation                $     (500.00)        │
│    Total Fixed Assets                              $    2,000.00         │
│                                                                          │
│  TOTAL ASSETS                                      $   21,300.67         │
│                                                                          │
│  LIABILITIES                                                             │
│    Current Liabilities                                                   │
│      Credit Card                                   $    1,250.75         │
│      Accounts Payable                              $      750.00         │
│    Total Current Liabilities                       $    2,000.75         │
│                                                                          │
│    Long Term Liabilities                                                 │
│      Loan Payable                                  $    5,000.00         │
│    Total Long Term Liabilities                     $    5,000.00         │
│                                                                          │
│  TOTAL LIABILITIES                                 $    7,000.75         │
│                                                                          │
│  EQUITY                                                                  │
│    Opening Balance Equity                          $   10,000.00         │
│    Retained Earnings                               $    2,500.00         │
│    Net Income                                      $    1,799.92         │
│  TOTAL EQUITY                                      $   14,299.92         │
│                                                                          │
│  TOTAL LIABILITIES AND EQUITY                      $   21,300.67         │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Income Statement Report

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Income Statement - January 1, 2025 to April 24, 2025                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  INCOME                                                                  │
│    Salary Income                                 $   12,500.00    71.4%  │
│    Consulting Revenue                            $    3,000.00    17.1%  │
│    Interest Income                               $      350.00     2.0%  │
│    Investment Income                             $    1,650.00     9.4%  │
│  TOTAL INCOME                                    $   17,500.00   100.0%  │
│                                                                          │
│  EXPENSES                                                                │
│    Housing                                                               │
│      Rent                                        $    6,000.00    38.2%  │
│      Utilities                                   $      750.00     4.8%  │
│      Internet                                    $      200.00     1.3%  │
│    Total Housing                                 $    6,950.00    44.3%  │
│                                                                          │
│    Food                                                                  │
│      Groceries                                   $    2,100.00    13.4%  │
│      Dining Out                                  $      850.00     5.4%  │
│    Total Food                                    $    2,950.00    18.8%  │
│                                                                          │
│    Transportation                                                        │
│      Gas                                         $      450.00     2.9%  │
│      Car Insurance                               $      350.00     2.2%  │
│      Car Maintenance                             $      200.00     1.3%  │
│      Public Transit                              $      100.00     0.6%  │
│    Total Transportation                          $    1,100.00     7.0%  │
│                                                                          │
│    Entertainment                                 $      800.00     5.1%  │
│    Healthcare                                    $    1,200.00     7.6%  │
│    Clothing                                      $      700.00     4.5%  │
│    Subscriptions                                 $      300.00     1.9%  │
│    Miscellaneous                                 $    1,700.08    10.8%  │
│  TOTAL EXPENSES                                  $   15,700.08   100.0%  │
│                                                                          │
│  NET INCOME                                      $    1,799.92           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Report Configuration Interface

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Report Configuration                                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Report Type: [Income Statement ▼]                                       │
│                                                                          │
│  Period:                                                                 │
│    ○ Standard: [Year to Date ▼]                                          │
│    ○ Custom:   From [01/01/2025] To [04/24/2025]                        │
│                                                                          │
│  Comparison:                                                             │
│    ☑ Include comparison period                                           │
│    ○ Previous period                                                     │
│    ○ Same period last year                                               │
│    ○ Custom: From [01/01/2024] To [04/24/2024]                          │
│                                                                          │
│  Account Selection:                                                      │
│    ○ All accounts                                                        │
│    ○ Selected accounts...  [Configure]                                   │
│                                                                          │
│  Display Options:                                                        │
│    ☑ Show percentages                                                    │
│    ☑ Show zero balance accounts                                          │
│    ☑ Include inactive accounts                                           │
│    ○ Summary view                                                        │
│    ○ Detailed view                                                       │
│                                                                          │
│  Currency: [USD ▼]                                                       │
│                                                                          │
│  Save Configuration As: [                                            ]   │
│                                                                          │
│  [ Cancel ]    [ Save Configuration ]    [ Generate Report ]             │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Report Viewer Interface

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Report: Income Statement - YTD 2025                                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  [ Export ▼ ]  [ Print ]  [ Share ]  [ Configure ]  [ Save ]             │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                                                                  │    │
│  │  [Report content appears here]                                   │    │
│  │                                                                  │    │
│  │                                                                  │    │
│  │                                                                  │    │
│  │                                                                  │    │
│  │                                                                  │    │
│  │                                                                  │    │
│  │                                                                  │    │
│  │                                                                  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                          │
│  Report Notes:                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                                                                  │    │
│  │ Add notes about this report here...                              │    │
│  │                                                                  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                          │
│  Generated: April 24, 2025 at 2:30 PM                                    │
│  Configuration: YTD Income Statement                                     │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Workflows

### Balance Sheet Generation Workflow

1. User navigates to reports section
2. User selects balance sheet report type
3. User chooses report date (defaults to current date)
4. User configures any filtering or display options
5. User generates the report
6. System retrieves account balances as of the specified date
7. System organizes accounts by type (Assets, Liabilities, Equity)
8. System calculates subtotals and totals
9. System verifies that Assets = Liabilities + Equity
10. System displays formatted balance sheet report
11. User can export, print, or save the report configuration

### Income Statement Generation Workflow

1. User navigates to reports section
2. User selects income statement report type
3. User selects date range for the report
4. User configures any additional options (comparison period, etc.)
5. User generates the report
6. System retrieves all income and expense transactions for the period
7. System groups transactions by account and calculates account balances
8. System calculates subtotals by account groups
9. System calculates total income, total expenses, and net income
10. System computes percentages if enabled
11. System displays formatted income statement
12. User can export, print, or save the report configuration

### Comparative Report Workflow

1. User configures a report (balance sheet or income statement)
2. User enables comparison feature
3. User selects comparison basis (previous period, same period last year, etc.)
4. System generates primary report
5. System generates comparison report using the same structure
6. System calculates variances between periods (absolute and percentage)
7. System displays both reports side by side or in columns
8. System highlights significant variances
9. User can drill down into variances for more details
10. User can save or export the comparative report

### Report Export Workflow

1. User generates a report
2. User selects export option
3. User chooses export format (PDF, CSV, JSON)
4. User configures export options (if applicable)
5. System generates export file in selected format
6. System provides download link or saves to selected location
7. User can share the exported file externally

## Technical Implementation Considerations

### Report Generation Engine
- Implement flexible report definition framework
- Create reusable reporting components
- Support both fixed and dynamic reports
- Implement caching for report data
- Use lazy loading for large reports

### Data Aggregation
- Create efficient query strategies for report generation
- Implement aggregation functions for account hierarchies
- Design optimized database queries for reporting
- Consider materialized views for common reports
- Implement balance aggregation with proper accounting rules

### Performance Considerations
- Cache commonly used reports
- Implement background generation for complex reports
- Optimize database queries with proper indices
- Consider pre-calculation of common metrics
- Use pagination for large reports

### Export Engine
- Implement templating system for different export formats
- Create consistent styling across export formats
- Support proper number formatting in exports
- Ensure date and currency localization in exports
- Handle large exports efficiently

## Business Rules

### Balance Sheet Rules
1. Balance sheet must always balance (Assets = Liabilities + Equity)
2. Accounts are presented based on liquidity (most liquid assets first)
3. Negative amounts are shown in parentheses
4. All balance sheet accounts must be included
5. Totals must be clearly distinguished from individual accounts

### Income Statement Rules
1. Income is shown before expenses
2. Net income equals total income minus total expenses
3. Percentages for income items are based on total income
4. Percentages for expense items can be based on total income or total expenses
5. Year-to-date figures are cumulative from the start of the fiscal year

### Presentation Rules
1. Currency symbols should be consistent throughout a report
2. Account hierarchies should be clearly visible through indentation
3. Account numbers are optional but should be consistent if shown
4. Zero balances may be shown or hidden based on user preference
5. Reporting periods should be clearly identified in the report header

## Testing Requirements

### Unit Testing
- Test calculation of account balances
- Test proper aggregation by account type
- Test correct subtotal and total calculations
- Test accurate percentage calculations
- Test date range filtering

### Integration Testing
- Test interaction with transaction data
- Test report generation with various filters
- Test export functionality in different formats
- Test caching mechanisms
- Test multi-currency report generation

### Validation Testing
- Verify balance sheet equation (Assets = Liabilities + Equity)
- Verify income statement calculations
- Verify comparative report calculations
- Test with edge cases (negative balances, zero totals)
- Validate proper account categorization

## Documentation Requirements

### User Documentation
- Report type descriptions
- Report configuration options guide
- Customization and filtering options
- Export and sharing functionality
- Saved report management

### Developer Documentation
- Report engine architecture
- Adding new report types
- Extending export capabilities
- Performance optimization strategies
- Query design for reports

## Dependencies

- Double-entry bookkeeping system must be implemented
- Account hierarchy and typing system must be in place
- Transaction data model must be complete
- Currency handling must be implemented
- User permission system must be in place to control report access
