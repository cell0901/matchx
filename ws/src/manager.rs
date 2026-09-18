use std::{collections::{HashMap, HashSet}, env, sync::Arc };

use actix_web::web::Data;
use actix_ws::{AggregatedMessage, AggregatedMessageStream, Session};
use futures_util::StreamExt;
use redis::{Client};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::types::{FromClient, FromEngine, Method};

pub struct UserManager{
    pub users : Arc<RwLock<HashMap<String, UserInfo>>>, // random id (or the client ws session address)
    pub active_channels: Arc<RwLock<HashSet<String>>>, // dedup. which channels already have
    // listener. for eg for a chennel if new user comes and subscribes. and second user comes
    // instead of spawning two redis pubsub we rely on one
    pub client: Client
}

pub struct UserInfo { // subscriptions. vector for each trade, depth, trades
    pub user_id: Option<Uuid>, // optional if the stream is private then we verify the jwt, store
    // the id and subscribe for order fills
    pub tx: Session,
    pub subscriptions: Vec<String>,
}

impl UserManager {
    pub fn new() -> Self{
        let redis_url = match env::var("REDIS_URL") {
            Ok(url) => url,
            Err(e) => panic!("redis url not found in environment: {}", e),
        };
        let client  = Client::open(redis_url).expect("error while creating client");
        UserManager { 
            users: Arc::new(RwLock::new(HashMap::new())), 
            active_channels: Arc::new(RwLock::new(HashSet::new())),
            client: client}
    }

    pub async fn add_user(&self, session: Session) -> String{
        let connection_id = Uuid::now_v7().to_string();

        let info = UserInfo {
            user_id: None,
            tx: session,
            subscriptions: Vec::new(),
        };

        self.users.write().await.insert(connection_id.clone(), info);
        connection_id
    }

    pub async fn remove_user(&self, user_addr: &str) {
        self.users.write().await.remove(user_addr);
    }

    pub async fn subscribe(self: &Arc<UserManager>, connection_id: &str, param: String) {

        let is_new_subscription = {
            if let Some(user) = self.users.write().await.get_mut(connection_id) {
                //          stream , users connection_ids
                if !user.subscriptions.contains(&param) { // only push if no string of param in array
                    user.subscriptions.push(param.clone());
                    true // return true first time user subscribing
                } else {
                    false // user already constns
                }
            } else { // no user found
                false
            }
        };

        if is_new_subscription {
             let mut channels = self.active_channels.write().await;
            if channels.insert(param.clone()) { // while inserting an hashtset value for first time. it
                // returns true. and false if already present and not updates anything
                let manager = self.clone();
                actix_web::rt::spawn(async move {
                    manager.run_channel_listener(param).await;
                });
            }
            // else do nothing there is alreayd a pubusb listener running 
        };
    }

    pub async fn unsubscribe(&self, connection_id: &str, param: &str) {
        if let Some(user) = self.users.write().await.get_mut(connection_id) {
            user.subscriptions.retain(|s| s != param); // removes all elements for this condition
            // returns false
        }
    }

    pub async fn run_channel_listener(&self, channel: String) { // connect to pub sub of each stream and 
        let mut pubsub = self.client.get_async_pubsub().await.expect("getting pubsub error");
        pubsub.subscribe(&channel).await.expect("error while subscribing");
        let mut stream = pubsub.on_message();

        while let Some(msg) = stream.next().await {
            match msg.get_payload::<String>() {
                Ok(val) => {
                    match serde_json::from_str::<FromEngine>(&val) {
                        Ok(payload) => {
                            let out = serde_json::to_string(&payload).unwrap_or_default();
                            self.broadcast(&channel, out).await; // fan-out lives here, once
                        }
                        Err(e) => eprintln!("deserialize error on {channel}: {e}"),
                    }            
                },
                Err(_) => eprintln!("error while getting msg payload")
            }
        }
    }


    pub async fn broadcast(&self, channel: &str, payload: String) {
        let users = self.users.read().await;
        for user in users.values(){
            if user.subscriptions.iter().any(|s| s == channel) { // any returns true or false based
                let _ = user.tx.clone().text(payload.clone()).await;
            }
        }
    }

}

pub async fn handle_connection(users: Data<UserManager>, mut stream: AggregatedMessageStream, connection_id: String) {
    while let Some(val) = stream.recv().await {
        match val{ 
            Ok(AggregatedMessage::Text(msg)) => {
                let message: FromClient = serde_json::from_str(&msg).expect("error while Deserialize");
                match message.method {
                    Method::SUBSCRIBE => {
                        // Todo- add check if its private stream. like starts with
                        // "account.orderUpdate."
                        for i in message.params.iter() {
                            users.subscribe(&connection_id, i.to_string()).await;
                        }
                    },
                    Method::UNSUBSCRIBE => {
                        // Todo- add check if its private stream.
                        for i in message.params.iter() {
                            users.unsubscribe(&connection_id, i).await;
                        }
                    }
                };
                println!("msg {:?}", message);
            },
            Ok(AggregatedMessage::Close(reason)) => {// on connection close
                // remove the entry 
                users.remove_user(&connection_id).await;
                println!("reason for close {:?}", reason);
                // to acutaly disconnect drop the task
                break;
            },
            _ => { // handling error case 
                users.remove_user(&connection_id).await;
                break;
            }
        };

    };
}
