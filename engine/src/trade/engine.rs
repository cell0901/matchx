use std::{collections::HashMap, num::TryFromIntError, thread};
use crossbeam_channel::{RecvError, Sender, bounded, unbounded};
use redis::{AsyncCommands, aio::MultiplexedConnection};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use rustc_hash::FxHashMap;
use serde::Serialize;
use uuid::Uuid;
use crate::{trade::{Asset, BalanceActions, MARKETS, Market, Order, OrderStatus, Orderbook, OrderbookActions, SCALE_FACTOR, SettleFillsData, SettleResult::{self}, ValidateAndLockData, ValidateAndLockResponse, publisher::{NullPublisher, RedisPublisher, ResultPublisher}}, types::{Code::{self, InsufficientFunds, 
    InvalidMarket, InvalidPriceOrQuantity, ServerError}, GetBalance, GetBalanceResponse, GetOpenOrderPayload, MessageFromApi, 
    MessageToApi, OrderCancelledPayload, OrderPlacedPayload, OrderSide, ResponsePayload, TradePublishData, WsPublisherActions }};

#[derive(Debug, Serialize)]
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
    pub balance_sender: Sender<BalanceActions>,
    pub redis_conn: MultiplexedConnection,
}

impl Engine {
    pub fn new(redis_conn: MultiplexedConnection, redis_client: redis::Client, is_benchmark: bool) -> Engine{ 
        let balance_publisher : Box<dyn ResultPublisher> = if is_benchmark { // if benchmark test
            // set NullPublisher
            Box::new(NullPublisher)
        } else {
            // set PublishToRedis
            Box::new(RedisPublisher::new(redis_client.clone()))
        };

        let ws_publisher: Box<dyn ResultPublisher> = if is_benchmark {
            Box::new(NullPublisher)
        } else {
            Box::new(RedisPublisher::new(redis_client.clone()))
        };

        let balance_sender = spawn_balance_thread(balance_publisher);
        let ws_publisher_sender  = spawn_ws_publisher(ws_publisher);

        let mut market_senders = HashMap::new();
        for market in MARKETS {
            let market_publisher: Box<dyn ResultPublisher> = if is_benchmark {
                Box::new(NullPublisher)
            } else {
                Box::new(RedisPublisher::new(redis_client.clone()))
            };
            let market_tx = spawn_market_thread(market, market_publisher, balance_sender.clone(), ws_publisher_sender.clone());
            market_senders.insert(market, market_tx);
        };

        Engine {
            market_senders,
            balance_sender,
            redis_conn: redis_conn // this redis conn is only for engine to do redis connection in
            // async
        }
    }

    pub async fn process(&self, message: MessageFromApi, client_id: String) {
        println!("process(), ran");
        match message {
            MessageFromApi::CreateOrder(payload) => {
                let market = payload.symbol;
                let orderbook = self.market_senders.get(&market);

                match orderbook {
                    Some(sender)  => {
                        if let Err(e) = sender.send(OrderbookActions::CreateOrder(payload, client_id.clone())) {
                            println!("market thread unreachable {}", e);
                            // early return to api. order rejected, market unavailable
                        self.publish_rejection(client_id, "market unavailable. please try later".to_string(), ServerError).await;
                        };
                    },
                    None => {
                        // return order rejected. invalid market
                        self.publish_rejection(client_id, "market does not exist. Please enter valid market".to_string(), InvalidMarket).await;
                    }
                };
            },
            MessageFromApi::CancelOrder(payload) => {
                let market = payload.symbol;
                let orderbook = self.market_senders.get(&market);

                match orderbook {
                    Some(sender)  => {
                        if let Err(e) = sender.send(OrderbookActions::CancelOrder(payload, client_id.clone())) {
                            println!("market thread unreachable {}", e);
                            // early return to api. order rejected, market unavailable
                            self.publish_rejection(client_id, "market unavailable. please try later".to_string(), ServerError).await;
                        };
                    },
                    None => {
                        // return order rejected. invalid market
                        self.publish_rejection(client_id, "market does not exist. Please enter valid market".to_string(), InvalidMarket).await;
                    }
                }
            },
            MessageFromApi::Onramp(payload) => { // increase the usdc amount for this user
                let _ = self.balance_sender.send(BalanceActions::Onramp(payload.amount, payload.user_id, client_id));
            },
            MessageFromApi::Deposit(payload) => { //
                let _ = self.balance_sender.send(BalanceActions::Deposit(payload.asset, payload.quantity, payload.user_id, client_id));
            },
            MessageFromApi::GetOpenOrders(payload) => {
                let orderbook = self.market_senders.get(&payload.symbol);

                match orderbook {
                    Some(sender) => {
                        if let Err(e) = sender.send(OrderbookActions::GetOpenOrders(payload, client_id.clone())) {
                            eprintln!("market thread unreachable {}", e);
                        self.publish_rejection(client_id, "market unavailable. please try later".to_string(), ServerError).await;
                        }
                    },
                    None => {
                        // return invalid market
                        self.publish_rejection(client_id, "market does not exist. Please enter valid market".to_string(), InvalidMarket).await;
                    }

                };
            },
            MessageFromApi::GetDepth(market) => {
                let orderbook = self.market_senders.get(&market);

                match orderbook {
                    Some(sender) => {
                        let _ = sender.send(OrderbookActions::GetDepth(client_id));
                    },
                    None => {
                        // return invalid market
                        self.publish_rejection(client_id, "market does not exist. Please enter valid market".to_string(), InvalidMarket).await;
                    }

                };
            },
            MessageFromApi::GetBalance(payload) => {
                println!("get balance MessageFromApi arm ");
                let _ = self.balance_sender.send(BalanceActions::GetBalance(payload.asset, payload.user_id, client_id));
            }
        };
    }

