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
    models::{Fee, Offer, Purchase, PurchaseAccount},
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
    /// The buyer actor made a purchase that the seller is now going to try to
    /// sell for better price.
    NewPurchase(Purchase),
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
        let message = if let Ok(message) = input.recv() {
            message
        } else {
            log::error!("The seller's input channel died. Stopping ...");
            break;
        };

        match route(message, &mut state) {
            Ok(Some(offer)) => {
                if output.send(offer).is_err() {
                    log::error!(
                        "The seller's output channel died. Stopping ..."
                    );
                    break;
                }
            }
            Ok(None) => (),
            Err(e) => {
                log::warn!("A message failed to be processed due to: {}", e)
            }
        }
    });
}

// Considers given message and if appropriate, commands bitcoins to be sold.
fn route(message: Message, state: &mut State) -> Result<Option<Offer>> {
    match message {
        Message::TrendReading {
            current_trend,
            observed_at,
        } => {
            if Instant::now().duration_since(observed_at) > _5MIN {
                Err(Box::new(Error::outdated_message()))
            } else {
                Ok(collect_profit(
                    &mut state.account,
                    current_trend,
                    state.fee,
                    state.min_margin,
                ))
            }
        }
        Message::NewPurchase(purchase) => {
            state.account.push(purchase);
            Ok(None)
        }
    }
}

// Looks at the purchases we've made and sells the ones which make profit.
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

            // If selling this offer yields expected margin, then sell it.
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
    use crossbeam_channel::bounded;

    use super::*;

    #[test]
    fn should_add_new_purchases_and_sell_the_one_with_profit() -> Result<()> {
        let fee = Fee::None;
        let min_margin = Percentage::new(10, 0);
        let (channel_in, seller_input) = bounded(0);
        let (seller_output, channel_out) = bounded(0);

        spawn(seller_input, seller_output, fee, min_margin);

        // Inserts a purchase with rate for 200 into the seller's msg box.
        let purchase_for_200 = {
            let rate = BtcExchangeRate::new(200, 0);
            let btc = Btc::new(1, 0);
            Purchase::new(btc, rate)
        };
        channel_in.send(Message::NewPurchase(purchase_for_200.clone()))?;

        assert!(channel_out.is_empty());

        let trend_500 = BtcExchangeRate::new(500, 0);

        // Tests that updates sent ages ago are not evaluated. We do this by
        // sending a message we expect to be ignored, sending another message
        // which confirms that the seller has evaluated this message already,
        // and then checking that the channel output is empty.
        let _10min_ago = Instant::now() - _5MIN - _5MIN;
        channel_in.send(Message::TrendReading {
            current_trend: trend_500,
            observed_at: _10min_ago,
        })?;

        // Inserts a purchase with rate for 1000 into the seller's msg box.
        let purchase_for_1000 = {
            let rate = BtcExchangeRate::new(1000, 0);
            let btc = Btc::new(1, 0);
            Purchase::new(btc, rate)
        };
        channel_in.send(Message::NewPurchase(purchase_for_1000.clone()))?;

        // This confirms that the channel ignored the deal which was observed
        // 10 minutes ago. Because we've sent another message to add a new
        // purchase, and still the channel is empty, means that the seller
        // hasn't put the purchase_for_200 to be offered even though the
        // trend we published was for 500.
        assert!(channel_out.is_empty());

        // This time we should get back an offer which contains the purchase
        // for 200.
        channel_in.send(Message::TrendReading {
            current_trend: trend_500,
            observed_at: Instant::now(),
        })?;
        let offer =
            channel_out.recv_timeout(Duration::from_millis(10)).unwrap();
        assert_eq!(&[purchase_for_200.clone()], offer.purchases.as_slice());
        assert_eq!(trend_500, offer.rate);

        // Send the same reading and check that the channel is empty. We wait
        // a few milliseconds to check that the error is not going to arrive
        // later. Although this behavior is tested later down the line as well
        // in a way which guarantees the correct behavior, realistically if
        // there's a bug the message will arrive in less than 10ms and it will
        // be clearer what's wrong.
        channel_in.send(Message::TrendReading {
            current_trend: trend_500,
            observed_at: Instant::now(),
        })?;
        assert!(channel_out.recv_timeout(Duration::from_millis(10)).is_err());

        // Inserts a purchase with rate for 1500 into the seller's msg box.
        let purchase_for_1500 = {
            let rate = BtcExchangeRate::new(1500, 0);
            let btc = Btc::new(1, 0);
            Purchase::new(btc, rate)
        };
        channel_in.send(Message::NewPurchase(purchase_for_1500.clone()))?;

        assert!(channel_out.is_empty(), "Cannot contain msgs at this point");

        let trend_2000 = BtcExchangeRate::new(2000, 0);
        channel_in.send(Message::TrendReading {
            current_trend: trend_2000,
            observed_at: Instant::now(),
        })?;
        let offer =
            channel_out.recv_timeout(Duration::from_millis(10)).unwrap();
        assert_eq!(
            &[purchase_for_1000, purchase_for_1500],
            offer.purchases.as_slice()
        );
        assert_eq!(trend_2000, offer.rate);

        Ok(())
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
                .expect(
                    "There is one purchase we want to sell with this profit",
                );
            assert_eq!(&[purchase_for_450.clone()], offer.purchases.as_slice());
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
                offer.purchases.as_slice()
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
