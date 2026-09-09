use std::{collections::HashMap, num::TryFromIntError, thread};
use crossbeam_channel::{RecvError, Sender, bounded, unbounded};
use rustc_hash::FxHashMap;
use uuid::Uuid;
use crate::{trade::{Asset, BalanceActions, Fill, MARKETS, Market, Order, Orderbook, OrderbookActions, SCALE_FACTOR, SettleFillsData, SettleResult, ValidateAndLockData, ValidateAndLockResponse}, types::{MessageFromApi, OrderSide}};

pub struct Balance {
    pub available: u64,
    pub locked: u64
}

impl Default for Balance {
    fn default() -> Self {
        Self {
            available: 0,
            locked: 0
        }
    }
}

#[derive(Clone)]
pub struct Engine {
    // Market -> sender
    pub market_senders: HashMap<Market, Sender<OrderbookActions>>,
    pub balance_sender: Sender<BalanceActions>
}

impl Engine {
    pub fn new() -> Engine{ 
        let balance_sender = spawn_balance_thread();
        
        let mut market_senders = HashMap::new();   
        for market in MARKETS {
            let market_tx = spawn_market_thread(market, balance_sender.clone());
            market_senders.insert(market, market_tx);
        }

        Engine {
            market_senders,
            balance_sender
        }
    }

    pub async fn process(&self, message: MessageFromApi) {
        // createorder
        // cancel order
        //  onramp
        //  deposit
        // getopen orders
        match message {
            MessageFromApi::CreateOrder(payload) => {
                let market = payload.symbol;
                let orderbook = self.market_senders.get(&market);

                match orderbook {
                    Some(sender)  => {
                        if let Err(e) = sender.send(OrderbookActions::CreateOrder(payload)) {
                            println!("market thread unreachable {}", e);
                            // early return to api. order rejected, market unavailable
                        };
                    },
                    None => {
                        // return order rejected. invalid market
                    }
                };
            }
        };
    }
}

fn spawn_balance_thread() -> Sender<BalanceActions> {
        let (tx, rx) = unbounded::<BalanceActions>();

        // User id -> assets
        let mut balances: FxHashMap<(Uuid, Asset), Balance> = FxHashMap::default();

        // spawn the balance thread
        thread::spawn(move|| {
           while let Ok(val) = rx.recv() {
            match val {
                BalanceActions::ValidateAndLockFunds(data) => {
                    if data.side == OrderSide::Buy { // if buy
                        let avl_balance = balances.get_mut(&(data.user_id, Asset::USDC)); 
                        match avl_balance {
                            Some(val) => {
                                let notional = ((data.price as u128) * (data.quantity as u128)) / SCALE_FACTOR as u128;
                                match u64::try_from(notional){
                                    Ok(required) => {
                                        if val.available < required {
                                            // send insufficient funds
                                            let _ = data.resp.send(ValidateAndLockResponse::InsufficientFunds);
                                        } else {
                                            // lock the funds 
                                            val.available -= required;
                                            val.locked += required;
                                            // send success
                                            let _ = data.resp.send(ValidateAndLockResponse::Success);
                                        }
                                    },
                                    Err(_) => {
                                        // send overflow
                                        let _ = data.resp.send(ValidateAndLockResponse::Overflow);
                                    }
                                    
                                };
                            },
                            None => { // if not balance entry found
                                  // send insufficient funds
                                let _ = data.resp.send(ValidateAndLockResponse::InsufficientFunds);
                            }
                        };
                    } else { // for ask
                        // get the asset balance. SOL, BTC etc
                        let avl_balance = balances.get_mut(&(data.user_id, data.asset)); 
                        match avl_balance {
                            Some(val) => {
                                if val.available < data.quantity { // since this is ask. just
                                    // compare qty
                                    // send insufficient funds
                                    let _ = data.resp.send(ValidateAndLockResponse::InsufficientFunds);
                                } else {
                                    val.available -= data.quantity;
                                    val.locked += data.quantity;
                                    // send success
                                    let _ = data.resp.send(ValidateAndLockResponse::Success);
                                }
                            },
                            None => {
                                  // send insufficient funds or 0 balance found
                                let _ = data.resp.send(ValidateAndLockResponse::InsufficientFunds);
                            }
                        };
                    }
                },
                BalanceActions::SettleFills(data) => {
                    if data.side == OrderSide::Buy {
                        for fill in &data.fills {
                            // first decrese the quote locked balance of taker
                            let notional = calc_quote_amount(fill.price, fill.quantity);
                            if let Err(err) = notional {
                                    println!("overflow error {}", err);
                                    data.resp.send(SettleResult::Overflow);
                                    continue;
                            };
                            if let Some(bal) = balances.get_mut(&(fill.user_id, data.quote_asset)) {
                                bal.locked -= notional.unwrap();
                            }

                            // increase taker base asset avlbl amount
                            balances.entry((fill.user_id, data.base_asset)).or_insert(Balance::default()).available += fill.quantity;

                            // decrease maker base Asset locked amount
                            if let Some(bal) = balances.get_mut(&(fill.other_user_id, data.base_asset)){
                                bal.locked -= fill.quantity;
                            }

                            // increase maker quote asset avlbl amount
                            balances.entry((fill.other_user_id, data.quote_asset)).or_insert(Balance::default()).available += notional.unwrap();
                        }
                    } else {
                        for fill in &data.fills {
                            let notional = calc_quote_amount(fill.price, fill.quantity);
                            if let Err(err) = notional {
                                    println!("overflow error {}", err);
                                    data.resp.send(SettleResult::Overflow);
                                    continue;
                            };
                            // decrease seller locked base asset  (taker)
                            if let Some(bal) = balances.get_mut(&(fill.user_id, data.base_asset)) {
                                bal.locked -= fill.quantity;
                            }

                            // increase seller quote asset avlbl amount
                            balances.entry((fill.user_id, data.quote_asset)).or_insert(Balance::default()).available += notional.unwrap();

                            // decrease maker locked quote asset amount
                            if let Some(bal) = balances.get_mut(&(fill.other_user_id, data.quote_asset)) {
                                bal.locked -= notional.unwrap();
                            }

                            // increase maker base asset avlbl amount
                            balances.entry((fill.other_user_id, data.base_asset)).or_insert(Balance::default()).available += fill.quantity;
                        }
                    }
                    // send Settle:ok
                    data.resp.send(SettleResult::Ok);
                }
            };
        }
        });
    tx
}

