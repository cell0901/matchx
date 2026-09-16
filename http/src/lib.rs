pub const SCALE_FACTOR: u64= 100_000_000;

pub mod entities;
pub mod redis;
pub mod routes;
pub mod types;
pub mod error;

pub use types::message_from_engine::MessageFromEngine;

