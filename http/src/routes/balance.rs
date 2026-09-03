use actix_web::{HttpResponse, Responder, get, post, web::{self, Json}};

use crate::{ redis::redis_manager::RedisManager, types::{ DepositSchema, GetBalanceParam,  OnrampSchema, 
    message_to_engine::{DepositPayload, GetBalancePayload, MessageToEngine, OnrampPayload}}};

#[post("/balance/onramp")]
// Json under the hood converts the incmoing json to rust struct
pub async fn onramp(user_id:web::ReqData<String>, body: Json<OnrampSchema>, data: web::Data<RedisManager>) -> impl Responder{
    // get the userId from the middlware and send to redis

    let res =  data.send_and_await(MessageToEngine::Onramp(OnrampPayload { // no need to deref *
        // data since it does it automatically
        amount: body.amount,
        user_id: user_id.to_string()
    })).await;

    match res {
        Ok(val) => HttpResponse::Ok().json(val),
        Err(err) => {
            println!("error occurrred {}", err);
            HttpResponse::BadRequest().body("error occurrred while onramp")
        }
    }
}

#[post("/balance/deposit")] 
pub async fn deposit(user_id: web::ReqData<String>, body: Json<DepositSchema>, data: web::Data<RedisManager>) -> impl Responder{
    // impl logic of user sending asset to that address and increase the balance here after
    
    let res = data.send_and_await(MessageToEngine::Deposit(DepositPayload {
        asset: body.asset.clone(), // clone since body doesnt own the schema its Json which doesnt
        // have Deref
        quantity: body.quantity,
        user_id: user_id.to_string()
    })).await;

    match res {
        Ok(val) => HttpResponse::Ok().json(val),
        Err(err) => {
            println!("error occurrred deposit{}", err);
            HttpResponse::BadRequest().body("error occurrred while deposit")
        }
    }
}

#[get("/balance")]
pub async fn get_balance(user_id:web::ReqData<String>, param: web::Query<GetBalanceParam>, data: web::Data<RedisManager>) -> impl Responder{

     let res = data.send_and_await(MessageToEngine::GetBalance(GetBalancePayload {
        asset: param.asset.clone(), // query param
        user_id: user_id.to_string()
    })).await;

    match res {
        Ok(val) => HttpResponse::Ok().json(val),
        Err(err) => {
            println!("error occurrred get balance{}", err);
            HttpResponse::BadRequest().body("unable to get balance")
        }
    }

}

