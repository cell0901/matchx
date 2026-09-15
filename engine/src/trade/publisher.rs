use redis::Commands;


pub trait ResultPublisher: Send {
    fn publish(&mut self, client_id:String, payload: String);
}

pub struct RedisPublisher {
    conn: redis::Connection
}

impl ResultPublisher for RedisPublisher {
    fn publish(&mut self, client_id: String, payload: String) {
        let _:Result<(), _> = self.conn.publish(client_id, payload);
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
    fn publish(&mut self, _client_id:String, _payload: String) {
        // no op, user for benchmarking 
    }
}
