use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{trade::Market, types::OrderSide};

#[derive(Deserialize, Serialize)]
pub struct TradePublishData {
    pub symbol: Market,
    pub price: u64,
    pub quantity: u64,
    pub order_side: OrderSide,
    pub other_user_id: Uuid,
    pub trade_id: u64
}

#[derive(Deserialize, Serialize)]
pub struct DepthUpdateData {
    pub symbol: Market, 
    // TODO bids and asks remaining
}

pub enum WsPublisherActions {
   PubishTrade(TradePublishData),
   DepthUpdate(DepthUpdateData)
}
