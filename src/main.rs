//! # Bitcoin broker
//! The broker app trades bitcoin over public APIs. See the design doc for more
//! information about the algorithm.
//!
//! The high level organization of the code are actors. Each relevant part is
//! wrapped into a thread that runs it. A communication between the threads is
//! achieved with channels.
//!
//! ## Flow of information
//!
//! ```text
//!      ...
//!
//!   ||     /\
//!   ||     ||
//!   \/     ||
//!
//! +------  Seller --------------------+
//! | Responsible for deciding which    |
//! | purchases to sell under what      |
//! | condition. Receives trend updates |
//! | and settings from control agent.  |
//! +-----------------------------------+
//!
//!   ||     /\
//!   ||     ||
//!   \/     ||
//!
//!     ...
//! ```

pub mod models;
pub mod prelude;
pub mod seller;

use {crossbeam_channel::unbounded, std::thread};

use {models::Fee, prelude::*};

fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    // The input (receiver) into the seller actor sends updates of current trend
    // or threshold for minimum_margin.
    let (_, seller_input) = unbounded();

    // The output of the seller (sender) actor is an order to sell certain
    // purchases.
    let (seller_output, _) = unbounded();
    let fee = Fee::Percentage(Percentage::new(25, 2));
    let min_margin = Percentage::new(5, 0);
    seller::spawn(seller_input, seller_output, fee, min_margin);

    loop {
        thread::park();
    }
}

#[cfg(test)]
mod tests {
    use {
        crossbeam_channel::bounded,
        rand::{thread_rng, Rng},
        serde::Deserialize,
        std::{collections::HashMap, time::Instant},
    };

    use {super::*, models::Purchase};

    // Path to a CSV file which contains historical data of btc/$ exchange
    // rates.
    // The header of the file is following:
    // `Date,Open,High,Low,Close,Adj Close,Volume`
    const HISTORICAL_DATA_PATH: &str =
        "tests/data/btc_usd_2019_02_01-2020_08_19.csv";

    #[derive(Debug, Deserialize)]
    #[serde(rename_all(deserialize = "PascalCase"))]
    struct HistoricalRow {
        date: String,
        open: BtcExchangeRate,
        high: BtcExchangeRate,
        low: BtcExchangeRate,
        close: BtcExchangeRate,
        adj_close: BtcExchangeRate,
        volume: usize,
    }

    // A naive test which runs the seller for a year while buyer buys bitcoin
    // randomly every ~ 2 days.
    //
    // * We conservatively set the buying price closer to daily maximum and
    // the selling price closer to daily minimum.
    // * initial investment of $2k
    // * We always buy BTC for $100.
    // * We don't buy if we don't have resources.
    // * Every second offer we place is not fulfilled.
    // * We sell for market's average calculated with (high - low) / 2.
    #[test]
    fn seller_should_yield_profit_from_historical_data() -> Result<()> {
        let fee = Fee::Percentage(Percentage::new(25, 2));
        let min_margin = Percentage::new(10, 0);
        let spending_per_purchase = Cash::new(250, 0);
        // Everyday we roll a dice whether we buy or not.
        let likelihood_of_purchase = 1.0 / 2.0;
        // Some fraction of our offers won't be accepted straight away
        let likelihood_of_offer_rejection = 1.0 / 2.0;
        let investment = Cash::new(2_000, 0);

        // env_logger::init();
        let mut rng = thread_rng();
        let (channel_in, seller_input) = bounded(0);
        let (seller_output, channel_out) = bounded(5);
        seller::spawn(seller_input, seller_output, fee, min_margin);

        let mut btc = Btc::new(0, 0);
        let mut cash = investment;

        // How much money have we made each month.
        let mut monthly_margin: HashMap<&str, Cash> =
            HashMap::with_capacity(24);

        let historical_data = load_historical_data();
        for row in &historical_data {
            let HistoricalRow {
                date, high, low, ..
            } = row;
            let avg = high + low / Decimal::new(2, 0);

            // Updates the current trend. We sell for value slightly below
            // market average to be conservative.
            channel_in.send(seller::Message::TrendReading {
                current_trend: avg - (avg - low) / Decimal::new(2, 0),
                observed_at: Instant::now(),
            })?;

            // Every now and then we buy some bitcoins without thinking for
            // a random price between daily average and daily high.
            if rng.gen_bool(likelihood_of_purchase) {
                let rate = avg + (high - avg) / Decimal::new(2, 0);
                let btc_to_buy = spending_per_purchase / rate;
                let purchase = Purchase::new(btc_to_buy, rate);
                channel_in.send(seller::Message::NewPurchase(purchase))?;

                btc += btc_to_buy;
                cash -= spending_per_purchase;
            }

            // We do this here to make sure every month is represented in the
            // hash map. This prevents incorrect average monhtly margin
            // calculation.
            let yyyy_mm_len = "2020-01".len();
            let margin_this_month =
                monthly_margin.entry(&date[0..yyyy_mm_len]).or_default();

            // Evaluates attempts to sell the bitcoins.
            while let Ok(offer) = channel_out.try_recv() {
                // Every second offer on the marketplace expires and is never
                // fulfilled.
                if rng.gen_bool(likelihood_of_offer_rejection) {
                    for purchase in offer.purchases {
                        cash += purchase.btc * offer.rate;
                        btc -= purchase.btc;

                        *margin_this_month +=
                            purchase.margin_after_fee(offer.rate, fee);
                    }
                } else {
                    for purchase in offer.purchases {
                        channel_in
                            .send(seller::Message::NewPurchase(purchase))?;
                    }
                }
            }
        }

        let total_monthly_margin = monthly_margin
            .iter()
            .fold(Cash::new(0, 0), |cash, (_, margin)| cash + margin);
        let avg_monthly_margin =
            total_monthly_margin / Cash::from(monthly_margin.len());
        let monthly_margin: Vec<_> = {
            let mut v: Vec<_> = monthly_margin.into_iter().collect();
            v.sort_by(|(date, _), (date2, _)| date.cmp(date2));
            v.into_iter()
                .map(|(date, margin)| {
                    format!("[{}] ${}", date, margin.round_dp(2))
                })
                .collect()
        };
        println!(
            "Monthly margin: \n{:#?} \n(total ${}, avg ${})",
            monthly_margin,
            total_monthly_margin.round_dp(2),
            avg_monthly_margin.round_dp(2),
        );
        println!(
            "Ended up with {} BTC and net ${}.",
            btc.round_dp(6),
            (cash - investment).round_dp(2)
        );

        Ok(())
    }

    fn load_historical_data() -> Vec<HistoricalRow> {
        let mut rdr = csv::Reader::from_path(HISTORICAL_DATA_PATH).unwrap();
        rdr.deserialize().map(|r| r.unwrap()).collect()
    }
}
