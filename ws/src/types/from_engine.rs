use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct Market {
    pub base: Asset,
    pub quote: Asset
}

#[derive(Deserialize, Serialize)]
pub enum Asset{
    SOL,
    BTC,
    ETH,
    HYPE,
    USDC
}

#[derive(Deserialize, Serialize)]
pub struct TradePublishData {
    symbol: Market,
    price: u64,
    quantity: u64,
    order_side: OrderSide,
    other_user_id: Uuid,
    trade_id: u64
}

#[derive(Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell
}

#[derive(Deserialize, Serialize)]
pub struct DepthLevel {
    price: u64,// what acutally the engine sends. then we convert this to acutaly json with string
    // numbers to client
    quantity:u64  
}

#[derive(Deserialize, Serialize)]
pub struct DepthUpdateData{
    symbol: Market,
    bids: Vec<DepthLevel>
}

#[derive(Deserialize, Serialize)]
pub enum FromEngine { // rename the enum type while sending to client to data
    TRADEPUBLISH(TradePublishData),
    DEPTHUPDATE (DepthUpdateData)
}