    async fn publish_rejection(&self, client_id: String, message: String, code: Code) {
        let mut conn = self.redis_conn.clone(); // multiplex clone is cheap
        
        let payload = serde_json::to_string(&MessageToApi::OrderRejected(ResponsePayload {
            code: code,
            message
        })).expect("serde error");

        let _: Result<(), _> = conn.publish(client_id, payload).await;
    }
}

fn spawn_balance_thread(mut balance_publisher: Box<dyn ResultPublisher>) -> Sender<BalanceActions> {
    let (tx, rx) = unbounded::<BalanceActions>();

    // User id -> assets
    // operations

    // spawn the balance thread
    thread::spawn(move|| {
        let mut balances: FxHashMap<(Uuid, Asset), Balance> = FxHashMap::default();
        
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
                                if val.available < data.quantity { // this is ask. just
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
                    println!("balance after /create order {:?}", balances);
                },
                BalanceActions::SettleFills(data) => {
                    if data.side == OrderSide::Buy {
                        for fill in &data.fills {
                            // first decrese the quote locked balance of taker
                            let notional = calc_quote_amount(fill.price, fill.quantity);
                            if let Err(err) = notional {
                                    println!("overflow error {}", err);
                                    let _ = data.resp.send(SettleResult::Overflow);
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
                                    let _ = data.resp.send(SettleResult::Overflow);
                                    continue;
                            };
                            // decrease seller locked base asset  (taker)
                            if let Some(bal) = balances.get_mut(&(fill.user_id, data.base_asset)) {
                                println!("decreseing seller locked base asset by {}", fill.quantity);
                                bal.locked -= fill.quantity;
                            }

                            // increase seller quote asset avlbl amount
                            println!("sell /order increasing seller quote amount {}", notional.unwrap());
                            balances.entry((fill.user_id, data.quote_asset)).or_insert(Balance::default()).available += notional.unwrap();

                            // decrease maker locked quote asset amount
                            if let Some(bal) = balances.get_mut(&(fill.other_user_id, data.quote_asset)) {
                                println!("decreasing maker (bid) locked quote asset {}", notional.unwrap());
                                bal.locked -= notional.unwrap();
                            }

                            // increase maker base asset avlbl amount
                            println!("increaesing maker base asset avlbl {}", fill.quantity);
                            balances.entry((fill.other_user_id, data.base_asset)).or_insert(Balance::default()).available += fill.quantity;
                        }
                    }
                    // send Settle::Success
                    let _ = data.resp.send(SettleResult::Success);
                },
                BalanceActions::CancelAndUpdateBalance(order, symbol, resp) => {
                    if order.order_side == OrderSide::Buy {
                        let notional = calc_quote_amount(order.price, order.quantity - order.filled);
                        match notional {
                            Ok(left_qty_price) => {
                                if let Some(bal) = balances.get_mut(&(order.user_id, symbol.quote)) {
                                    bal.available += left_qty_price;
                                    bal.locked -= left_qty_price;
                                    let _ = resp.send(SettleResult::Success);
                                }
                            },
                            Err(e) => {
                                eprintln!("overflow error {}", e);
                                let _ = resp.send(SettleResult::Overflow);
                            }
                        };
                    } else { // if sell was cancelled increase base_asset avlbl amount
                        if let Some(bal) = balances.get_mut(&(order.user_id, symbol.base)) {
                            println!("bal in just starting {:?}", bal );
                            println!("order quantity: {} and orer filled quantity: {}", order.quantity, order.filled);
                            bal.available += order.quantity - order.filled;
                            println!("bal before changing locked{:?}", bal);
                            println!("quantity to be removed{:?}", order.quantity - order.filled);
                            bal.locked -= order.quantity - order.filled;
                            let _ = resp.send(SettleResult::Success);
                        }
                    }
                },
                BalanceActions::Onramp(amount, user_id, client_id) => {
                 balances.entry((user_id, Asset::USDC)).or_insert(Balance::default()).available += amount; 
                    println!("balance after /onramp order {:?}", balances);
                    // send back to pub sub successfull
                    let payload = serde_json::to_string(&MessageToApi::OnrampResponse(ResponsePayload{
                        code: Code::OnrampSuccess,
                        message: format!("onramp successfull amount {}", amount)
                    })).expect("serde error");

                    balance_publisher.publish(client_id, payload);
                },
                BalanceActions::Deposit(asset, qty, user_id, client_id) => {
                 balances.entry((user_id, asset)).or_insert(Balance::default()).available += qty; 
                 // send back to pub sub successfull
                    println!("balance after /deposit order {:?}", balances);
                let payload = serde_json::to_string(&MessageToApi::DepositResponse(ResponsePayload{
                        code: Code::DepositSuccess,
                        message: format!("Deposit successfull amount {}", qty)
                    })).expect("serde error");
                    balance_publisher.publish(client_id, payload);
                },
                BalanceActions::GetBalance(asset, user_id, client_id) => {
                    match balances.get(&(user_id,asset)){
                        Some(bal) => {
                            let parsed_avlbl = u64_to_string(bal.available);
                            let parsed_locked = u64_to_string(bal.locked);


                            if let (Ok(available), Ok(locked)) = (parsed_avlbl, parsed_locked) {
                                let payload = serde_json::to_string(&MessageToApi::GetBalance(GetBalanceResponse {
                                    asset,
                                    balance:GetBalance {
                                        available,
                                        locked
                                    } 
                                })).expect("serde error");
                            balance_publisher.publish(client_id, payload);
                            } else {
                                eprintln!("failed to parse user balance");
                            }
                        }, 
                        None => {
                            // send back amount 0
                            println!("None arm ran");
                            let payload = serde_json::to_string(&MessageToApi::GetBalance(GetBalanceResponse{
                                asset,
                                balance:GetBalance {
                                    available: "0".to_string(),
                                    locked: "0".to_string()
                                } 
                            })).expect("serde error");
                            balance_publisher.publish(client_id, payload);
                        }
                    }
                }
            }; 
        }
        });
    tx
}

