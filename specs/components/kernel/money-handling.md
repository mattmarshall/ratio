# Component Specification: Money Handling

## Overview
The Money Handling component provides precise financial calculations for the Ratio application, ensuring accurate accounting without floating-point errors. It implements a currency-aware decimal representation system optimized for financial operations.

## Responsibilities
- Implement precise numeric representation for financial values
- Handle multiple currencies with different decimal places
- Provide accurate arithmetic operations (addition, subtraction, multiplication, division)
- Implement proper rounding according to financial standards
- Enable currency conversion capabilities
- Ensure immutability and thread-safety for financial calculations

## Design
The Money Handling component follows a value-object pattern with immutable data structures and pure functions for transformations.

### Key Abstractions

#### Currency
The Currency type encapsulates information about a specific currency:

```rust
/// Currency definition with precision information
pub struct Currency {
    pub code: String,        // ISO 4217 code (e.g., "USD")
    pub name: String,        // Human-readable name (e.g., "US Dollar")
    pub symbol: String,      // Currency symbol (e.g., "$")
    pub decimal_places: u8,  // Number of decimal places (e.g., 2 for USD)
    pub rounding_method: RoundingMethod, // Default rounding method
}

/// Rounding methods for financial calculations
pub enum RoundingMethod {
    RoundHalfUp,    // Round up if the fraction is 0.5 or greater
    RoundHalfDown,  // Round down if the fraction is less than 0.5
    RoundDown,      // Always round down (truncate)
    RoundUp,        // Always round up
    Bankers,        // Round to nearest even number (common in finance)
}

impl Currency {
    /// Create a new currency instance
    pub fn new(code: String, name: String, symbol: String, decimal_places: u8, rounding_method: RoundingMethod) -> Self {
        Self { code, name, symbol, decimal_places, rounding_method }
    }
    
    /// Get standard currencies
    pub fn usd() -> Self {
        Self::new("USD".to_string(), "US Dollar".to_string(), "$".to_string(), 2, RoundingMethod::RoundHalfUp)
    }
    
    /// More standard currency constructors...
}
```

#### Money
The Money type represents a financial value with currency information:

```rust
/// Money value with currency association
pub struct Money {
    amount: i64,            // Scaled integer amount
    currency: Rc<Currency>, // Reference to currency definition
}

impl Money {
    /// Create a new money value with the specified amount and currency
    pub fn new(amount: i64, currency: Rc<Currency>) -> Self {
        Self { amount, currency }
    }
    
    /// Create from a decimal value
    pub fn from_decimal(value: Decimal, currency: Rc<Currency>) -> Self {
        let scale = 10_i64.pow(currency.decimal_places as u32);
        let amount = (value * Decimal::from(scale)).round().to_i64().unwrap_or(0);
        Self { amount, currency }
    }
    
    /// Get the actual decimal value
    pub fn decimal_value(&self) -> Decimal {
        let scale = 10_i64.pow(self.currency.decimal_places as u32);
        Decimal::from(self.amount) / Decimal::from(scale)
    }
    
    /// Format the money value as a string
    pub fn format(&self) -> String {
        let value = self.decimal_value();
        format!("{}{}", self.currency.symbol, value)
    }
    
    /// Add another money value (must be same currency)
    pub fn add(&self, other: &Money) -> Result<Money, Error> {
        if !Rc::ptr_eq(&self.currency, &other.currency) {
            return Err(Error::CurrencyMismatch);
        }
        Ok(Money::new(self.amount + other.amount, Rc::clone(&self.currency)))
    }
    
    /// Subtract another money value (must be same currency)
    pub fn subtract(&self, other: &Money) -> Result<Money, Error> {
        if !Rc::ptr_eq(&self.currency, &other.currency) {
            return Err(Error::CurrencyMismatch);
        }
        Ok(Money::new(self.amount - other.amount, Rc::clone(&self.currency)))
    }
    
    /// Multiply by a decimal factor
    pub fn multiply(&self, factor: Decimal) -> Money {
        let result = Decimal::from(self.amount) * factor;
        let rounded = self.round(result);
        Money::new(rounded, Rc::clone(&self.currency))
    }
    
    /// Divide by a decimal factor
    pub fn divide(&self, divisor: Decimal) -> Result<Money, Error> {
        if divisor.is_zero() {
            return Err(Error::DivisionByZero);
        }
        
        let result = Decimal::from(self.amount) / divisor;
        let rounded = self.round(result);
        Ok(Money::new(rounded, Rc::clone(&self.currency)))
    }
    
    /// Negate the money value
    pub fn negate(&self) -> Money {
        Money::new(-self.amount, Rc::clone(&self.currency))
    }
    
    /// Round according to the currency's rounding method
    fn round(&self, value: Decimal) -> i64 {
        match self.currency.rounding_method {
            RoundingMethod::RoundHalfUp => value.round().to_i64().unwrap_or(0),
            RoundingMethod::RoundHalfDown => {
                // Implementation details for half-down rounding
                value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                    .to_i64().unwrap_or(0)
            },
            RoundingMethod::RoundDown => value.floor().to_i64().unwrap_or(0),
            RoundingMethod::RoundUp => value.ceil().to_i64().unwrap_or(0),
            RoundingMethod::Bankers => {
                // Implementation details for banker's rounding
                value.round_dp_with_strategy(0, RoundingStrategy::MidpointTowardZero)
                    .to_i64().unwrap_or(0)
            }
        }
    }
    
    /// Check if money value is zero
    pub fn is_zero(&self) -> bool {
        self.amount == 0
    }
    
    /// Check if money value is positive
    pub fn is_positive(&self) -> bool {
        self.amount > 0
    }
    
    /// Check if money value is negative
    pub fn is_negative(&self) -> bool {
        self.amount < 0
    }
}
```

