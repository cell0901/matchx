use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::{OrderCancelError}, trade::Asset, types::{OrderSide, OrderType}};

pub type Price= u64;
pub type CurrentPrice= u64;
pub type UserId = Uuid;

pub struct Fill{
    pub price: u64,
    pub quantity: u64,
    pub user_id: Uuid,
    pub other_user_id: Uuid,
    pub trade_id: u64,
}

#[derive(Clone, PartialEq, Eq, Copy, Hash, Deserialize, Serialize)]
pub struct Market {
    pub base: Asset,
    pub quote: Asset
}

#[derive(Clone, Serialize, Debug)]
pub struct Order{
    pub order_id: Uuid,
    pub price: Price,
    pub user_id: Uuid,
    pub quantity: u64,
    pub order_side: OrderSide,
    pub order_type: OrderType, // mostly the order type will limit. order but we still added the
    // market. just in case
    pub filled: u64 // how much quantity filled
}


#[derive(Serialize, Deserialize)]
pub enum OrderStatus {
    New, // no match found sitting on orderbook
    Filled,
    PartiallyFilled,
    Cancelled,
}

pub struct Orderbook{
    pub market: Market, // SOL, BTC etc
    pub bids: BTreeMap<Price, VecDeque<Order>>, //  but this needs to be .iter().rev() 
    pub asks: BTreeMap<Price, VecDeque<Order>>, // this is ok key is lowest to highest
    pub current_price: CurrentPrice,
    pub last_trade_id: u64, // ++ on each trade. (match)
    // Order ID  -> order location
    pub order_index: HashMap<Uuid, OrderLocation>, // to find and remove fast
    pub user_orders: HashMap<UserId, HashSet<Uuid>> // userId -> order Ids
}

pub struct OrderLocation{
    pub side: OrderSide,
    pub price: Price
}
#[derive(Serialize)]
pub struct DepthLevel {
    pub price: String,
    pub quantity: String 
}

#[derive(Serialize)]
pub struct DepthResponse {
    pub bids : Vec<DepthLevel>,
    pub asks : Vec<DepthLevel>
}

impl Orderbook{
   pub fn new (market: Market) -> Self {
        Orderbook {
            market,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            current_price: 0,
            last_trade_id: 0,
            order_index: HashMap::new(),
            user_orders: HashMap::new()
        }
   }

    pub fn add_order(&mut self, order: Order) -> (u64, OrderStatus, Vec<Fill>, Uuid) {
       match order.order_type {
            OrderType::Limit => {
                return self.fill_limit_order(order)
            },
            OrderType::Market => {
                // IOC. if no order otherside then Cancelled. else partial and full fills.
                println!("market order hit. implement function for this");
                return  (4 as u64, OrderStatus::Cancelled, vec![], Uuid::now_v7())
            }
       };
    }

