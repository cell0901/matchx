pub mod orderbook;
pub mod engine;

use crossbeam_channel::Sender;
pub use orderbook::*;
pub use engine::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{CancelOrderPayload, CreateOrderPayload, GetBalancePayload, OpenOrderPayload, OrderSide};

pub const SCALE_FACTOR: u64= 100_000_000;

pub const MARKETS: [Market; 4] = [
    Market {
        base: Asset::SOL,
        quote: Asset::USDC
    },
    Market {
        base: Asset::BTC,
        quote: Asset::USDC
    },
    Market {
        base: Asset::ETH,
        quote: Asset::USDC
    },
    Market {
        base: Asset::HYPE,
        quote: Asset::USDC
    },
];

#[derive(Clone,Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Asset{
    SOL,
    BTC,
    ETH,
    HYPE,
    USDC
}

pub enum BalanceActions {
    ValidateAndLockFunds(ValidateAndLockData),
    SettleFills(SettleFillsData),
    CancelAndUpdateBalance(Order, Market, Sender<SettleResult>), // using same enum for now
    Onramp(u64, Uuid), // amount, user_id
    Deposit(Asset, u64, Uuid),// asset, qty, user_id
    GetBalance(Asset, Uuid)
}

pub enum ValidateAndLockResponse{
    Success,
    InsufficientFunds,
    Overflow // u64 overflow
}

#[derive(PartialEq, Eq)]
pub enum SettleResult {// this will alawys be success since prevalidate before order. unless there
    // is some overflow u64 error
    Success,
    Overflow
}

pub struct SettleFillsData {
    fills: Vec<Fill>,
    side: OrderSide,
    base_asset: Asset,
    quote_asset: Asset,
    resp: Sender<SettleResult>
}


pub struct ValidateAndLockData{
    user_id: Uuid,
    price: u64,
    quantity: u64,
    side: OrderSide,
    asset: Asset, // or send Market, but we know all quote asset are USDC for now
    resp: Sender<ValidateAndLockResponse>

}

pub enum OrderbookActions{
    CreateOrder(CreateOrderPayload),
    CancelOrder(CancelOrderPayload),
    GetOpenOrders(OpenOrderPayload), // ideally we should get open orders for users from in other in
    // memory logs or database
    GetDepth
}
