//! Seller is an actor which decides when to sell of owned bitcoins.
//! It will make a decision when a new bitcoin rate has been calculated. When
//! it reaches the decision to sell, it sends a message about the offer we
//! should make at the bitcoin exchange marketplace. Relevant logic which
//! implements the API sends the request from that message.

use {
    crossbeam_channel::{Receiver, Sender},
    std::{
        thread,
        time::{Duration, Instant},
    },
};

use crate::{
    models::{Fee, Offer, PurchaseAccount},
    prelude::*,
};

const _5MIN: Duration = Duration::from_secs(5 * 60);

pub enum Message {
    /// We've got an update on the current exchange rate.
    TrendReading {
        current_trend: BtcExchangeRate,
        // We send a timestamp of when was this rate observed. Messages older
        // than N minutes are discarded.
        observed_at: Instant,
    },
}

struct State {
    // Lists the purchases that have been done so far.
    account: PurchaseAccount,
    // How much does the market place change us for the transaction.
    //
    // # Important
    // This should only be the selling fee. The fee we paid to buy the bitcoins
    // is already accounted for in the purchase exchange rate.
    fee: Fee,
    // What's the minimum that we expect to earn on each purchase.
    min_margin: Percentage,
}

/// Spawns a new thread which runs the seller logic. Use the parameters of this
/// method to configure the seller.
pub fn spawn(
    input: Receiver<Message>,
    output: Sender<Offer>,
    fee: Fee,
    min_margin: Percentage,
) {
    let mut state = State {
        account: PurchaseAccount::default(),
        fee,
        min_margin,
    };

    thread::spawn(move || loop {
        match input.recv() {
            Ok(Message::TrendReading {
                current_trend,
                observed_at,
            }) => {
                if Instant::now().duration_since(observed_at) > _5MIN {
                    todo!("Warn I guess");
                } else if let Some(offer) = collect_profit(
                    &mut state.account,
                    current_trend,
                    state.fee,
                    state.min_margin,
                ) {
                    output.send(offer).expect("TODO");
                }
            }
            Err(_e) => todo!("Log the error and shut down."),
        }
    });
}

/// Looks at the purchases we've made and sells the ones which make profit.
fn collect_profit(
    account: &mut PurchaseAccount,
    rate: BtcExchangeRate,
    fee: Fee,
    min_margin: Percentage,
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
                top_purchase.buying_price() / Decimal::new(100, 0) * min_margin;

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

#[cfg(test)]
mod tests {
    use super::*;
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
                .expect(
                    "There is one purchase we want to sell with this profit",
                );
            assert_eq!(&[purchase_for_450.clone()], offer.purchases());
        }

        {
            let trend = BtcExchangeRate::new(1000, 0);
            let min_margin = Percentage::new(5, 0);
            let mut account = account.clone();
            let offer = collect_profit(&mut account, trend, fee, min_margin)
                .expect(
                    "There is one purchase we want to sell with this profit",
                );
            assert_eq!(
                &[purchase_for_450, purchase_for_900],
                offer.purchases()
            );
        }

        {
            let trend = BtcExchangeRate::new(400, 0);
            let min_margin = Percentage::new(5, 0);
            let mut account = account.clone();
            assert!(
                collect_profit(&mut account, trend, fee, min_margin).is_none()
            );
        }
    }
}
