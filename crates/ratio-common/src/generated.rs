#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Currency {
    Usd,
    Eur,
    Gbp,
    Jpy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundingMethod {
    RoundHalfUp,
    RoundHalfDown,
    RoundDown,
    RoundUp,
    Bankers,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Money {
    pub minor: i64,
    pub currency: Currency,
}
pub fn money_zero(currency: Currency) -> Money {
    Money {
        minor: 0,
        currency: currency,
    }
}
pub fn money_add(a: Money, b: Money) -> Money {
    Money {
        minor: a.minor + b.minor,
        currency: a.currency,
    }
}
pub fn money_neg(m: Money) -> Money {
    Money {
        minor: 0 - m.minor,
        currency: m.currency,
    }
}
pub fn money_is_zero(m: Money) -> bool {
    m.minor == 0
}
