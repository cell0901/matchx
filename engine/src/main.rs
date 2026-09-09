use redis::RedisResult;

use crate::{handle_incoming::{handle_queue, handle_stream}, trade::Engine};

pub mod handle_incoming;
pub mod trade;
pub mod error;
pub mod types;

#[tokio::main]
async fn main() -> RedisResult<()>{
    let engine = Engine::new();

    let client = redis::Client::open("redis://localhost:6379")?;
    let queue_task = tokio::spawn(handle_queue(client.clone(), engine.clone())); // cloning engine
    // is fine , we only storing tx for balance and markets. which is fine and are safe to clone
    let stream_task = tokio::spawn(handle_stream(client.clone(), engine.clone()));

    let _ =tokio::join!(queue_task, stream_task); // waits for both the spawned tasks to be completed
    Ok(())
}
