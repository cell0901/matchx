use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::OrderCancelError, trade::{Asset, SCALE_FACTOR}, types::{OrderSide, OrderType}};

pub type Price= u64;
pub type CurrentPrice= u64;
pub type UserId = Uuid;

#[derive(Clone)]
pub struct Fill{
    pub price: u64,
    pub quantity: u64,
    pub user_id: Uuid,
    pub other_user_id: Uuid,
    pub trade_id: u64,
    pub maker_order_id: Uuid,
    pub maker_order_side: OrderSide,
    pub maker_order_type: OrderType,
    pub maker_order_quantity: u64,
    pub maker_filled_quantity: u64,
}

#[derive(Clone, PartialEq, Eq, Copy, Hash, Deserialize, Serialize, Debug)]
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

#[derive(Deserialize, Serialize, Debug)]
pub struct DepthDelta {
    pub side: OrderSide,
    pub price: Price,
    pub new_total_qty: u64 // 0 means the level is removed from orderbook, in the frontend the level
                           // should be removed
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

    pub fn add_order(&mut self, order: Order) -> (u64, OrderStatus, Vec<Fill>, Uuid, Vec<DepthDelta>) {
       match order.order_type {
            OrderType::Limit => {
                return self.fill_limit_order(order)
            },
            OrderType::Market => {
                // IOC. if no order otherside then Cancelled. else partial and full fills.
                println!("market order hit");
                return self.fill_market_order(order)
            }
       };
    }

    fn fill_market_order(&mut self, order: Order) -> (u64, OrderStatus, Vec<Fill>, Uuid, Vec<DepthDelta>) {
        match order.order_side {
            OrderSide::Buy => {
                // simulation already confirmed full liquidity exists — reuse match_bid as-is,
                // since it already walks and consumes asks the same way
                let (executed_qty, fills, deltas) = self.match_bid(order.clone());
                // executed_qty should equal order.quantity by construction (simulated first)
                (executed_qty, OrderStatus::Filled, fills, order.order_id, deltas)
            }
            OrderSide::Sell => {
                let (executed_qty, fills, deltas) = self.match_ask(order.clone());
                (executed_qty, OrderStatus::Filled, fills, order.order_id, deltas)
            }
        }
    }

    // first simulate if enough quantity instead directly modifying the orderbook
      pub fn simulate_market_buy(&self, quantity: u64) -> Option<u64> { // returns total_cost for order
                                                                    // if enough quantity available
        let mut remaining = quantity;
        let mut total_cost: u128 = 0;

        for (&price, level) in self.asks.iter() { // no need to sort as asks are already lowest to highest
            for order in level.iter() {
                if remaining == 0 { break; }
                let available = order.quantity - order.filled;
                let take = available.min(remaining);
                total_cost += (price as u128) * (take as u128);
                remaining -= take;
            }
            if remaining == 0 { break; }
        }

        if remaining > 0 {
            None // not enough liquidity in the whole book
        } else {
            // descale: price and qty are both fixed-point scaled, so their product is double-scaled
            u64::try_from(total_cost / SCALE_FACTOR as u128).ok()
        }
    }

      pub fn has_enough_bid_liquidity(&self, quantity: u64) -> bool{
          let mut remaining = quantity;

          for level in self.bids.values().rev() {
              for order in level.iter() {
                  if remaining == 0 {return true;}
                  remaining = remaining.saturating_sub(order.quantity - order.filled); // saturating_sub
                // doenst goes below zero
              }

          }
          remaining == 0 // returns true if remaining equals zero

      }

