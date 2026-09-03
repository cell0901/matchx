use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct CreateOrderPayload{
    pub symbol: String, // SOL_USD, ETH_USD, BTC_USD
    pub order_type: OrderType, // market/ limit
    pub order_side: OrderSide, // buy or sell
    pub price: u64,
    pub quantity: u64,
    pub user_id: String, // authorized userId
    pub order_id: String // random generated order id in the route
}

#[derive(Serialize, Deserialize)]
pub struct CancelOrderPayload{
    pub symbol: String, 
    pub order_id: String,
    pub user_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct OnrampPayload { // the usd balance
    pub amount: u64,
    pub user_id: String
}

#[derive(Serialize, Deserialize)]
pub struct DepositPayload{
    pub asset: String,
    pub quantity: u64,
    pub user_id: String
}

#[derive(Serialize, Deserialize)]
pub struct OpenOrderPayload{
    pub symbol: String,
    pub user_id: String
}

#[derive(Serialize, Deserialize)]
pub struct GetBalancePayload{
    pub asset: String,
    pub user_id: String
}

#[derive(Serialize, Deserialize, Clone)]
pub enum OrderSide {
    Buy,
    Sell
}

#[derive(Serialize, Deserialize, Clone)]
pub enum OrderType {
    Limit,
    Market
}

#[derive(Serialize, Deserialize)]
pub enum MessageToEngine{
    CreateOrder(CreateOrderPayload),
    CancelOrder(CancelOrderPayload),
    Onramp(OnrampPayload),
    Deposit(DepositPayload),
    GetOpenOrders(OpenOrderPayload),
    GetDepth(String), // symbol
    GetBalance(GetBalancePayload)
}