fn spawn_market_thread(market:Market, balance_trasmitter: Sender<BalanceActions>) -> Sender<OrderbookActions>{
    let (market_tx, market_rx) = unbounded::<OrderbookActions>(); // normal mpsc channel but with no limit. since we
    // will handle many messages. although if orderbook matching gets slowed but the incoming
    // orders keeps flowing then this will increase indefenitely. fine for this
    // but worth considering using bounded(n) with some limit later for robustness 

    thread::spawn(move || {// spawn each orderbook/market
        let mut orderbook = Orderbook::new(market);

        while let Ok(val) = market_rx.recv() {
            // Place order
            // Cancel Order
            // Get open orders
            // Get depth
            match val {
               OrderbookActions::CreateOrder(payload) => {
                    // validate and lock funds
                    let res = validate_and_lock(payload.user_id, payload.price, payload.quantity, 
                        payload.order_side, payload.symbol, balance_trasmitter.clone());

                    match res {
                        Ok(ValidateAndLockResponse::Success) => { // if success create order and
                            // add
                            let (balance_settle_tx,balance_settle_rx) = bounded::<SettleResult>(1);
                            let order = Order {
                                order_id: payload.order_id,
                                price: payload.price,
                                user_id: payload.user_id,
                                quantity: payload.quantity,
                                order_side: payload.order_side,
                                order_type: payload.order_type,
                                filled: 0
                            };
                            let (executed_qty,order_status, fills) = orderbook.add_order(order); // get the fills and change balances
                            balance_trasmitter.clone().send(BalanceActions::SettleFills(SettleFillsData {
                                fills,
                                side: payload.order_side,
                                base_asset: payload.symbol.base,
                                quote_asset: payload.symbol.quote,
                                resp: balance_settle_tx
                            }));

                            if let Ok(val) = balance_settle_rx.recv() { // wait till balance thread
                                // update funds
                                if val == SettleResult::Ok {
                                    // return order success created with executed_qty and
                                    // order_status
                                }
                            } else {
                                println!("balance thread recv Error");
                            }
                        },
                        Ok(ValidateAndLockResponse::InsufficientFunds) => {
                            // reject order 
                        },
                        Ok(ValidateAndLockResponse::Overflow) => {

                        },
                        Err(err) => {
                            eprintln!("balance thread is unreachable, recv error {}", err);

                        }
                    };
                    
               }
            };
        }
    });

    market_tx
}

fn validate_and_lock(user_id: Uuid, price:u64, quantity: u64, side: OrderSide, market: Market, balance_tx: Sender<BalanceActions>) 
    -> Result<ValidateAndLockResponse, RecvError>
{
    let (balance_resp_tx, balance_resp_rx) = bounded::<ValidateAndLockResponse>(1); // so only 1 value can fit inside

    let a = balance_tx.send(BalanceActions::ValidateAndLockFunds(ValidateAndLockData {
        user_id,
        price,
        quantity,
        side,
        asset: market.base,
        resp: balance_resp_tx
    }));

    if let Err(err)= a {
        println!("failed to send recv was dropped {}", err);
    };

    balance_resp_rx.recv().map_err(|e| e)
}

fn calc_quote_amount(price: u64, quantity: u64)-> Result<u64, TryFromIntError> { // to safely parse after multiplyin two u64
    let product = ((price as u128) * (quantity as u128)) / SCALE_FACTOR as u128;

    u64::try_from(product).map_err(|e| e)
}