    fn fill_limit_order(&mut self, order: Order) -> (u64, OrderStatus, Vec<Fill>, Uuid, Vec<DepthDelta>){
        match order.order_side {
            OrderSide::Buy => {
                let mut order_status = OrderStatus::New;
                let (executed_qty, fills, mut deltas) = self.match_bid(order.clone());
                
                if executed_qty == order.quantity {
                    // return executed_quantity nd order status (Filled)
                    return (executed_qty, OrderStatus::Filled, fills, order.order_id, deltas);
                }

                // if all order quantity is not filled. then insert to bids
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
                let level = self.bids.entry(order.price)
                    .or_insert_with(|| VecDeque::new());
                level.push_back(final_order); // for same price the new order will be at last. and
                // will we will pop from front
                self.order_index.insert(order.order_id, OrderLocation { side: order.order_side, price: order.price });
                // create the HashSet if doesnt exist and insert with order_id
                self.user_orders.entry(order.user_id).or_default().insert(order.order_id);
                if executed_qty > 0 {
                    order_status = OrderStatus::PartiallyFilled;
                } 

                // if the order is PartiallyFilled then deltas of fills and this new order as as
                // delta should also be added 
                let level_orders = self.bids.get(&order.price).unwrap();
                let qty_at_level:u64 = level_orders.iter().map(|order| order.quantity -order.filled).sum();
                deltas.push(DepthDelta { side: OrderSide::Buy, price: order.price, new_total_qty: qty_at_level });

                // let json= serde_json::to_string_pretty(&self.bids);
                // let json2=  serde_json::to_string_pretty(&self.asks);
                // println!("orderbook after buy /order bids: {}", json.unwrap());
                // println!("orderbook after buy /order asks: {}", json2.unwrap());
                return (executed_qty , order_status, fills, order.order_id, deltas);
            },
            OrderSide::Sell => {
                let mut order_status = OrderStatus::New;

                println!("sell order came {}", order.quantity);
                let (executed_qty, fills, mut deltas) = self.match_ask(order.clone()); 
                println!("sell order after match ask{}", order.quantity);
                if executed_qty == order.quantity {
                    return (executed_qty, OrderStatus::Filled, fills, order.order_id, deltas);
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

                let level_orders = self.asks.get(&order.price).unwrap();

                let qty_at_level:u64 = level_orders.iter().map(|order| order.quantity - order.filled).sum();

                deltas.push(DepthDelta { side: OrderSide::Sell, price: order.price, new_total_qty: qty_at_level });

                // let json= serde_json::to_string_pretty(&self.bids);
                // let json2=  serde_json::to_string_pretty(&self.asks);
                // println!("orderbook after sell /order bids: {}", json.unwrap());
                // println!("orderbook after sell /order asks: {}", json2.unwrap());
                return (executed_qty, order_status, fills, order.order_id, deltas);
            }
        }
    }

    fn match_bid(&mut self, order: Order)-> ( u64, Vec<Fill>, Vec<DepthDelta>) {
        let mut fills: Vec<Fill> = Vec::new();  // maintain fills to send to it ws stream in future
        let mut remaining_quantity = order.quantity;
        let mut touched_prices: HashSet<Price> = HashSet::new();

        while remaining_quantity > 0 {
            let best_ask_price =match self.asks.keys().next(){
                Some(&price) => price,
                None => break,
            };
            if best_ask_price > order.price {
                break; // best ask too expensive break
            };

            touched_prices.insert(best_ask_price);
           
            // get the price level orders array
            let level = self.asks.get_mut(&best_ask_price).unwrap();
            
            while let Some(resting_ask) = level.front_mut() { // getting the front order mutable
                // ref. since we need to increase it filled quantity
                if remaining_quantity == 0 {
                    break; // everything filled
                }
                
                let trade_qty = remaining_quantity.min(resting_ask.quantity - resting_ask.filled);

                self.last_trade_id += 1; // on each fill 
                //
                fills.push(Fill {
                    price: best_ask_price,
                    quantity: trade_qty,
                    user_id: order.user_id,
                    other_user_id: resting_ask.user_id,
                    trade_id: self.last_trade_id,
                    maker_order_id: resting_ask.order_id,
                    maker_order_side: resting_ask.order_side.clone(),
                    maker_order_type: resting_ask.order_type.clone(),
                    maker_order_quantity: resting_ask.quantity,
                    maker_filled_quantity: resting_ask.filled + trade_qty,
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

        // for all touched price cal the depth Delta
        let deltas: Vec<DepthDelta> = touched_prices.into_iter().map(|price| {
            // gets the remaining_quantity for that price level users can trade with
            let qty = self.asks.get(&price).map(|orders| orders.iter().map(|order| order.quantity - order.filled).sum())
                .unwrap_or(0); // the value for this price value might not actualy exist so we
                               // handle the option with return 0
            DepthDelta {side: OrderSide::Sell, price: price, new_total_qty: qty} // since we are
                                                                                 // changing the asks
        }).collect();

        (order.quantity - remaining_quantity, fills, deltas) // executed_quantity and fills
        
    }

    fn match_ask(&mut self, order: Order) -> (u64, Vec<Fill>, Vec<DepthDelta>){
        let mut fills: Vec<Fill>  = Vec::new();
        let mut remaining_quantity = order.quantity;
        let mut touched_prices: HashSet<Price> = HashSet::new();

        while remaining_quantity > 0 {
            let best_bid_price = match self.bids.keys().next_back(){ // to get higest to lowest bid
                Some(&price) => price,
                None => break,
            };  
            
            if best_bid_price < order.price {
                break;
            }

            touched_prices.insert(best_bid_price);

            let level = self.bids.get_mut(&best_bid_price).unwrap();

            while let Some(resting_bid) = level.front_mut() {
                if remaining_quantity == 0 {
                    break;
                }  

                let trade_qty = remaining_quantity.min(resting_bid.quantity- resting_bid.filled);

                self.last_trade_id +=1;

                fills.push(Fill {
                    price: best_bid_price,
                    quantity: trade_qty,
                    user_id: order.user_id,
                    other_user_id: resting_bid.user_id,
                    trade_id: self.last_trade_id,
                    maker_order_id: resting_bid.order_id,
                    maker_order_side: resting_bid.order_side.clone(),
                    maker_order_type: resting_bid.order_type.clone(),
                    maker_order_quantity: resting_bid.quantity,
                    maker_filled_quantity: resting_bid.filled + trade_qty,
                });

                
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

        let deltas: Vec<DepthDelta> = touched_prices.into_iter().map(|price| {
                let qty = self.bids.get(&price).map(|orders| orders.iter().map(|order| order.quantity - order.filled).sum()).unwrap_or(0);

                DepthDelta {side: OrderSide::Buy, price, new_total_qty: qty}
            }).collect();

        (order.quantity - remaining_quantity, fills, deltas) // executed_quantity and fills
    }

    pub fn cancel_order(&mut self, order_id: Uuid, user_id: Uuid ) -> Result<(Order, DepthDelta), OrderCancelError> { // returnns
        // filled quantity
        let location = self.order_index.get(&order_id).ok_or(OrderCancelError::OrderNotFound)?;
        let price = location.price;
        let order_side  = location.side.clone();

        let cancelled_order;
        let side = match location.side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell=> &mut self.asks,
        };

        println!("after finding side");

        let level = side.get_mut(&location.price).ok_or(OrderCancelError::OrderNotFound)?; // this should
        // exist. because we already add a check before to check in order_index
        // in that level find the order_id index that matches
        let pos = level.iter().position(|o| o.order_id == order_id);

        match pos {
            Some(index) => {
                let order = &level[index];
                if order.user_id != user_id {
                    return  Err(OrderCancelError::Unauthorized); // dont let other users cancel other's
                    // orders
                }
                cancelled_order = order.clone();
                level.remove(index); // remove from VecDeque
            },
            None =>  {
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

        // compute remaining qty at this level after removal
        let remaining_qty: u64 = match order_side {
            OrderSide::Buy => self.bids.get(&price),
            OrderSide::Sell => self.asks.get(&price),
        }.map(|deque| deque.iter().map(|o| o.quantity - o.filled).sum()).unwrap_or(0);

        let delta = DepthDelta { side: order_side, price, new_total_qty: remaining_qty };

        Ok((cancelled_order, delta))
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
