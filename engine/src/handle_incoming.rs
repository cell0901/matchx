use redis::{AsyncCommands, AsyncConnectionConfig, Client, Value, streams::{StreamReadOptions, StreamReadReply}};

use crate::{error::EngineError, trade::{Engine}, types::EngineMessage};

pub async fn handle_stream(client: Client, engine: Engine) -> Result<(), EngineError>{
    let mut con = client.get_multiplexed_async_connection().await?; // using async connection to
    // avoid blocking io

    // diff between default xgropu create is that this one creates the stream if it doesnt
    // exists instead of panic
    let _: Result<(), _> = con.xgroup_create_mkstream("order:stream", "engine_group", "$").await; // $
    // means start the group for only 
    
    loop {
        
    // running one consumer 
    let ops = StreamReadOptions::default().group("engine_group", "consumer_1").count(100).block(200);// block upto 200 millcs if none are present. 
        // then loop again. process 100 batch at once even if there are 500 messages queued up
    let res: StreamReadReply =  con.xread_options(&["order:stream"], &[">"], &ops).await?; // >
    // only new, undelivered messages
        
        for stream_key in res.keys { // currently only one stream key
           for entry in stream_key.ids {
                let data = entry.map.get("data").unwrap();

                match data {
                    Value::BulkString(val) => {
                        let message: EngineMessage= serde_json::from_slice(&val).unwrap();
                        println!("message came inside handle_stream");
                        engine.process(message.data, message.client_id, Some(entry.id)).await;
                        // publish to pubsub the repsonse
                    },
                    _ => {
                        // unexpected value
                        // return order cancelled
                    }
                };
                // only after the matching and every memory chanages are done. ack the message that
                // it has been processed and not in Pending state
                // let _: i64 = con.xack("order:stream", "engine_group", &[entry.id]).await?;
            } 
        }
    }
}

pub async fn handle_queue(client: Client, engine:Engine)-> Result<(), EngineError> {
    // BRPOP with a timeout of 0 blocks indefinitely. redis-rs 1.x gives async
    // connections a 500 ms response timeout by default, so disable that timeout
    // only for this dedicated blocking-consumer connection.
    let config = AsyncConnectionConfig::new().set_response_timeout(None);
    let mut con = client
        .get_multiplexed_async_connection_with_config(&config)
        .await?;
    
    println!("handle_queue spawned");

    loop {
        let res: Option<(String, String)> =
            con.brpop("engine:queue", 0.0).await?;

        if let Some((_, message)) = res {
            let message: EngineMessage = serde_json::from_str(&message)?;

            println!("message came inside queue");

            engine
                .process(message.data, message.client_id, None)
                .await;
        } else {
            println!("error while getting queue item");
        }
    }
}
