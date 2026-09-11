use std::{collections::HashMap, env, num::TryFromIntError, thread};
use crossbeam_channel::{RecvError, Sender, bounded, unbounded};
use redis::Commands;
use rustc_hash::FxHashMap;
use uuid::Uuid;
use crate::{trade::{Asset, BalanceActions, MARKETS,
    Market, Order, Orderbook, OrderbookActions, SCALE_FACTOR, SettleFillsData, SettleResult::{self}, ValidateAndLockData, ValidateAndLockResponse}, types::{MessageFromApi, OrderSide}};

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
            },
            MessageFromApi::CancelOrder(payload) => {
                let market = payload.symbol;
                let orderbook = self.market_senders.get(&market);

                match orderbook {
                    Some(sender)  => {
                        if let Err(e) = sender.send(OrderbookActions::CancelOrder(payload)) {
                            println!("market thread unreachable {}", e);
                            // early return to api. order rejected, market unavailable
                        };
                        // wait on orderbook response
                    },
                    None => {
                        // return order rejected. invalid market
                    }
                }
            },
            MessageFromApi::Onramp(payload) => { // increase the usdc amount for this user
                self.balance_sender.send(BalanceActions::Onramp(payload.amount, payload.user_id));
            },
            MessageFromApi::Deposit(payload) => { //
                self.balance_sender.send(BalanceActions::Deposit(payload.asset, payload.quantity, payload.user_id));
            },
            MessageFromApi::GetOpenOrders(payload) => {
                let orderbook = self.market_senders.get(&payload.symbol);

                match orderbook {
                    Some(sender) => {
                        if let Err(e) = sender.send(OrderbookActions::GetOpenOrders(payload)) {
                            eprintln!("market thread unreachable {}", e);
                        }
                    },
                    None => {
                        // return invalid market
                    }

                };
            },
            MessageFromApi::GetDepth(market) => {
                let orderbook = self.market_senders.get(&market);

                match orderbook {
                    Some(sender) => {
                        sender.send(OrderbookActions::GetDepth);
                    },
                    None => {
                        // return invalid market
                    }

                };
            },
            MessageFromApi::GetBalance(payload) => {
                self.balance_sender.send(BalanceActions::GetBalance(payload.asset, payload.user_id));
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
                    // send Settle::Success
                    data.resp.send(SettleResult::Success);
                },
                BalanceActions::CancelAndUpdateBalance(order, symbol, resp) => {
                    if order.order_side == OrderSide::Buy {
                        let notional = calc_quote_amount(order.price, order.quantity - order.filled);
                        match notional {
                            Ok(left_qty_price) => {
                                if let Some(bal) = balances.get_mut(&(order.user_id, symbol.quote)) {
                                    bal.available += left_qty_price;
                                    bal.locked -= left_qty_price;
                                    resp.send(SettleResult::Success);
                                }
                            },
                            Err(e) => {
                                eprintln!("overflow error {}", e);
                                resp.send(SettleResult::Overflow);
                            }
                        };

                    } else { // if sell was cancelled increase base_asset avlbl amount
                        if let Some(bal) = balances.get_mut(&(order.user_id, symbol.base)) {
                            bal.available += order.quantity - order.filled;
                            bal.locked -= order.quantity - order.filled;
                            resp.send(SettleResult::Success);
                        }
                    }
                },
                BalanceActions::Onramp(amount, user_id) => {
                 balances.entry((user_id, Asset::USDC)).or_insert(Balance::default()).available += amount; 
                 // send back to pub sub successfull
                },
                BalanceActions::Deposit(asset, qty, user_id) => {
                 balances.entry((user_id, asset)).or_insert(Balance::default()).available += qty; 
                 // send back to pub sub successfull
                },
                BalanceActions::GetBalance(asset, user_id) => {
                    match balances.get(&(user_id,asset)){
                        Some(bal) => {
                            // send back to pub sub the balance
                        }, 
                        None => {
                            // send back amount 0
                        }
                        
                    }
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
        let redis_url = match env::var("REDIS_URL") {
            Ok(url) => url,
            Err(e) => panic!("redis url not found in environment: {}", e),
        };
        let redis_client = redis::Client::open(redis_url).expect("failed to get redis client");

        let mut redis_conn = redis_client.get_connection().unwrap(); // sync connection

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
                        payload.order_side.clone(), payload.symbol, balance_trasmitter.clone());

                    match res {
                        Ok(ValidateAndLockResponse::Success) => { // if success create order and
                            // add
                            let (balance_settle_tx,balance_settle_rx) = bounded::<SettleResult>(1);
                            let order = Order {
                                order_id: payload.order_id,
                                price: payload.price,
                                user_id: payload.user_id,
                                quantity: payload.quantity,
                                order_side: payload.order_side.clone(),
                                order_type: payload.order_type,
                                filled: 0
                            };
                            let (executed_qty, order_status, fills) = orderbook.add_order(order); // get the fills and change balances
                            balance_trasmitter.clone().send(BalanceActions::SettleFills(SettleFillsData {
                                fills,
                                side: payload.order_side,
                                base_asset: payload.symbol.base,
                                quote_asset: payload.symbol.quote,
                                resp: balance_settle_tx
                            }));

                            if let Ok(val) = balance_settle_rx.recv() { // wait till balance thread
                                // update funds
                                if val == SettleResult::Success {
                                    // return order success created with executed_qty and
                                    // order_status
                                    // let _ = redis_conn.publish(channel, message)
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
               },
                OrderbookActions::CancelOrder(payload) => {
                    let res = orderbook.cancel_order(payload.order_id, payload.user_id);
                    match res {
                        Ok(cancelled_order)  => { // if removed successfully. then update balances
                            let (update_balance_tx, update_balance_rx) = bounded::<SettleResult>(1);

                            balance_trasmitter.clone().send(BalanceActions::CancelAndUpdateBalance(cancelled_order, payload.symbol, update_balance_tx));
                            if let Ok(val) = update_balance_rx.recv() { // wait till balance thread
                                if val == SettleResult::Success {
                                    // return cancelled_order with order_id and remaining quantity
                                }
                            } else {
                                println!("balance thread recv Error");
                            }
                        },
                        Err(_) => { 
                            // send response back with acutual erro message

                        }

                    };
                },
                OrderbookActions::GetDepth => { // ideally we should change the way to get depth 
                    // on starting as every order comes the http gets subs to engine and engine
                    // pubs every delta of order/cancel . http maintains local in memory depth
                    let depth = orderbook.get_depth();
                    // publish
                },
                OrderbookActions::GetOpenOrders(payload) => {
                    let open_orders = orderbook.get_open_orders(payload.user_id);
                    // publish
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
