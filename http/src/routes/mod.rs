pub mod auth;
pub mod balance;
pub mod order;
pub mod depth;

use std::str::FromStr;

pub use balance::*; // instead of importing every functoin by line in main.rs . we do this
pub use auth::*;
pub use order::*;
pub use depth::*;
use serde::{Deserialize, Serialize};
use strum::EnumString;

#[derive(Serialize, Deserialize, EnumString)]
pub enum Asset{
    SOL,
    BTC,
    ETH,
    HYPE,
    USDC
}

#[derive(Serialize, Deserialize)]
pub struct Market {
    pub base: Asset,
    pub quote: Asset
}

impl FromStr for Market {
   type Err = ();  // associated error type which can be returned

   fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
          "SOL_USDC"=> Ok(Market {base: Asset::SOL, quote: Asset::USDC}),
          "BTC_USDC"=> Ok(Market {base: Asset::BTC, quote: Asset::USDC}),
          "ETH_USDC"=> Ok(Market {base: Asset::ETH, quote: Asset::USDC}),
          "HYPE_USDC"=> Ok(Market {base: Asset::HYPE, quote: Asset::USDC}),
            _ => Err(())
        }
   } 
}


