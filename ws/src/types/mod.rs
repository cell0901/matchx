pub mod from_client;
pub mod from_engine;

pub use from_client::*;
pub use from_engine::*;

pub const SCALE_FACTOR: u64= 100_000_000;

pub const VALID_PREFIX: [&str; 2]= ["depth.", "trade."];
