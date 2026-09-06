use std::{collections::{BTreeMap, HashMap, VecDeque}};

use uuid::Uuid;

pub type Price= u64;
pub type CurrentPrice= u64;

pub struct Fill{
    pub price: u64,
    pub quantity: u64,
    pub user_id: String,
    pub other_user_id: String
}

#[derive(Clone)]
pub struct Order{
    pub order_id: Uuid,
    pub price: Price,
    pub user_id: String,
    pub quantity: u64,
    pub order_side: OrderSide,
    pub order_type: OrderType, // mostly the order type will limit. order but we still added the
    // market. just in case
    pub filled: u64 // how much quantity filled
}

#[derive(Clone)]
pub enum OrderType {
    Limit,
    Market
}
#[derive(Clone)]
pub enum OrderSide {
    Buy,
    Sell
}

pub enum OrderStatus {
    New, // no match found sitting on orderbook
    Filled,
    PartiallyFilled,
    Cancelled,
}

pub struct Orderbook{
    pub market: String, // SOL, BTC etc
    pub bids: BTreeMap<Price, VecDeque<Order>>, //  but this needs to be .iter().rev() 
    pub asks: BTreeMap<Price, VecDeque<Order>>, // this is ok key is lowest to highest
    pub current_price: CurrentPrice,
    pub last_trade_id: u64, // ++ on each trade. (match)
    // Order ID  -> order location
    pub order_index: HashMap<Uuid, OrderLocation> // to find and remove fast
}

pub struct OrderLocation{
    pub side: OrderSide,
    pub price: Price
}

impl Orderbook{
   pub fn new (market: String) -> Self {
        Orderbook {
            market,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            current_price: 0,
            last_trade_id: 0,
            order_index: HashMap::new()
        }
   }

    pub fn add_order(&mut self, order: Order) {
       match order.order_type {
            OrderType::Limit => {
                self.fill_limit_order(order);
            },
            OrderType::Market => {
                println!("market order hit. implement function for this")
            }
       };
    }

    fn fill_limit_order(&mut self, order: Order) -> (u64, OrderStatus){
        match order.order_side {
            OrderSide::Buy => {
                let mut order_status = OrderStatus::New;
                let (executed_qty, fills) = self.match_bid(order.clone());  // from all those
                
                // TODO ADD balance chagnes
                
                // do something with fills like balance changes etc
                if executed_qty == order.quantity {
                    // return executed_quantity nd order status (Filled)
                    return (executed_qty, OrderStatus::Filled);
                }

                // if value for this key doesnt exist then insert with empty VecDeque. else return
                // with mutable ref to the value
                let a = self.bids.entry(order.price)
                    .or_insert_with(|| VecDeque::new());
                a.push_back(order.clone()); // for same price the new order will be at last. and
                // will we will pop from front
                self.order_index.insert(order.order_id, OrderLocation { side: order.order_side, price: order.price });
                if executed_qty > 0 {
                    order_status = OrderStatus::PartiallyFilled;
                } 
                return (executed_qty , order_status);
            },
            OrderSide::Sell => {
                let mut order_status = OrderStatus::New;

                let (executed_qty, fills) = self.match_ask(order.clone()); 
                if executed_qty == order.quantity {
                    return (executed_qty, OrderStatus::Filled);
                }
                let a = self.bids.entry(order.price)
                    .or_insert_with(|| VecDeque::new());
                a.push_back(order.clone()); 
                self.order_index.insert(order.order_id, OrderLocation { side: order.order_side, price: order.price });
                if executed_qty> 0 {
                    order_status = OrderStatus::PartiallyFilled;
                } 
                return (executed_qty, order_status);
            }
        }
    }

    fn match_bid(&mut self, order: Order)-> ( u64, Vec<Fill>) {
        let mut fills: Vec<Fill> = Vec::new();  // maintain fills to send to it ws stream in future
        let mut remaining_quantity = order.quantity;

        while remaining_quantity > 0 {
            let best_ask_price =match self.asks.keys().next(){
                Some(&price) => price,
                None => break,
            };
            if best_ask_price > order.price {
                break; // best ask too expensive break
            };
           
            // get the price level orders array
            let level = self.asks.get_mut(&best_ask_price).unwrap();
            
            while let Some(resting_ask) = level.front_mut() { // getting the front order mutable
                // ref. since we need to increase it filled quantity
                if remaining_quantity == 0 {
                    break; // everything filled
                }
                
                let trade_qty = remaining_quantity.min(resting_ask.quantity);
                fills.push(Fill {
                    price: best_ask_price,
                    quantity: trade_qty,
                    user_id: order.user_id.clone(),
                    other_user_id: resting_ask.user_id.clone()
                });
                
                self.last_trade_id += 1; // on each fill
                
                remaining_quantity -= trade_qty; // decrease the remaining_quantity
                resting_ask.filled += trade_qty; // increase the other user filled qty

                if resting_ask.quantity == resting_ask.filled { // fully filled resting order.
                    // remove from order_index
                    self.order_index.remove(&resting_ask.order_id);
                    // remove it
                    level.pop_front();
                }

            }
           if level.is_empty() { // means no asks currrent at this price
                self.asks.remove(&best_ask_price); // no asks left at this price. drop the level
            }
        }

        (order.quantity - remaining_quantity, fills) // executed_quantity and fills
        
    }

    fn match_ask(&mut self, order: Order) -> (u64, Vec<Fill>){
        let mut fills: Vec<Fill>  = Vec::new();
        let mut remaining_quantity = order.quantity;

        while remaining_quantity > 0 {
            let best_bid_price = match self.bids.keys().next_back(){ // to get higest to lowest bid
                Some(&price) => price,
                None => break,
            };  
            
            if best_bid_price < order.price {
                break;
            }

            let level = self.bids.get_mut(&best_bid_price).unwrap();

            while let Some(resting_bid) = level.front_mut() {
                if remaining_quantity == 0 {
                    break;
                }  

                let trade_qty = remaining_quantity.min(resting_bid.quantity);
                fills.push(Fill {
                    price: best_bid_price,
                    quantity: trade_qty,
                    user_id: order.user_id.clone(),
                    other_user_id: resting_bid.user_id.clone()
                });

                self.last_trade_id += 1; // on each fill
                
                remaining_quantity -= trade_qty;
                resting_bid.filled += trade_qty;

                if resting_bid.quantity == resting_bid.filled { // fully filled resting order.
                    // remove from order_index
                    self.order_index.remove(&resting_bid.order_id);
                    // remove it
                    level.pop_front();
                }
            }

            if level.is_empty() { 
                self.asks.remove(&best_bid_price);
            }
        }
        (order.quantity - remaining_quantity, fills) // executed_quantity and fills
    }
}
