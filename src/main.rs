use {
    rust_decimal::Decimal,
    std::{cmp::Ordering, collections::BinaryHeap},
    uuid::Uuid,
};

/// Bitcoin currency.
pub type Btc = Decimal;

/// How much hard currency we have to pay to get one bitcoin.
pub type BtcExchangeRate = Decimal;

/// Hard currency such as dollar.
pub type HardCurr = Decimal;

/// A purchase holds information about transaction history of our buy requests
/// at market.
pub struct Purchase {
    /// The unique id generated when the purchase was made.
    pub id: Uuid,
    /// How much bitcoin have we bought with this purchase.
    pub btc: Btc,
    /// How much have we paid for the exchange. This includes overal price of
    /// the bitcoins and the fees.
    pub buying_price: HardCurr,
    /// The rate at which the bitcoin was exchanged.
    pub rate: BtcExchangeRate,
}

/// The provider will take a cut from the transaction.
pub enum Fee {
    Percentage(Decimal),
}

/// Each Purchase is evaluated primarily based on what was the exchange rate we
/// bought it for. That's why we order the queue by the rate and when we get an
/// update on the current $/BTC trend, we start evaluating purchases from the
/// top.
impl Ord for Purchase {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rate.cmp(&other.rate)
    }
}

impl PartialOrd for Purchase {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Purchase {}
impl PartialEq for Purchase {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub type PurchaseAccount = BinaryHeap<Purchase>;

impl Purchase {
    /// Creates a new purchase from given data.
    pub fn new(btc: Btc, buying_price: HardCurr, rate: BtcExchangeRate) -> Self {
        Self {
            id: Uuid::new_v4(),
            btc,
            buying_price,
            rate,
        }
    }

    /// If we sold the purchase for the current exchange rate trend, and
    /// deducted the provider's cut, how much would we make on the purchase.
    pub fn margin_after_fee(&self, current_trend: BtcExchangeRate, fee: Fee) -> HardCurr {
        let margin = self.margin(current_trend);
        match fee {
            Fee::Percentage(p) => {
                let flat_fee: Decimal = margin / Decimal::new(100, 0) * p;
                margin - flat_fee
            }
        }
    }

    /// If we sold the purchase for the current exchange rate trend, ignoring
    /// the sell fees, what would be net profit on this purchase.
    pub fn margin(&self, current_trend: BtcExchangeRate) -> HardCurr {
        self.btc * current_trend - self.buying_price
    }
}

fn main() {
    //
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_margin() {
        let purchase = {
            let rate = BtcExchangeRate::new(8_000, 0);
            let buying_price = HardCurr::new(8_000_50, 2);
            let btc = Btc::new(175, 2);
            Purchase::new(btc, buying_price, rate)
        };
        let current_trend = BtcExchangeRate::new(10_000, 0);
        assert_eq!(HardCurr::new(9_499_50, 2), purchase.margin(current_trend));

        let purchase = {
            let rate = BtcExchangeRate::new(100, 0);
            let buying_price = HardCurr::new(510, 0);
            let btc = Btc::new(5, 0);
            Purchase::new(btc, buying_price, rate)
        };
        let current_trend = BtcExchangeRate::new(50, 0);
        assert_eq!(HardCurr::new(-260, 0), purchase.margin(current_trend));
    }

    #[test]
    fn should_return_margin_minus_fee() {
        let purchase = {
            let rate = BtcExchangeRate::new(100, 0);
            let buying_price = HardCurr::new(205, 0);
            let btc = Btc::new(2, 0);
            Purchase::new(btc, buying_price, rate)
        };
        let current_trend = BtcExchangeRate::new(1000, 0);
        let fee = Fee::Percentage(Decimal::new(10, 0));
        assert_eq!(
            Decimal::new(1_615_50, 2),
            purchase.margin_after_fee(current_trend, fee)
        );
    }

    #[test]
    fn should_compare_two_purchases_on_basis_of_their_exchange_rate() {
        let purchase_lower_rate = {
            let rate = BtcExchangeRate::new(100, 0);
            let buying_price = HardCurr::new(205, 0);
            let btc = Btc::new(2, 0);
            Purchase::new(btc, buying_price, rate)
        };

        let purchase_higher_rate = {
            let rate = BtcExchangeRate::new(200, 0);
            let buying_price = HardCurr::new(230, 0);
            let btc = Btc::new(1, 0);
            Purchase::new(btc, buying_price, rate)
        };

        assert!(purchase_lower_rate < purchase_higher_rate);

        let mut account = PuchaseAccount::default();
        account.push(purchase_lower_rate);
        account.push(purchase_higher_rate.clone());

        assert!(a);
    }
}
