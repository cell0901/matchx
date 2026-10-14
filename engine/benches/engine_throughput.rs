use std::time::{Duration, Instant};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use crossbeam_channel::{bounded, unbounded};
use engine::{
    trade::{
        BalanceActions, MARKETS, Market, OrderbookActions, Origin, SCALE_FACTOR,
        spawn_benchmark_actors,
    },
    types::{CreateOrderPayload, OrderSide, OrderType},
};
use uuid::Uuid;

// Start actors → create orders → fund users → wait for funding
//                                       [timer starts]
// Send all orders → receive every post-settlement completion
//                                       [timer stops]
// Check that none were rejected


const PAIRS_PER_MARKET: usize = 1_000;
const PRICE: u64 = 100 * SCALE_FACTOR;
const QUANTITY: u64 = SCALE_FACTOR;

fn run_batch(markets: &[Market]) -> Duration {
    let (completed_tx, completed_rx) = unbounded();
    let (market_senders, balance_sender) = spawn_benchmark_actors(completed_tx);
    let mut orders = Vec::with_capacity(markets.len() * PAIRS_PER_MARKET * 2);

    for (market_index, &market) in markets.iter().enumerate() {
        let seller = Uuid::from_u128(1_000 + market_index as u128 * 2);
        let buyer = Uuid::from_u128(1_001 + market_index as u128 * 2);

        balance_sender
            .send(BalanceActions::Deposit(
                market.base,
                QUANTITY * PAIRS_PER_MARKET as u64,
                seller,
                String::new(),
                Origin::List,
            ))
            .unwrap();
        balance_sender
            .send(BalanceActions::Onramp(
                PRICE * PAIRS_PER_MARKET as u64,
                buyer,
                String::new(),
                Origin::List,
            ))
            .unwrap();
    }

    // Alternate markets so the four market threads have work concurrently.
    for pair in 0..PAIRS_PER_MARKET {
        for (market_index, &market) in markets.iter().enumerate() {
            let seller = Uuid::from_u128(1_000 + market_index as u128 * 2);
            let buyer = Uuid::from_u128(1_001 + market_index as u128 * 2);
            let id = ((market_index as u128 + 1) << 64) | (pair as u128 * 2);
            orders.push((
                market,
                CreateOrderPayload {
                    symbol: market,
                    order_type: OrderType::Limit,
                    order_side: OrderSide::Sell,
                    price: Some(PRICE),
                    quantity: QUANTITY,
                    user_id: seller,
                    order_id: Uuid::from_u128(id),
                },
            ));
            orders.push((
                market,
                CreateOrderPayload {
                    symbol: market,
                    order_type: OrderType::Limit,
                    order_side: OrderSide::Buy,
                    price: Some(PRICE),
                    quantity: QUANTITY,
                    user_id: buyer,
                    order_id: Uuid::from_u128(id + 1),
                },
            ));
        }
    }

    // FIFO barrier: all funding messages have been applied before timing starts.
    let (ready_tx, ready_rx) = bounded(1);
    balance_sender
        .send(BalanceActions::BenchmarkBarrier(ready_tx))
        .unwrap();
    ready_rx.recv().unwrap();

    let order_count = orders.len();
    let start = Instant::now();
    for (market, order) in orders {
        market_senders
            .get(&market)
            .unwrap()
            .send(OrderbookActions::CreateOrder(
                order,
                String::new(),
                Origin::List,
            ))
            .unwrap();
    }

    // A completion comes from the market actor after the balance actor replies.
    let mut placed = 0;
    for _ in 0..order_count {
        if completed_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("engine stalled")
        {
            placed += 1;
        }
    }
    let elapsed = start.elapsed();
    assert_eq!(placed, order_count, "benchmark orders were rejected");
    elapsed
}

fn engine_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_actor_throughput");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    for (name, markets) in [
        ("one_market", &MARKETS[..1]),
        ("four_markets", &MARKETS[..]),
    ] {
        group.throughput(Throughput::Elements( // Criterion to report orders/sec—two orders per
                                               // pair, not fills per sec
            (markets.len() * PAIRS_PER_MARKET * 2) as u64,
        ));
        group.bench_function(name, |b| {
            b.iter_custom(|iterations| (0..iterations).map(|_| run_batch(markets)).sum());
        });
    }

    group.finish();
}

criterion_group!(benches, engine_throughput);
criterion_main!(benches);
