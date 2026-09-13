use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::trade::{Asset, Market};

#[derive(Serialize, Deserialize)]
pub struct EngineMessage {
    pub client_id: String,
    pub data: MessageFromApi 
}

#[derive(Serialize, Deserialize)]
pub enum MessageFromApi{
    CreateOrder(CreateOrderPayload),
    CancelOrder(CancelOrderPayload),
    Onramp(OnrampPayload),
    Deposit(DepositPayload),
    GetOpenOrders(OpenOrderPayload),
    GetDepth(Market), // symbol
    GetBalance(GetBalancePayload)
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum OrderSide {
    Buy,
    Sell
}

#[derive(Serialize, Deserialize , Clone, Debug)]
pub enum OrderType {
    Limit,
    Market
}

#[derive(Serialize, Deserialize)]
pub struct CreateOrderPayload{
    pub symbol: Market, // SOL_USD, ETH_USD, BTC_USD
    pub order_type: OrderType, // market/ limit
    pub order_side: OrderSide, // buy or sell
    pub price: u64,
    pub quantity: u64,
    pub user_id: Uuid, // authorized userId
    pub order_id: Uuid // random generated order id in the route
}


#[derive(Serialize, Deserialize)]
pub struct CancelOrderPayload{
    pub symbol: Market, 
    pub order_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub struct OnrampPayload { // the usd balance
    pub amount: u64,
    pub user_id: Uuid 
}

#[derive(Serialize, Deserialize)]
pub struct DepositPayload{
    pub asset: Asset, // should be Asset type
    pub quantity: u64,
    pub user_id: Uuid 
}

#[derive(Serialize, Deserialize)]
pub struct OpenOrderPayload{
    pub symbol: Market,
    pub user_id: Uuid 
}

#[derive(Serialize, Deserialize)]
pub struct GetBalancePayload{
    pub asset: Asset,
    pub user_id: Uuid 
}

