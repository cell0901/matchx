use crate::trade::Origin;


pub trait ResultPublisher: Send {
    fn complete(&mut self, client_id:String, payload: String, origin: &Origin); // complete means publish + xack
    fn publish(&mut self, client_id:String, payload: String);
}

pub struct RedisPublisher {
    conn: redis::Connection
}

impl ResultPublisher for RedisPublisher {
    fn complete(&mut self, client_id:String, payload: String, origin: &Origin) {
        let mut pipe = redis::pipe();
        pipe.publish(client_id, payload).ignore(); // .ignore to ignore the return value from this

        if let Origin::Stream { entry_id } = origin {
            pipe.xack("order:stream", "engine_group", &[entry_id]).ignore();
        };

        if let Err(e) = pipe.query::<()>(&mut self.conn){
            eprintln!("publish /ack failed {}", e);
        }
    }

    fn publish(&mut self, client_id:String, payload: String) {
        let mut pipe = redis::pipe();
        pipe.publish(client_id, payload).ignore(); // .ignore to ignore the return value from this
    }
}

impl RedisPublisher {
    pub fn new (client: redis::Client) -> Self{
        let conn = client.get_connection().expect("failed to get redis connection");
        RedisPublisher{ conn }
    }
}


pub struct NullPublisher; // for bench mark engine


impl ResultPublisher for NullPublisher {
    fn complete(&mut self, _client_id:String, _payload: String, _origin: &Origin) {
        // no op, user for benchmarking 
    }
    fn publish(&mut self, _client_id:String, _payload: String) {
        // no op
    }
}
