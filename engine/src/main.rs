use std::env;

pub mod error;
pub mod handle_incoming;
pub mod types;
pub mod trade;

use redis::RedisResult;

use crate::{handle_incoming::{handle_queue, handle_stream}, trade::Engine};



#[tokio::main]
async fn main() -> RedisResult<()>{
    dotenvy::dotenv().expect("no .env exist");
    let redis_url = match env::var("REDIS_URL") {
        Ok(url) => url,
        Err(e) => panic!("redis url not found in environment: {}", e),
    };

    let client = redis::Client::open(redis_url)?;

    let con = client.get_multiplexed_async_connection().await.expect("some error occured while getting connection"); 
    let engine = Engine::new(con, client.clone(), false);

    let stream_client = client.clone();
    let stream_engine = engine.clone();

    let stream_task = tokio::spawn(async move {
        if let Err(err) = handle_stream(stream_client, stream_engine).await {
            eprintln!("stream worker stopped: {err}");
        }
    });
    let queue_task = tokio::spawn(async move {
        if let Err(err) = handle_queue(client, engine).await {
            eprintln!("queue worker stopped: {err}");
        }
    });
    // is fine , we only storing tx for balance and markets. which is fine and are safe to clone

    let _ = tokio::join!(queue_task, stream_task); // waits for both workers to be completed
    Ok(())
}
