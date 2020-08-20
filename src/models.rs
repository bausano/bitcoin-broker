use {
    std::{cmp::Ordering, collections::BinaryHeap},
    uuid::Uuid,
};

use crate::prelude::*;

/// The list of purchases sorted by the exchange rate at which the purchase
/// was made.
pub type PurchaseAccount = BinaryHeap<Purchase>;

/// A purchase holds information about transaction history of our buy requests
/// at market. The lower the exchange rate the better purchase we've made.
#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub struct Purchase {
    /// The unique id generated when the purchase was made.
    pub id: Uuid,
    /// How much bitcoin have we bought with this purchase.
    pub btc: Btc,
    /// The rate at which the bitcoin was exchanged. This rate takes into an
    /// account the fees paid to buy the bitcoins, so that the exact price we
    /// paid for this purchase can be calculated with btc * rate.
    pub rate: BtcExchangeRate,
}

/// The provider will take a cut from the transaction.
#[derive(Clone, Copy)]
pub enum Fee {
    Percentage(Percentage),
    None,
}

/// Represents an existing offer on the marketplace to sell bitcoins. When the
/// offer is accepted on the market place, it might happen for larger sum of
/// bitcoins than a single offer - this is because we merge several purchases
/// into one when we get a good deal.
///
/// When the offer is accepted, we calculate net profit by subtracting all
/// purchase costs from it.
#[derive(Debug)]
pub struct Offer {
    pub id: Uuid,
    // How much do we expect to trade the bitcoins for.
    pub rate: BtcExchangeRate,
    // What purchases are calculated in for the offer.
    pub purchases: Vec<Purchase>,
}

/// Each Purchase is evaluated primarily based on what was the exchange rate we
/// bought it for. That's why we order the queue by the rate and when we get an
/// update on the current $/BTC trend, we start evaluating purchases from the
/// top.
///
/// The lower the rate the better, therefore
/// `Purchase(rate: 8.000 $/btc) > Purchase(rate: 9.000 $/btc)`
impl Ord for Purchase {
    fn cmp(&self, other: &Self) -> Ordering {
        other.rate.cmp(&self.rate)
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

impl Purchase {
    /// Creates a new purchase from given data.
    pub fn new(btc: Btc, rate: BtcExchangeRate) -> Self {
        Self {
            id: Uuid::new_v4(),
            btc,
            rate,
        }
    }

    /// If we sold the purchase for the current exchange rate trend, and
    /// deducted the provider's cut, how much would we make on the purchase.
    pub fn margin_after_fee(
        &self,
        current_trend: BtcExchangeRate,
        fee: Fee,
    ) -> Cash {
        let margin = self.margin(current_trend);
        match fee {
            Fee::Percentage(p) => {
                let flat_fee: Decimal = margin / Decimal::new(100, 0) * p;
                margin - flat_fee
            }
            Fee::None => margin,
        }
    }

    /// If we sold the purchase for the current exchange rate trend, ignoring
    /// the sell fees, what would be net profit on this purchase.
    pub fn margin(&self, current_trend: BtcExchangeRate) -> Cash {
        self.btc * current_trend - self.buying_price()
    }

    /// How much have we paid in total for this offer, including fees.
    pub fn buying_price(&self) -> Cash {
        self.btc * self.rate
    }
}

impl Offer {
    pub fn new(rate: BtcExchangeRate, purchases: Vec<Purchase>) -> Self {
        Self {
            id: Uuid::new_v4(),
            rate,
            purchases,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_margin() {
        let purchase = {
            let rate = BtcExchangeRate::new(8_000, 0);
            let btc = Btc::new(175, 2);
            Purchase::new(btc, rate)
        };
        let current_trend = BtcExchangeRate::new(10_000, 0);
        assert_eq!(Cash::new(3500, 0), purchase.margin(current_trend));

        let purchase = {
            let rate = BtcExchangeRate::new(100, 0);
            let btc = Btc::new(5, 0);
            Purchase::new(btc, rate)
        };
        let current_trend = BtcExchangeRate::new(50, 0);
        assert_eq!(Cash::new(-250, 0), purchase.margin(current_trend));
    }

    #[test]
    fn should_return_margin_minus_fee() {
        let purchase = {
            let rate = BtcExchangeRate::new(100, 0);
            let btc = Btc::new(2, 0);
            Purchase::new(btc, rate)
        };
        let current_trend = BtcExchangeRate::new(1000, 0);
        let fee = Fee::Percentage(Decimal::new(10, 0));
        assert_eq!(
            Decimal::new(1_620, 0),
            purchase.margin_after_fee(current_trend, fee)
        );
    }

    // Lower rate is better as it was cheaper to buy the bitcoins.
    #[test]
    fn should_compare_two_purchases_on_basis_of_their_exchange_rate() {
        let purchase_lower_rate = {
            let rate = BtcExchangeRate::new(100, 0);
            let btc = Btc::new(2, 0);
            Purchase::new(btc, rate)
        };

        let purchase_higher_rate = {
            let rate = BtcExchangeRate::new(200, 0);
            let btc = Btc::new(1, 0);
            Purchase::new(btc, rate)
        };

        assert!(purchase_lower_rate > purchase_higher_rate);

        let mut account = PurchaseAccount::default();
        account.push(purchase_lower_rate.clone());
        account.push(purchase_higher_rate);

        assert_eq!(Some(&purchase_lower_rate), account.peek());
    }
}
