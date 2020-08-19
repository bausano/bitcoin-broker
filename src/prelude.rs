pub use rust_decimal::Decimal;

/// Bitcoin currency.
pub type Btc = Decimal;

/// How much hard currency we have to pay to get one bitcoin.
pub type BtcExchangeRate = Decimal;

/// Hard currency such as dollar.
pub type HardCurr = Decimal;

/// A number type refering to percents.
pub type Percentage = Decimal;