    fn fill_limit_order(&mut self, order: Order) -> (u64, OrderStatus, Vec<Fill>, Uuid){
        match order.order_side {
            OrderSide::Buy => {
                let mut order_status = OrderStatus::New;
                let (executed_qty, fills) = self.match_bid(order.clone());
                
                if executed_qty == order.quantity {
                    // return executed_quantity nd order status (Filled)
                    return (executed_qty, OrderStatus::Filled, fills, order.order_id);
                }

                let final_order = Order {
                    order_id: order.order_id,
                    price: order.price,
                    user_id: order.user_id,
                    quantity: order.quantity,
                    order_side: order.order_side.clone(),
                    order_type: order.order_type,
                    filled: executed_qty
                };

                // if value for this key doesnt exist then insert with empty VecDeque. else return
                // with mutable ref to the value
                let a = self.bids.entry(order.price)
                    .or_insert_with(|| VecDeque::new());
                a.push_back(final_order); // for same price the new order will be at last. and
                // will we will pop from front
                self.order_index.insert(order.order_id, OrderLocation { side: order.order_side, price: order.price });
                // create the HashSet if doesnt exist and insert with order_id
                self.user_orders.entry(order.user_id).or_default().insert(order.order_id);
                if executed_qty > 0 {
                    order_status = OrderStatus::PartiallyFilled;
                } 
                let json= serde_json::to_string_pretty(&self.bids);
                let json2=  serde_json::to_string_pretty(&self.asks);
                println!("orderbook after buy /order bids: {}", json.unwrap());
                println!("orderbook after buy /order asks: {}", json2.unwrap());
                return (executed_qty , order_status, fills, order.order_id);
            },
            OrderSide::Sell => {
                let mut order_status = OrderStatus::New;

                println!("sell order came {}", order.quantity);
                let (executed_qty, fills) = self.match_ask(order.clone()); 
                println!("sell order after match ask{}", order.quantity);
                if executed_qty == order.quantity {
                    return (executed_qty, OrderStatus::Filled, fills, order.order_id);
                }
                let final_order = Order {
                    order_id: order.order_id,
                    price: order.price,
                    user_id: order.user_id,
                    quantity: order.quantity,
                    order_side: order.order_side.clone(),
                    order_type: order.order_type,
                    filled: executed_qty
                };
                let a = self.asks.entry(order.price)
                    .or_insert_with(|| VecDeque::new());
                a.push_back(final_order); 
                self.order_index.insert(order.order_id, OrderLocation { side: order.order_side, price: order.price });
                self.user_orders.entry(order.user_id).or_default().insert(order.order_id);
                if executed_qty > 0 {
                    order_status = OrderStatus::PartiallyFilled;
                } 
                let json= serde_json::to_string_pretty(&self.bids);
                let json2=  serde_json::to_string_pretty(&self.asks);
                println!("orderbook after sell /order bids: {}", json.unwrap());
                println!("orderbook after sell /order asks: {}", json2.unwrap());
                return (executed_qty, order_status, fills, order.order_id);
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

                self.last_trade_id += 1; // on each fill 
                //
                fills.push(Fill {
                    price: best_ask_price,
                    quantity: trade_qty,
                    user_id: order.user_id,
                    other_user_id: resting_ask.user_id,
                    trade_id: self.last_trade_id
                });
                
                
                remaining_quantity -= trade_qty; // decrease the remaining_quantity
                resting_ask.filled += trade_qty; // increase the other user filled qty

                if resting_ask.quantity == resting_ask.filled { // fully filled resting order.
                    // remove from order_index
                    self.order_index.remove(&resting_ask.order_id);
                    // remove the order id from the HashSet
                    if let Some(orders) = self.user_orders.get_mut(&resting_ask.user_id) {
                        orders.remove(&resting_ask.order_id);
                    };
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

                self.last_trade_id +=1;

                fills.push(Fill {
                    price: best_bid_price,
                    quantity: trade_qty,
                    user_id: order.user_id,
                    other_user_id: resting_bid.user_id,
                    trade_id: self.last_trade_id
                });

                self.last_trade_id += 1; // on each fill
                
                remaining_quantity -= trade_qty;
                println!("sell order resting_bid: {:?} and trade_qty {:?}", resting_bid, trade_qty);
                resting_bid.filled += trade_qty;

                if resting_bid.quantity == resting_bid.filled { // fully filled resting order.
                    // remove from order_index
                    self.order_index.remove(&resting_bid.order_id);
                    if let Some(orders) = self.user_orders.get_mut(&resting_bid.user_id) {
                        orders.remove(&resting_bid.order_id);
                    };
                    level.pop_front();
                }
            }

            if level.is_empty() { 
                self.bids.remove(&best_bid_price); // remove the bid
            }
        }
        (order.quantity - remaining_quantity, fills) // executed_quantity and fills
    }

    pub fn cancel_order(&mut self, order_id: Uuid, user_id: Uuid ) -> Result<Order, OrderCancelError> { // returnns
        // filled quantity
        println!("cancel_order engine hit");
        let location = self.order_index.get(&order_id).ok_or(OrderCancelError::OrderNotFound)?;

        let cancelled_order;
        let side = match location.side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell=> &mut self.asks,
        };

        println!("after finding side");

        let level = side.get_mut(&location.price).ok_or(OrderCancelError::OrderNotFound)?; // this should
        // exist. because we already add a check before to check in order_index

        println!("after finding level");
        // in that level find the order_id index that matches
        let pos = level.iter().position(|o| o.order_id == order_id);

        match pos {
            Some(index) => {
                let order = &level[index];
                println!("the cancel_order order {:?}", order);
                if order.user_id != user_id {
                    return  Err(OrderCancelError::Unauthorized); // dont let other users cancel other's
                    // orders
                }
                cancelled_order = order.clone();
                level.remove(index); // remove from VecDeque
            },
            None =>  {
                println!("inside none case");
                return  Err(OrderCancelError::OrderNotFound);
            }
        }

        if level.is_empty() {
            side.remove(&location.price); // if after removing the order. the level is empty then
            // remove the price level
        }

        // also clean order_index entry
        self.order_index.remove(&order_id);
        if let Some(orders) = self.user_orders.get_mut(&user_id) {
                orders.remove(&cancelled_order.order_id);
        };

        Ok(cancelled_order)
    }

    pub fn get_depth(&self) -> DepthResponse {
        let bids :Vec<DepthLevel>  = self.bids.iter().rev().map(|(price, orders)| { // for every diff price
            // calculate total qty of orders
            let total_qty: u64 = orders.iter().map(|o| o.quantity - o.filled).sum();
            DepthLevel { // on each iter returns this 
                price: price.to_string(),
                quantity: total_qty.to_string()
            }
        }).collect(); 

        let asks: Vec<DepthLevel> = self.asks.iter().map(|(price,orders)| {
            let total_qty: u64 = orders.iter().map(|o| o.quantity - o.filled).sum();
            DepthLevel {
                price: price.to_string(),
                quantity: total_qty.to_string()
            }
        }).collect();

        DepthResponse {
            bids,
            asks
        }
    }

    pub fn get_open_orders(&self, user_id: Uuid) -> Vec<Order> {
        let Some(order_ids) = self.user_orders.get(&user_id) else {
            return vec![]; // if not entry for this userId means no orders return empty vec
        };

        order_ids.iter().filter_map(|id| {
            let loc = self.order_index.get(id)?;
            let side = match loc.side {
                OrderSide::Buy => &self.bids,
                OrderSide::Sell=> &self.asks,
            };

            // ? this returns an option
            // fix this only returns one side orders 
            side.get(&loc.price)?.iter().find(|o|  o.user_id == user_id).cloned()
        }).collect()
    }

}
