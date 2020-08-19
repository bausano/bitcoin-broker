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
//! ```
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

mod models;
mod prelude;
mod seller;

use {crossbeam_channel::unbounded, std::thread};

use {models::Fee, prelude::*};

fn main() {
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