#### Money Error Handling

```rust
/// Errors related to money operations
pub enum MoneyError {
    /// Attempted operation on different currencies
    CurrencyMismatch,
    
    /// Division by zero
    DivisionByZero,
    
    /// Overflow during calculation
    Overflow,
    
    /// Invalid decimal places (e.g., negative value)
    InvalidDecimalPlaces,
    
    /// Invalid currency code
    InvalidCurrencyCode,
}
```

### CurrencyRegistry

A central registry for managing available currencies:

```rust
/// Registry for currency definitions
pub struct CurrencyRegistry {
    currencies: HashMap<String, Rc<Currency>>,
}

impl CurrencyRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        let mut registry = Self {
            currencies: HashMap::new(),
        };
        
        // Register standard currencies
        registry.register(Currency::usd());
        registry.register(Currency::eur());
        registry.register(Currency::gbp());
        registry.register(Currency::jpy());
        
        registry
    }
    
    /// Register a new currency
    pub fn register(&mut self, currency: Currency) -> Rc<Currency> {
        let currency_rc = Rc::new(currency);
        self.currencies.insert(currency_rc.code.clone(), Rc::clone(&currency_rc));
        currency_rc
    }
    
    /// Get a currency by code
    pub fn get(&self, code: &str) -> Option<Rc<Currency>> {
        self.currencies.get(code).map(Rc::clone)
    }
    
    /// List all registered currencies
    pub fn list(&self) -> Vec<Rc<Currency>> {
        self.currencies.values().map(Rc::clone).collect()
    }
}
```

### CurrencyConverter

Handles currency conversion operations:

