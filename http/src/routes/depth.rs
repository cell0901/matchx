use actix_web::{HttpResponse, Responder, get, web::{self, Json}};
use serde::Deserialize;

use crate::{redis::redis_manager::RedisManager, routes::Market, types::message_to_engine::MessageToEngine};

#[derive(Deserialize)]
pub struct GetDepthPayload {
    symbol: String
}

#[get("/depth")]
pub async fn get_depth(body: Json<GetDepthPayload>,  data: web::Data<RedisManager>) -> impl Responder{

    let parsed = body.symbol.parse::<Market>().map_err(|_| format!("invalid market"));

   match parsed {
        Ok(market) => {
            let res = data.send_and_await(MessageToEngine::GetDepth(market)).await;
            match res {
                Ok(val) => {
                    // return the direct response from engine to user
                    return HttpResponse::Ok().json(val)
                },
                Err(err) => {
                    eprintln!("error occured /depth {}", err);
                    return  HttpResponse::BadRequest().json("error occured while gettign depth")
                }
            }
        },
        Err(e) => {
            return HttpResponse::BadRequest().json(e);
        }
       
   } 

}
