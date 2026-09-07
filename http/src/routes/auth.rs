use std::{env, time::{SystemTime, UNIX_EPOCH}};

use actix_web::{Error, HttpMessage, HttpResponse, Responder, body::MessageBody, dev::{ServiceRequest, ServiceResponse}, error::ErrorUnauthorized, http::header, middleware::Next, post, web::{self, Json}};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, NotSet, QueryFilter, Set};

use crate::{entities::users, types::{JwtPayload, SigninResponse, Signupresponse, UserSignup }};

// middleware 
pub async fn auth_middleware(req:ServiceRequest, next: Next<impl MessageBody>) -> Result<ServiceResponse<impl MessageBody>, Error>{

    if let Some(jwt_token) = req.headers().get(header::AUTHORIZATION) 
            && let Ok(token) = jwt_token.to_str() { // checks if the value can be converted into string

        let secret = env::var("JWT_SECRET").expect("jwtsecret must be set");
        let decoded = decode::<JwtPayload>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::new(jsonwebtoken::Algorithm::HS256));
        if let Ok(val) = decoded { 
            // if jwt is verfied then set the id in req and call
            // return next function. else throw Error
            req.extensions_mut().insert(val.claims.id);
            return next.call(req).await;
        }
        return  Err(ErrorUnauthorized("Wrong auth token sent"));
    }
    Err(ErrorUnauthorized("No auth token sent or wrong format sent"))
}


#[post("/signup")]
// a new task for every request that came
pub async fn signup(body: Json<UserSignup>, data: web::Data<DatabaseConnection>) -> impl Responder {  // return type should soomething that implements Responder
    // create db entry
   
    let a= users::ActiveModel{
        id: NotSet,
        username: Set(body.username.clone()),
        // TODO hash the password
        password: Set(body.password.clone())
    };

    // returns read only database updates model
    let res= a.insert(&**data).await; 

    match res {
        Ok(val) => {
            println!("singup success id {}", val.id);
            HttpResponse::Ok().json(Signupresponse {
                body: "Signup successfull".to_string()
            })
        },
        Err(DbErr::Query(_err)) => { // if this specific error comes then send this request
            HttpResponse::BadRequest().json(Signupresponse {
                body: "user already exists".to_string()
            })
        },
        Err(_) => {
            HttpResponse::InternalServerError().json("error while signup")
        }
    }
}

#[post("/signin")]
pub async fn signin(body: Json<UserSignup>, data: web::Data<DatabaseConnection>) -> impl Responder{
    let res = users::Entity::find()
        .filter(users::Column::Username.contains(body.username.clone()))
        .filter(users::Column::Password.contains(body.password.clone()))
        .one(&**data) // get one model from the select query
        .await;
   
   match res {
        Ok(val) => {
            if let Some(user) = val {
                // generate token and return
                let secret = env::var("JWT_SECRET").expect("jwtsecret must be set");

                let payload = JwtPayload{
                    id: user.id,
                    // 7 days from login 
                    exp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()+ 7 * 24 * 60 * 60
                };
                let token = encode(&Header::default(), &payload, &EncodingKey::from_secret(secret.as_bytes())).unwrap();

                HttpResponse::Ok().json(SigninResponse {
                    token
                })
            } else {
                HttpResponse::BadRequest().json("no user found. please signup")
            }
        },
        Err(err) => {
            println!("db error {}", err);
            HttpResponse::InternalServerError().json("error while signin")
        }
   } 
}
