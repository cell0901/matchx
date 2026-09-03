use serde::{Deserialize, Serialize};

use crate::types::message_to_engine::{OrderSide, OrderType};

#[derive(Serialize, Deserialize)]
pub struct OrderSchema { // from client
    pub symbol: String,
    pub order_type: OrderType,
    pub order_side: OrderSide,
    pub price: String, // it should be an option if the order type is market. but currenly we will
    // default
    pub quantity: String,
    pub order_id: String 
}

#[derive(Deserialize)]
pub struct CancelOrderSchema {
    pub symbol: String,
    pub order_id: String
}

#[derive(Deserialize)]
pub struct OpenOrderSchema{
    pub symbol: String,
}