fn spawn_market_thread(market:Market, mut market_publisher: Box<dyn ResultPublisher>,  balance_trasmitter: Sender<BalanceActions>, ws_publisher:Sender<WsPublisherActions>) -> Sender<OrderbookActions>{
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
               OrderbookActions::CreateOrder(payload, client_id) => {
                    // validate and lock funds
                    println!("order came {:?}" , payload.order_side);
                    let res = validate_and_lock(payload.user_id, payload.price, payload.quantity, 
                        payload.order_side.clone(), payload.symbol, balance_trasmitter.clone());

                    println!("create order arm after validate lock {:?}" , payload.order_side);

                    match res {
                        Ok(ValidateAndLockResponse::Success) => { // if success create order and
                            println!("Success validate lock arm {:?}" , payload.order_side);
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
                            let (executed_qty, order_status, fills, order_id) = orderbook.add_order(order); // get the fills and change balances
                            println!("after add_order{:?}" , payload.order_side);
                            let _ = balance_trasmitter.clone().send(BalanceActions::SettleFills(SettleFillsData {
                                fills: fills.clone(),
                                side: payload.order_side.clone(),
                                base_asset: payload.symbol.base,
                                quote_asset: payload.symbol.quote,
                                resp: balance_settle_tx
                            }));

                            if let Ok(val) = balance_settle_rx.recv() { // wait till balance thread
                                // update funds
                                if val == SettleResult::Success {
                                    // return order success created with executed_qty and
                                    // order_status
                                    let res_payload = serde_json::to_string(&MessageToApi::OrderPlaced(OrderPlacedPayload {
                                        order_id: order_id,
                                        executed_quantity: executed_qty,
                                        order_status
                                    })).expect("serde error");

                                    market_publisher.publish(client_id, res_payload);
                                    // this thread publishes to client directly- no round trip to engine

                                    // publish trade update 
                                    for fill in fills {
                                        let payload = TradePublishData {
                                            symbol:payload.symbol,
                                            price: fill.price,
                                            quantity: fill.quantity,
                                            order_side: payload.order_side.clone(),
                                            other_user_id: fill.other_user_id,
                                            trade_id: fill.trade_id
                                        };
                                        let _ = ws_publisher.send(WsPublisherActions::PubishTrade(payload));
                                    };
                                    

                                }
                            } else {
                                println!("balance thread recv Error");
                            }
                        },
                        Ok(ValidateAndLockResponse::InsufficientFunds) => {
                            // reject order 
                            let payload = serde_json::to_string(&MessageToApi::OrderRejected(ResponsePayload {
                                code: InsufficientFunds,
                                message: "Please deposit some asset to trade".to_string()
                            })).expect("serde error");

                            market_publisher.publish(client_id, payload);
                        },
                        Ok(ValidateAndLockResponse::Overflow) => {
                        let payload = serde_json::to_string(&MessageToApi::OrderRejected(ResponsePayload {
                                code: InvalidPriceOrQuantity,
                                message: "Please enter valid price or quantity".to_string()
                            })).expect("serde error");
                            market_publisher.publish(client_id, payload);
                        }
                        Err(err) => {
                            eprintln!("balance thread is unreachable, recv error {}", err);
                        }
                    };
               },
                OrderbookActions::CancelOrder(payload, client_id) => {
                    let res = orderbook.cancel_order(payload.order_id, payload.user_id);
                    match res {
                        Ok(cancelled_order)  => { // if removed successfully. then update balances
                            let (update_balance_tx, update_balance_rx) = bounded::<SettleResult>(1);

                            let _ = balance_trasmitter.clone().send(BalanceActions::CancelAndUpdateBalance(cancelled_order.clone(), payload.symbol, update_balance_tx));
                            if let Ok(val) = update_balance_rx.recv() { // wait till balance thread
                                if val == SettleResult::Success {
                                    // return cancelled_order with order_id and remaining quantity
                                    let payload = serde_json::to_string(&MessageToApi::OrderCancelled(OrderCancelledPayload {
                                        order_id: cancelled_order.order_id,
                                        executed_quantity: cancelled_order.filled,
                                        order_status: OrderStatus::Cancelled,
                                        quantity: cancelled_order.quantity
                                    })).expect("serde error");
                            market_publisher.publish(client_id, payload);
                                }
                            } else {
                                println!("balance thread recv Error");
                            }
                        },
                        Err(e) => { 
                            // send response back with acutual erro message
                            let payload = serde_json::to_string(&MessageToApi::CancelRejected(ResponsePayload{
                                code: e.clone().into(), // implemented From Error for Code
                                message: format!("{}",e)
                            })).expect("serde error");
                            market_publisher.publish(client_id, payload);
                        }

                    };
                },
                OrderbookActions::GetDepth(client_id)=> { // ideally we should change the way to get depth 
                    // on starting as every order comes the http gets subs to engine and engine
                    // pubs every delta of order/cancel . http maintains local in memory depth
                    let depth = orderbook.get_depth();
                    // publish
                    let payload = serde_json::to_string(&MessageToApi::GetDepth(depth)).expect("serde error");
                            market_publisher.publish(client_id, payload);
                },
                OrderbookActions::GetOpenOrders(payload, client_id) => {
                    let open_orders = orderbook.get_open_orders(payload.user_id);
                    // publish
                    let payload = serde_json::to_string(&MessageToApi::GetOpenOrders(GetOpenOrderPayload {
                        orders :open_orders
                    })).expect("serde error");
                    market_publisher.publish(client_id, payload);
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

pub fn u64_to_string(value: u64) -> Result<String, &'static str> {
    let scale_factor = Decimal::from_u64(SCALE_FACTOR)
        .ok_or("Failed to convert scale_factor to decimal")?;

    let dec = Decimal::from_u64(value)
        .ok_or("Failed to convert value to decimal")?
        / scale_factor;

    Ok(dec.normalize().to_string())
}

fn spawn_ws_publisher(mut ws_publisher: Box<dyn ResultPublisher>) -> Sender<WsPublisherActions> {
    let (tx,rx) = unbounded::<WsPublisherActions>();

    thread::spawn(move || {
        while let Ok(val) = rx.recv() {
            match val {
                WsPublisherActions::PubishTrade(data) => {
                    let payload = serde_json::to_string(&data).expect("error while Serialize");
                    // get the trade.<symbol>
                    let channel = format!("trade.{}_{}", data.symbol.base.as_str(), data.symbol.quote.as_str());
                    ws_publisher.publish(channel, payload);
                },
                WsPublisherActions::DepthUpdate(data) => {
                    let payload = serde_json::to_string(&data).expect("error while Serialize");
                    // get the depth.<symbol>
                    let channel = format!("depth.{}_{}", data.symbol.base.as_str(), data.symbol.quote.as_str());
                    ws_publisher.publish(channel, payload);
                }
            }

        }
    });

    tx
}
