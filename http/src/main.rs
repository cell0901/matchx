pub mod types;
pub mod routes;
pub mod redis;
pub mod db;
pub mod error;
pub mod entities;

use std::env;

use sea_orm::{ Database };
use actix_web::middleware::{ from_fn};
use actix_web::{App,  HttpServer,  web::{ self}};

use crate::redis::redis_manager::RedisManager;
use crate::routes::{auth_middleware, deposit, get_balance, onramp};
use crate::routes::auth::{signin, signup};

// #[derive(Debug)]
// struct UserBalances{
//     // (username, Asset), balances
//     user_balances: Mutex<HashMap<(String,String), Balance>>// we should use u32,u32 for user name and
//     // asset since hashing them is fast and reduces heap allocation unlike inthis case
//     // for faster and reduce latency we can use Dashmap(it doesnt locks the whole haspmap)
// }


pub const SCALE_FACTOR: u64= 100_000_000;

#[actix_web::main] // this executre the main function in actix web runtime or tokio runtime
async fn main() -> std::io::Result<()> {
    // Load the .env file ONCE for the entire application
    dotenvy::dotenv().expect("no .env exist");

    let database_url = match env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(e) => panic!("DATABASE_URL not found in environment: {}", e),
    };

   let db = Database::connect(database_url).await.expect("Database connection failed");
    
    let db_conn = web::Data::new(db);
    

    // init redis connection before starting server
    let r = RedisManager::new().await; 
    let redis_state = web::Data::new(r);// pass this state clone to every route

    let server = HttpServer::new(move|| {
    App::new()
        .app_data(redis_state.clone())
        .app_data(db_conn.clone())
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
