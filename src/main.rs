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

/// A number type refering to percents.
pub type Percentage = Decimal;

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

pub type PurchaseAccount = BinaryHeap<Purchase>;

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
        self.btc * current_trend - self.buying_price()
    }

    /// How much have we paid in total for this offer, including fees.
    pub fn buying_price(&self) -> HardCurr {
        self.btc * self.rate
    }
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
    id: Uuid,
    // How much do we expect to trade the bitcoins for.
    rate: BtcExchangeRate,
    // What purchases are calculated in for the offer.
    purchases: Vec<Purchase>,
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

/// Looks at the purchases we've made and sells the ones which make profit.
pub fn collect_profit(
    account: &mut PurchaseAccount,
    rate: BtcExchangeRate,
    fee: Fee,
    purchase_minimum_margin: Percentage,
) -> Option<Offer> {
    let mut purchases_to_sell = Vec::new();

    loop {
        // Iterates the queue of the purchases, always looking at the one we
        // got for the lowest price.
        if let Some(top_purchase) = account.peek() {
            let margin = top_purchase.margin_after_fee(rate, fee);

            // We calculate the minimum margin by finding out how much is
            // N % from the money spent on the bitcoin.
            let flat_minimum_margin =
                top_purchase.buying_price() / Decimal::new(100, 0) * purchase_minimum_margin;

            // If selling this offer yields expected marging, then sell it.
            if margin > flat_minimum_margin {
                // It's safe to unwrap here because we've just peeked into the
                // queue and it returned Some.
                purchases_to_sell.push(account.pop().unwrap());
                continue;
            }
        }

        break;
    }

    if !purchases_to_sell.is_empty() {
        Some(Offer::new(rate, purchases_to_sell))
    } else {
        None
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
            let btc = Btc::new(175, 2);
            Purchase::new(btc, rate)
        };
        let current_trend = BtcExchangeRate::new(10_000, 0);
        assert_eq!(HardCurr::new(3500, 0), purchase.margin(current_trend));

        let purchase = {
            let rate = BtcExchangeRate::new(100, 0);
            let btc = Btc::new(5, 0);
            Purchase::new(btc, rate)
        };
        let current_trend = BtcExchangeRate::new(50, 0);
        assert_eq!(HardCurr::new(-250, 0), purchase.margin(current_trend));
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

    #[test]
    fn should_collect_all_purchases_which_yield_profit() {
        let fee = Fee::Percentage(Decimal::new(1, 0));

        let purchase_for_1000 = {
            let rate = BtcExchangeRate::new(1000, 0);
            let btc = Btc::new(5, 0);
            Purchase::new(btc, rate)
        };

        let purchase_for_900 = {
            let rate = BtcExchangeRate::new(900, 0);
            let btc = Btc::new(1, 0);
            Purchase::new(btc, rate)
        };

        let purchase_for_450 = {
            let rate = BtcExchangeRate::new(450, 0);
            let btc = Btc::new(5, 0);
            Purchase::new(btc, rate)
        };

        let mut account = PurchaseAccount::default();
        account.push(purchase_for_1000.clone());
        account.push(purchase_for_450.clone());
        account.push(purchase_for_900.clone());

        {
            let trend = BtcExchangeRate::new(1000, 0);
            let min_margin = Percentage::new(20, 0);
            let mut account = account.clone();
            let offer = collect_profit(&mut account, trend, fee, min_margin)
                .expect("There is one purchase we want to sell with this profit");
            assert_eq!(vec![purchase_for_450.clone()], offer.purchases);
        }

        {
            let trend = BtcExchangeRate::new(1000, 0);
            let min_margin = Percentage::new(5, 0);
            let mut account = account.clone();
            let offer = collect_profit(&mut account, trend, fee, min_margin)
                .expect("There is one purchase we want to sell with this profit");
            assert_eq!(
                vec![purchase_for_450.clone(), purchase_for_900.clone()],
                offer.purchases
            );
        }

        {
            let trend = BtcExchangeRate::new(400, 0);
            let min_margin = Percentage::new(5, 0);
            let mut account = account.clone();
            assert!(collect_profit(&mut account, trend, fee, min_margin).is_none());
        }
    }
}
