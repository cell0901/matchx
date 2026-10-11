use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    trade::{Asset, DepthResponse, OrderStatus},
    types::{OrderSide, OrderType},
};

#[derive(Serialize, Deserialize)]
pub struct OrderCancelledPayload{
    pub order_id: Uuid,
    pub executed_quantity: String,
    pub order_status: OrderStatus,
    pub quantity: String
}

#[derive(Serialize)]
pub struct OrderPlacedPayload {
    pub order_id: Uuid,
    pub executed_quantity: String,
    pub order_status: OrderStatus
}

#[derive(Serialize)]
pub struct ResponsePayload {
    pub code: Code,
    pub message: String
}
#[derive(Serialize)]
pub struct OpenOrder {
    pub order_id: Uuid,
    pub price: String,
    pub user_id: Uuid,
    pub quantity: String,
    pub order_side: OrderSide,
    pub order_type: OrderType,
    pub filled: String,
}

#[derive(Serialize)]
pub struct GetOpenOrderPayload {
    pub orders: Vec<OpenOrder>
}

#[derive(Serialize)]
pub struct GetBalance {
    pub available: String,
    pub locked: String
}

#[derive(Serialize)]
pub struct GetBalanceResponse {
    pub asset: Asset,
    pub balance: GetBalance
}


#[derive(Serialize)]
pub enum MessageToApi {
    OrderPlaced(OrderPlacedPayload),
    OrderRejected(ResponsePayload),
    OrderCancelled(OrderCancelledPayload),
    CancelRejected(ResponsePayload),
    GetDepth(DepthResponse),
    GetOpenOrders(GetOpenOrderPayload),
    GetBalance(GetBalanceResponse),
    OnrampResponse(ResponsePayload),
    DepositResponse(ResponsePayload)
}


#[derive(serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // rename all to rename enum's type 
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
