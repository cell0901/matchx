use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{trade::{DepthDelta, Market}, types::OrderSide};

#[derive(Deserialize, Serialize)]
pub struct TradePublish {
    pub symbol: Market,
    pub price: u64,
    pub quantity: u64,
    pub order_side: OrderSide,
    pub other_user_id: Uuid,
    pub trade_id: u64
}

#[derive(Deserialize, Serialize)]
pub struct TradePublishData { // each trade updates channell message will have array of all fills
    pub trades: Vec<TradePublish>
}

#[derive(Deserialize, Serialize)]
pub struct DepthUpdateMsg {
    pub symbol: Market, 
    pub depth_deltas: Vec<DepthDelta>
}

pub enum WsPublisherActions {
   PubishTrade(TradePublishData),
   DepthUpdate(DepthUpdateMsg)
}
