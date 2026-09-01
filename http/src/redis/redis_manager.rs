use futures_util::StreamExt;
use redis::{AsyncCommands, Client, RedisResult, aio::{MultiplexedConnection}};
use serde::{Serialize};
use uuid::Uuid;

use crate::types::{message_from_engine::MessageFromEngine, message_to_engine::MessageToEngine::{self}};

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
        let client = Client::open("redis://localhost:6379").unwrap();
        let con = client.get_multiplexed_async_connection().await.expect("some error occured while getting connection"); 
        RedisManager {
            client,
            conn: con
        }
    }

    pub async fn send_and_await(&self, message:MessageToEngine) -> RedisResult<MessageFromEngine>{
        let mut connection = self.conn.clone();
        let client_id = self.generate_client_id(); // generate the random id for this user request
        let mut pubsub = self.client.get_async_pubsub().await.expect("somme error while getting pubsub");

        pubsub.subscribe(&client_id).await?; // sub to this id
        let mut stream= pubsub.on_message(); // gets the stream of messages from pubsub
        
        let is_query = matches!(
            message,
            MessageToEngine::GetOpenOrders(_) | MessageToEngine::GetDepth(_)
        );
        
        let payload = serde_json::to_string(&EngineMessage {
            client_id: client_id.clone(),
            data: message
        }).expect("error while Serialize");

        if is_query {
            // send the message to stream
            let _: () = connection.lpush("engine", payload).await?;  // returns length of new list
            // () since lpush returns
            // something which type needs to known even if u dont need. so we just coerce it into
            // ()
        } else {
            let _: () = connection.xadd("order:stream","*", &[("data", payload)]).await?;
        };

        // duration is min 2 secs from engine response until i return error to user.
        let msg = stream.next().await.unwrap();

        let payload: String = msg.get_payload()?;
        // recived from the pubsub
        let from_engine: MessageFromEngine = serde_json::from_str(&payload).expect("error while parsing from engine");
        Ok(from_engine)
    }

    fn generate_client_id (&self) -> String{
        let a = Uuid::new_v4();            
        a.to_string()
    }
}
