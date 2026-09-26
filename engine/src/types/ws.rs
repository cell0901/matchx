use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{trade::{DepthDelta, Market}, types::OrderSide};

#[derive(Deserialize, Serialize, Debug)]
pub struct TradePublishMsg{
    pub symbol: Market,
    pub price: u64,
    pub quantity: u64,
    pub order_side: OrderSide,
    pub other_user_id: Uuid,
    pub trade_id: u64
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TradePublishData { // each trade updates channell message will have array of all fills
    pub trades: Vec<TradePublishMsg>
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DepthUpdateMsg {
    pub symbol: Market, 
    pub depth_deltas: Vec<DepthDelta>
}

#[derive(Deserialize, Serialize )]
pub enum WsPublisherActions {
   PublishTrade(TradePublishData),
   DepthUpdate(DepthUpdateMsg)
}

#[derive(Deserialize, Serialize )]
pub enum ToWs {
   PublishTrade(TradePublishMsg),
   DepthUpdate(DepthUpdateMsg)
}
