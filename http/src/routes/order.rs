use actix_web::{HttpResponse, Responder, delete, get, post, web::{self, Json}};
use uuid::Uuid;
use rust_decimal::prelude::*;

use crate::{SCALE_FACTOR, redis::redis_manager::RedisManager, types::{CancelOrderSchema, OpenOrderSchema, OrderSchema, message_to_engine::{CancelOrderPayload, CreateOrderPayload, MessageToEngine, OpenOrderPayload}}};

#[post("/order")]
async fn order(user_id: web::ReqData<String>, body:Json<OrderSchema>, data:web::Data<RedisManager> ) -> impl Responder{
    let order_id = Uuid::now_v7();    

    let parsed_price = match string_to_64(body.price.as_str()) {
        Ok(price) => {
            price
        },
        Err(err) => {
            println!("error occured while price parsing {}", err);
            return HttpResponse::BadRequest().json("price format not supported")
        }
    };
    let parsed_quantity = match string_to_64(body.quantity.as_str()) {
        Ok(qty) => {
            qty
        },
        Err(err) => {
            println!("error occured while quantity parsing {}", err);
            return HttpResponse::BadRequest().json("quantity format not supported")
        }
    };

    let res = data.send_and_await(MessageToEngine::CreateOrder(CreateOrderPayload {
        symbol: body.symbol.clone(),
        order_type: body.order_type.clone(),
        order_side: body.order_side.clone(),
        price: parsed_price,
        quantity: parsed_quantity,
        user_id: user_id.to_string(),
        order_id: order_id.to_string()
        
    })).await;
    
    match res {
        Ok(val) => {
            // return the direct response from engine to user
            HttpResponse::Ok().json(val)
        },
        Err(err) => {
            println!("error occured /order {}", err);
            HttpResponse::BadRequest().json("error occured while placing order")
        }
    }
}

#[delete("/cancel-order")]
async fn cancel_order(user_id:web::ReqData<String>, body:Json<CancelOrderSchema>, data: web::Data<RedisManager> ) -> impl Responder{
    // cancells the sitting order on the orderbook. even if it is partially or zero filled 

    let res = data.send_and_await(MessageToEngine::CancelOrder(CancelOrderPayload {
        symbol: body.symbol.clone(),
        order_id: body.order_id.clone(),
        user_id: user_id.to_string()
    })).await;

    match res {
        Ok(val) => {
            HttpResponse::Ok().json(val)
        },
        Err(err) => {
            println!("error occured /cancle {}", err);
            HttpResponse::BadRequest().json("error occured while cancelling order")
        }
    }
}

#[get("/open-orders")]
async fn get_orders (user_id:web::ReqData<String> , body: Json<OpenOrderSchema>, data: web::Data<RedisManager>) -> impl Responder{
     
    let res = data.send_and_await(MessageToEngine::GetOpenOrders(OpenOrderPayload {
            symbol: body.symbol.clone(),
            user_id: user_id.to_string()
    })).await;

    match res {
        Ok(val) => {
            HttpResponse::Ok().json(val)
        },
        Err(err) => {
            println!("error occured getting orders {}", err);
            HttpResponse::BadRequest().json("error occured while getting orders")
        }
    }
}


fn string_to_64(val_str: &str ) -> Result<u64, &'static str>{

    // parse string to decimal
    let dec = Decimal::from_str(val_str).map_err(|_| "Invalid decimal string format")?;

    // convert u64 scale number to Decimal type since we cant multiply with type above directly
    let scale_factor = Decimal::from_u64(SCALE_FACTOR).ok_or("Failed to convert scale_factor to decimal")?;

    // scale the decimal up
    let scaled_dec = (dec * scale_factor).normalize(); // strips any trailing 00 from decimal.
    // example- 3.1200 - 3.12. so the final value. 312000000. instead of 312000000.00
    
   // Guard against overflow/fraction overflow. means if there is any zero after "."
    // this means our backend only supports 8 decimals
    if scaled_dec.scale() > 0 {
        return Err("Input string has more decimal places than allowed precision");
    }
    scaled_dec.to_u64().ok_or("Value too large to fit in into 64")
}
