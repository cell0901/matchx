use std::fmt::{Display};
use std::error::Error;

use crossbeam_channel::RecvError;



#[derive(Debug)]
pub enum EngineError {
    Redis(String), // for any RedisError type
    Serde(String),
    StreamClosed
}

#[derive(Debug)]
pub enum ValidateError {
    InsufficientFunds,
    Overflow,
    Reciever(RecvError)
}

// using From trait for value to value type conversion. here serde error to string
impl From<serde_json::Error> for EngineError {
    fn from(value: serde_json::Error) -> Self {
        EngineError::Serde(format!("Serde Error, {}", value))
    }
}

impl From<redis::RedisError> for EngineError {
    fn from(value: redis::RedisError) -> Self {
        EngineError::Redis(format!("Redis Error {}", value)) // this is stored in
        // EngineError::Redis(String) string value
    }
}

impl Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
        EngineError::Redis(val)  => write!(f, "Redis Error occurred {}",val ),
        EngineError::Serde(val)=> write!(f, "serialization error {}", val),
        EngineError::StreamClosed => write!(f, "pubsub stream closed before receving a reply")
      }  
    }
}

impl Display for ValidateError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
            ValidateError::InsufficientFunds => write!(f, "insufficient funds"),
            ValidateError::Overflow=> write!(f, "overflow error"),
            ValidateError::Reciever(err)=> write!(f, "overflow error {}", err),
        }    
    } 
}

impl Error for EngineError {} 
impl Error for ValidateError {} 
