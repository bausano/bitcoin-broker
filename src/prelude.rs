pub use rust_decimal::Decimal;

use std::{borrow::Cow, fmt};

/// Bitcoin currency.
pub type Btc = Decimal;

/// How much hard currency we have to pay to get one bitcoin.
pub type BtcExchangeRate = Decimal;

/// Hard currency such as dollar.
pub type Cash = Decimal;

/// A number type refering to percents.
pub type Percentage = Decimal;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
pub struct Error(Cow<'static, str>);
impl Error {
    pub fn outdated_message() -> Self {
        Self(Cow::Borrowed("Received an outdated message"))
    }
}
impl std::error::Error for Error {}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
