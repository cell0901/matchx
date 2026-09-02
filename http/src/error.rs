use std::fmt::{Display};
use std::error::Error;
// We could have also used iserror crate to create custom error easily

#[derive(Debug)]
pub enum EngineError {
    Redis(String), // for any RedisError type
    Timeout(String), // for tokio timeout if engine failed to return anything in the duration
    Serde(String),
    StreamClosed
}

// using From trait for value to value type conversion. here serde error to string
impl From<serde_json::Error> for EngineError {
    fn from(value: serde_json::Error) -> Self {
        EngineError::Serde(format!("Serde Error, {}", value))
    }
}

impl From<redis::RedisError> for EngineError {
    fn from(value: redis::RedisError) -> Self {
        // eg. Redis Error Connection Refused.
        EngineError::Redis(format!("Redis Error {}", value)) // this is stored in
        // EngineError::Redis(String) string value
    }
}

impl Display for EngineError { // this is actualy get called for logging in the route handler to check what
    // error came
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
        EngineError::Redis(val)  => write!(f, "Redis Error occurred {}",val ),
        EngineError::Timeout(val) => write!(f, "Engine did not responsd in time {}", val),
        EngineError::Serde(val)=> write!(f, "serialization error {}", val),
        EngineError::StreamClosed => write!(f, "pubsub stream closed before receving a reply")
      }  
    }
}

impl Error for EngineError {} 
