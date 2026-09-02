use serde::{Deserialize, Serialize};

use crate::types::message_to_engine::OrderSide;

#[derive(Serialize, Deserialize)]
pub struct Fill{
    pub price: u64,
    pub quantity: u64,
    pub side: OrderSide,
    pub other_user_id: String
}

#[derive(Serialize, Deserialize)]
pub struct Order{
    pub user_id: String,
    pub price: u64,
    pub quantity: u64,
    pub side: OrderSide,
    pub filled: u64,
    pub order_id: String
}


#[derive(Serialize, Deserialize)]
pub struct OrderPlacedPayload{
    pub order_id: String,
    // pub executed_quantity: u64,
    pub fills: Vec<Fill>,
}

#[derive(Serialize, Deserialize)]
pub struct OrderCancelledPayload{
    pub order_id: String,
    pub executed_quantity: u64,
    pub remaining_quantity: u64
}

#[derive(Serialize, Deserialize)]
pub struct GetOpenOrdersPayload{
    pub orders: Vec<Order>
}

#[derive(Serialize, Deserialize)]
pub struct OnrampResponse{
    pub message: String // onramp or deposit successfull
}

#[derive(Serialize, Deserialize)]
pub struct GetBalanceResponse{
    pub balance: u64
}

#[derive(Serialize, Deserialize)]
pub enum MessageFromEngine {
    OrderPlaced(OrderPlacedPayload),
    OrderCancelled(OrderCancelledPayload),
    GetOpenOrders(GetOpenOrdersPayload),
    Onramp(OnrampResponse), //  both for deposit and onramp initially
    GetBalance(GetBalanceResponse)
    // GET_DEPTH
}
