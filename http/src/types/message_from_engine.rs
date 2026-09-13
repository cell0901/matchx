use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{routes::Asset, types::message_to_engine::{OrderSide, OrderType}};

#[derive(Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    Filled,
    PartiallyFilled,
    Cancelled,
}

#[derive(Serialize, Deserialize)]
pub struct Fill{
    pub price: u64,
    pub quantity: u64,
    pub side: OrderSide,
    pub other_user_id: String
}

#[derive(Deserialize, Serialize )]
pub struct Order{
    pub order_id: Uuid,
    pub price: u64,
    pub user_id: Uuid,
    pub quantity: u64,
    pub order_side: OrderSide,
    pub order_type: OrderType, // mostly the order type will limit. order but we still added the
    // market. just in case
    pub filled: u64 // how much quantity filled
}


#[derive(Serialize, Deserialize)]
pub struct OrderPlacedPayload{
    pub order_id: Uuid,
    pub executed_quantity: u64,
    // pub fills: Vec<Fill>,
    pub order_status: OrderStatus
}

#[derive(Serialize, Deserialize)]
pub struct OrderCancelledPayload{
    pub order_id: Uuid,
    pub executed_quantity: u64,
    pub order_status: OrderStatus,
    pub quantity: u64
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

#[derive(Deserialize, Serialize)]
pub struct DepthLevel {
    pub price: String,
    pub quantity: String 
}

#[derive(Serialize, Deserialize)]
pub struct ResponsePayload {
    pub code: Code,
    pub message: String
}
#[derive(Deserialize, Serialize)]
pub struct DepthResponse {
    pub bids : Vec<DepthLevel>,
    pub asks : Vec<DepthLevel>
}

#[derive(Serialize, Deserialize)]
pub struct GetOpenOrderPayload {
    pub orders: Vec<Order>
}

#[derive(Serialize, Deserialize)]
pub struct GetBalance {
    pub available: String,
    pub locked: String
}

#[derive(Serialize, Deserialize)]
pub struct GetBalancePayload {
    pub asset: Asset,
    pub balance: GetBalance
}

#[derive(Serialize, Deserialize)]
pub enum MessageFromEngine {
    OrderPlaced(OrderPlacedPayload),
    OrderRejected(ResponsePayload),
    OrderCancelled(OrderCancelledPayload),
    CancelRejected(ResponsePayload),
    GetDepth(DepthResponse),
    GetOpenOrders(GetOpenOrderPayload),
    GetBalance(GetBalancePayload),
    OnrampResponse(ResponsePayload),
    DepositResponse(ResponsePayload)
}

#[derive(serde::Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Code {
    InsufficientFunds,
    InvalidPriceOrQuantity,
    OrderNotFound,
    Unauthorized,
    InvalidMarket,
    ServerError,
    OnrampSuccess,
    DepositSuccess
}
