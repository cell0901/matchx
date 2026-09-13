use actix_web::{HttpResponse, Responder, get, post, web::{self, Json}};
use serde_json::json;
use uuid::Uuid;

use crate::{ redis::redis_manager::RedisManager, routes::{Asset, string_to_64}, types::{ DepositSchema, GetBalanceParam,  OnrampSchema, 
    message_to_engine::{DepositPayload, GetBalancePayload, MessageToEngine, OnrampPayload}}};

#[post("/balance/onramp")]
// Json under the hood converts the incmoing json to rust struct
pub async fn onramp(user_id:web::ReqData<Uuid>, body: Json<OnrampSchema>, data: web::Data<RedisManager>) -> impl Responder{
    // get the userId from the middlware and send to redis
    
    let parsed = match string_to_64(&body.amount) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("error onramp {}", e);
            return HttpResponse::BadRequest().json(json!({"code": "INVALID_RANGE", "message": "amount range not supported. please enter valid amout"}));
        }
    }; 

    if parsed == 0 {
        return HttpResponse::BadRequest().json(json!({"code": "INVALID_QUANTITY", "message": "please enter amount more than 0"}));
    };

    
    let res =  data.send_and_await(MessageToEngine::Onramp(OnrampPayload { // no need to deref *
        // data since it does it automatically
        amount: parsed,
        user_id: *user_id
    })).await;

    match res {
        Ok(val) => HttpResponse::Ok().json(val),
        Err(err) => {
            println!("error occurrred {}", err);
            HttpResponse::BadRequest().body("internal server error /onramp route")
        }
    }
}

#[post("/balance/deposit")] 
pub async fn deposit(user_id: web::ReqData<Uuid>, body: Json<DepositSchema>, data: web::Data<RedisManager>) -> impl Responder{
    // impl logic of user sending asset to that address and increase the balance here after
    
    let parsed = match string_to_64(&body.quantity) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("error onramp {}", e);
            return HttpResponse::BadRequest().json(json!({"code": "INVALID_RANGE", "message": "amount range not supported. please enter valid amout"}));
        }
    }; 

    if parsed == 0 {
        return HttpResponse::BadRequest().json(json!({"code": "INVALID_QUANTITY", "message": "please enter amount more than 0"}));
    };

    let asset = body.asset.parse::<Asset>().map_err(|_| format!("Invalid asset"));

    if let Err(err) = asset {
        return HttpResponse::BadRequest().json(err);
    };

    let res = data.send_and_await(MessageToEngine::Deposit(DepositPayload {
        asset: asset.unwrap(), // clone since body doesnt own the schema its Json which doesnt
        // have Deref
        quantity: parsed,
        user_id: *user_id
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
pub async fn get_balance(user_id:web::ReqData<Uuid>, param: web::Query<GetBalanceParam>, data: web::Data<RedisManager>) -> impl Responder{
    let asset = param.asset.parse::<Asset>().map_err(|_| format!("Invalid asset"));

    if let Err(err) = asset {
        return HttpResponse::BadRequest().json(err);
    };

     let res = data.send_and_await(MessageToEngine::GetBalance(GetBalancePayload {
        asset: asset.unwrap(), // query param
        user_id: *user_id
    })).await;

    match res {
        Ok(val) => HttpResponse::Ok().json(val),
        Err(err) => {
            println!("error occurrred /get balance {}", err);
            HttpResponse::BadRequest().body("unable to get balance")
        }
    }

}

