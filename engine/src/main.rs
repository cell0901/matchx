use std::env;

use redis::RedisResult;

use crate::{handle_incoming::{handle_queue, handle_stream}, trade::Engine};

pub mod handle_incoming;
pub mod trade;
pub mod error;
pub mod types;

#[tokio::main]
async fn main() -> RedisResult<()>{
    dotenvy::dotenv().expect("no .env exist");
    let redis_url = match env::var("REDIS_URL") {
        Ok(url) => url,
        Err(e) => panic!("redis url not found in environment: {}", e),
    };

    let engine = Engine::new();

    let client = redis::Client::open(redis_url)?;
    let queue_task = tokio::spawn(handle_queue(client.clone(), engine.clone())); // cloning engine
    // is fine , we only storing tx for balance and markets. which is fine and are safe to clone
    let stream_task = tokio::spawn(handle_stream(client.clone(), engine.clone()));

    let _ =tokio::join!(queue_task, stream_task); // waits for both the spawned tasks to be completed
    Ok(())
}
