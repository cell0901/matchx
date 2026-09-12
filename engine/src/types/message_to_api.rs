use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::trade::{Asset, Balance, DepthResponse, Order, OrderStatus};

#[derive(Serialize, Deserialize)]
pub struct OrderCancelledPayload{
    pub order_id: Uuid,
    pub executed_quantity: u64,
    pub order_status: OrderStatus,
    pub quantity: u64
}

#[derive(Serialize)]
pub struct OrderPlacedPayload {
    pub order_id: Uuid,
    pub executed_quantity: u64,
    pub order_status: OrderStatus
}

#[derive(Serialize)]
pub struct RejectedPayload {
    pub code: Code,
    pub message: String
}
#[derive(Serialize)]
pub struct GetOpenOrderPayload {
    pub orders: Vec<Order>
}

#[derive(Serialize)]
pub struct GetBalance {
    pub available: String,
    pub locked: String
}

#[derive(Serialize)]
pub struct GetBalancePayload {
    pub asset: Asset,
    pub balance: GetBalance
}

#[derive(Serialize)]
pub enum MessageToApi {
    OrderPlaced(OrderPlacedPayload),
    OrderRejected(RejectedPayload),
    OrderCancelled(OrderCancelledPayload),
    CancelRejected(RejectedPayload),
    GetDepth(DepthResponse),
    GetOpenOrders(GetOpenOrderPayload),
    GetBalance(GetBalancePayload)
}


#[derive(serde::Serialize)]
#[serde(rename = "UNDER_SCORE")]
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
