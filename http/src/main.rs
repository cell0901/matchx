pub mod types;
pub mod routes;
pub mod redis;
pub mod db;

use std::{collections::HashMap, sync::Mutex};

use actix_web::middleware::{ from_fn};
use actix_web::{App,  HttpServer,  web::{ self}};

use crate::redis::redis_manager::RedisManager;
use crate::routes::{auth_middleware, deposit, get_balance, onramp};
use crate::routes::user::{signin, signup};
use crate::types::{Balance,    User };

#[derive(Debug)]
struct Users {
    users: Mutex<HashMap<String, User>>  // Key is username
}

#[derive(Debug)]
struct UserBalances{
    // (username, Asset), balances
    user_balances: Mutex<HashMap<(String,String), Balance>>// we should use u32,u32 for user name and
    // asset since hashing them is fast and reduces heap allocation unlike inthis case
    // for faster and reduce latency we can use Dashmap(it doesnt locks the whole haspmap)
}


#[actix_web::main] // this executre the main function in actix web runtime or tokio runtime
async fn main() -> std::io::Result<()> {
    // Load the .env file ONCE for the entire application
    dotenvy::dotenv().expect("no .env exist");
    
    // init redis connection before starting server
    let r = RedisManager::new().await; 
    let redis_state = web::Data::new(r);// pass this state clone to every route

    let temp  = web::Data::new(Users {
        users: Mutex::new(HashMap::new()), // we dont need to do Arc since web::Data already does
        // internally
    });

    let balances= web::Data::new(UserBalances {
        user_balances: Mutex::new(HashMap::new()), // we dont need to do Arc since web::Data already does
        // internally
    });

    let server = HttpServer::new(move|| {
    App::new()
        .app_data(redis_state.clone())
        // 1. Register public routes FIRST
        .service(signin)
        .service(signup) 
        // 2. Register the protected scope LAST
        .service(
            web::scope("")
            .wrap(from_fn(auth_middleware))
            .service(onramp)
            .service(deposit)
            .service(get_balance)
        )
})
    .bind(("127.0.0.1", 8080))?
    .run();
    println!("listening on localhost:8080");
    server.await //spawns worker thread same as no. of cores in ur machine
    
}
