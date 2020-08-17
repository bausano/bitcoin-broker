# Broker

Broker connects to bitcoin trading APIs and makes decisions to buy and sell the
commodity. Broker should run more or less autonomously. It is a naive trading
algorithm which aims to make low risk net profit, albeit slowly.

## How it works

### Selling
A queue of purchases is stored. Each purchase has an information about how much
money was spent, how many bitcoins were bought and other metadata about the
transaction. It then monitors the market and puts out offers which would make
good margins against those purchases.

```
[
    {
        "spent": $4500,
        "amount": 0.75 BTN,
        "$/BTN": 5990
    },
    {
        "spent": $8010,
        "amount": 1 BTN,
        "$/BTN": 8000
    }
]

CURRENT $/BTN: 8000
```

In the example above we have two transactions. First transaction bought 0.75
bitcons for $4.5k including process fees. The bitcoins were bought for exchange
rate of $5990 for 1 BTN. Compared towards current trend, this was a good
purchase.

Selling the bitcoins from the second transaction would lead to a net loss.
Selling the first transaction would now make around $6k ignoring additional
fees. That's $1.5k net profit on this transaction.

Our algorithm will therefore try to sell the bitcoins bought in the first
transaction, but refuse to sell off the bitcoins from the second transaction.

In practise, we collect all transactions which are evaluated as "to sell" and
merge them into one offer for current trending price.

The evaluation of buy/sell is influenced by how much out of order the bitcoin
price is. If the current price is close to minimum over past 3 months which
lasted at least N days, the algorithm will require larger margins to sell the
bitcoins.

Another factor that incluences the required margin is how much bitcoin do we
have in the owned purchases. The less bitcoin we own, the higher margin we
required in order to sell.

The above is designed to be an algorithm which is a net positive, although it
is quite slow to generate money. We guard against selling the bitcoins low by
requiring that bitcoin is not sold during minima. We make sure not to sell all
bitcoins in the event of sudden rush. Since the $/BTN rate fluctuates a lot,
the strategy of just waiting for a good margin on a purchase will eventually
work out, especially if our purchases are all of similar value.

### Buying
To be designed yet, first we test the validity of the selling part.

## Validation
We pick some different buying algorithms, such as daily average, weekly minimum,
random every N days.
