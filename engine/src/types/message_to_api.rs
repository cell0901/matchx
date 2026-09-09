use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct OrderCancelledPayload{
    pub order_id: String,
    pub executed_quantity: u64,
    pub remaining_quantity: u64
}
