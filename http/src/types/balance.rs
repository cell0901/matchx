use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Balance {
    pub available: u64,
    pub _locked: u64
}

#[derive(Serialize,Deserialize)]
pub struct OnrampSchema {
    pub amount: u64
}

#[derive(Serialize,Deserialize)]
pub struct DepositSchema {
    pub asset: String,
    pub quantity: u64,
}

#[derive(Serialize,Deserialize)]
pub struct GetBalanceParam {
    pub asset: String
}

#[derive(Serialize,Deserialize)]
pub struct GetBalanceResponse {
   pub balance: u64 
}
