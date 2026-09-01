use actix_web::{HttpResponse, Responder, get, post, web::{self, Json}};

use crate::{UserBalances, redis::redis_manager::RedisManager, types::{Balance, DepositSchema, GetBalanceParam, GetBalanceResponse, OnrampSchema}};

#[post("/balance/onramp")]
// Json under the hood converts the incmoing json to rust struct
pub async fn onramp(user_id:web::ReqData<String>, body: Json<OnrampSchema>, data: web::Data<RedisManager>) -> impl Responder{
     
    // get the userId from the middlware and send to redis

    HttpResponse::Ok().body("Onramp successfull")
}

#[post("/balance/deposit")] 
pub async fn deposit(username: web::ReqData<String>, body: Json<DepositSchema>, data: web::Data<UserBalances>) -> impl Responder{
    // impl logic of user sending asset to that address and increase the balance here after
   
    HttpResponse::Ok().body("Deposit successfull")
}

#[get("/balance")]
pub async fn get_balance(username:web::ReqData<String>, param: web::Query<GetBalanceParam>, data: web::Data<UserBalances>) -> impl Responder{
    let res = GetBalanceResponse {
            balance: 0
    };
      HttpResponse::Ok().json(res)
}