```rust
/// Exchange rate between two currencies
pub struct ExchangeRate {
    from_currency: Rc<Currency>,
    to_currency: Rc<Currency>,
    rate: Decimal,
    timestamp: DateTime<Utc>,
}

/// Currency conversion service
pub struct CurrencyConverter {
    rates: HashMap<(String, String), ExchangeRate>,
}

impl CurrencyConverter {
    /// Create a new converter
    pub fn new() -> Self {
        Self {
            rates: HashMap::new(),
        }
    }
    
    /// Update an exchange rate
    pub fn update_rate(&mut self, from: Rc<Currency>, to: Rc<Currency>, rate: Decimal) {
        let exchange_rate = ExchangeRate {
            from_currency: Rc::clone(&from),
            to_currency: Rc::clone(&to),
            rate,
            timestamp: Utc::now(),
        };
        
        self.rates.insert((from.code.clone(), to.code.clone()), exchange_rate);
    }
    
    /// Convert money from one currency to another
    pub fn convert(&self, money: &Money, to_currency: Rc<Currency>) -> Result<Money, MoneyError> {
        if Rc::ptr_eq(&money.currency, &to_currency) {
            return Ok(Money::new(money.amount, Rc::clone(&to_currency)));
        }
        
        let key = (money.currency.code.clone(), to_currency.code.clone());
        
        if let Some(rate) = self.rates.get(&key) {
            let decimal_value = money.decimal_value() * rate.rate;
            return Ok(Money::from_decimal(decimal_value, Rc::clone(&to_currency)));
        }
        
        // Try reverse rate
        let reverse_key = (to_currency.code.clone(), money.currency.code.clone());
        
        if let Some(rate) = self.rates.get(&reverse_key) {
            let decimal_value = money.decimal_value() / rate.rate;
            return Ok(Money::from_decimal(decimal_value, Rc::clone(&to_currency)));
        }
        
        Err(MoneyError::CurrencyMismatch)
    }
}
```

## Interfaces

### Money Service API

```rust
/// Service for money-related operations
pub trait MoneyService {
    /// Create money from a decimal value
    fn create_money(&self, value: Decimal, currency_code: &str) -> Result<Money, MoneyError>;
    
    /// Convert money to a different currency
    fn convert(&self, money: &Money, to_currency_code: &str) -> Result<Money, MoneyError>;
    
    /// Get a currency by code
    fn get_currency(&self, code: &str) -> Option<Rc<Currency>>;
    
    /// List available currencies
    fn list_currencies(&self) -> Vec<Rc<Currency>>;
    
    /// Update an exchange rate
    fn update_exchange_rate(&mut self, from_code: &str, to_code: &str, rate: Decimal) -> Result<(), MoneyError>;
}
```

## Dependencies
- **rust_decimal**: For precise decimal arithmetic
- **chrono**: For timestamps on exchange rates
- **Database Layer**: For persistence of currencies and exchange rates

## Performance Considerations
- Currency instances should be cached and shared via `Rc` to minimize memory usage
- Exchange rate lookups should be optimized with a fast lookup structure
- Money arithmetic operations should be optimized for performance
- Large batch operations should avoid unnecessary allocations

## Error Handling
- All operations return Result types for proper error handling
- Currency mismatches are explicitly checked and reported
- Mathematical errors like division by zero are properly handled
- Overflows are detected and reported

## Testing Approach
- **Unit Testing**: Test all money operations with different currencies and values
- **Property-Based Testing**: Verify mathematical properties (associativity, commutativity)
- **Edge Cases**: Test boundary conditions (zero, very large numbers, negative numbers)
- **Rounding Behavior**: Verify different rounding strategies produce expected results

Example test:

```rust
#[test]
fn test_money_addition() {
    let currency = Rc::new(Currency::usd());
    let money1 = Money::new(100, Rc::clone(&currency));  // $1.00
    let money2 = Money::new(250, Rc::clone(&currency));  // $2.50
    
    let result = money1.add(&money2).unwrap();
    assert_eq!(result.amount, 350);  // $3.50
}

#[test]
fn test_currency_mismatch() {
    let usd = Rc::new(Currency::usd());
    let eur = Rc::new(Currency::eur());
    
    let money_usd = Money::new(100, Rc::clone(&usd));  // $1.00
    let money_eur = Money::new(100, Rc::clone(&eur));  // €1.00
    
    assert!(matches!(money_usd.add(&money_eur), Err(MoneyError::CurrencyMismatch)));
}
```

## Security Considerations
- Prevent manipulation of exchange rates without proper authorization
- Validate all inputs to prevent overflow or underflow attacks
- Ensure thread-safety for concurrent operations
- Log all currency operations for audit purposes
