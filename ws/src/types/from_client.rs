use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct FromClient {
    pub method: Method,
    pub params: Vec<String>
}

#[derive(Deserialize, Debug)]
pub enum Method {
    SUBSCRIBE,
    UNSUBSCRIBE
}


