use std::env;

use futures_util::StreamExt;
use redis::{AsyncCommands, Client, aio::{MultiplexedConnection}};
use serde::{Serialize};
use uuid::Uuid;

use crate::{error::EngineError, types::{message_from_engine::MessageFromEngine, message_to_engine::MessageToEngine::{self}}};

#[derive(Serialize)]
pub struct EngineMessage {
    client_id: String,
    data: MessageToEngine
}

#[derive(Clone)]
pub struct RedisManager {
    pub client: Client,
    pub conn: MultiplexedConnection,
}

impl RedisManager {
    pub async fn new() -> Self{
        let redis_url = match env::var("REDIS_URL") {
            Ok(url) => url,
            Err(e) => panic!("redis url not found in environment: {}", e),
        };

        let client = Client::open(redis_url).unwrap();
        let con = client.get_multiplexed_async_connection().await.expect("some error occured while getting connection"); 
        RedisManager {
            client,
            conn: con
        }
    }

    pub async fn send_and_await(&self, message:MessageToEngine) -> Result<MessageFromEngine, EngineError>{
        let mut connection = self.conn.clone();
        let client_id = Uuid::now_v7().to_string(); // generate the random id for this user request
        let mut pubsub = self.client.get_async_pubsub().await?; 
        // ? returns RedisError so it looks for impl From<RedisError> for EngineError.
        // EngineError::from(val)
        // takes acutual error and format! it into string

        pubsub.subscribe(&client_id).await?; // sub to this id
        let mut stream= pubsub.on_message(); // gets the stream of messages from pubsub
        
        let is_query = matches!(
            message,
            MessageToEngine::GetOpenOrders(_) | MessageToEngine::GetDepth(_) | MessageToEngine::GetBalance(_)
        );
        
        let payload = serde_json::to_string(&EngineMessage {
            client_id: client_id.clone(),
            data: message
        })?;

        if is_query { 
            // if the request is getorders or depth then just send to normal queue
            println!("ithis is queue");
            let _: () = connection.lpush("engine:queue", payload).await?;  // returns length of new list
            // lpush returns ()
            // something which type needs to known even if u dont need. so we just coerce it into
            // ()
        } else {
            // send to stream to replay data after engine restart
            let _: () = connection.xadd("order:stream","*", &[("data", payload)]).await?;
        };

        // Wait for pubsub for message from engine
        // duration is min 2 secs from engine response until i return error to user.
        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .map_err(|_| EngineError::Timeout(client_id))?
            .ok_or(EngineError::StreamClosed)?; // stream close
             

        let payload: String = msg.get_payload()?;
        // recived from the pubsub
        let from_engine: MessageFromEngine = serde_json::from_str(&payload)?;
        Ok(from_engine)
    }

}
