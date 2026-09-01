use std::env;

use actix_web::{Error, HttpMessage, HttpResponse, Responder, body::MessageBody, dev::{ServiceRequest, ServiceResponse}, error::ErrorUnauthorized, http::header, middleware::Next, post, web::{self, Json}};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};

use crate::{Users, types::{JwtPayload, SigninResponse, Signupresponse, User, UserSignup}};

// middleware 
pub async fn auth_middleware(req:ServiceRequest, next: Next<impl MessageBody>) -> Result<ServiceResponse<impl MessageBody>, Error>{

        if let Some(jwt_token) = req.headers().get(header::AUTHORIZATION) && let Ok(token) = jwt_token.to_str() {

            let secret = env::var("jwtsecret").expect("jwtsecret must be set");
            let decoded = decode::<JwtPayload>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::new(jsonwebtoken::Algorithm::HS256));
            if let Ok(val) = decoded { // if jwt is verfied then set the username in req and call
                // return next function. else throw Error
                req.extensions_mut().insert(val.claims.username);
                     return  next.call(req).await
            }
        return  Err(ErrorUnauthorized("Wrong auth token sent"))
    }
    Err(ErrorUnauthorized("No auth token sent"))
}


#[post("/signup")]
// a new task for every request that came
pub async fn signup(body: Json<UserSignup>, data: web::Data<Users>) -> impl Responder {  // return type should soomething that implements Responder
    println!("{}", body.username);
    println!("{}", body.password);

    // create db entry
    
    let response = Signupresponse {
        body: "signed up successfully".to_string()
    };
    HttpResponse::Ok().json(response)
}

#[post("/signin")]
pub async fn signin(body: Json<UserSignup>, data: web::Data<Users>) -> impl Responder{
    let users = data.users.lock().unwrap();
    let user = users.get(&body.username);
    match user {
        Some(val)=>{
            if body.username == val.username && body.password == val.password {

                let payload = JwtPayload{
                    username: body.username.clone(),
                    exp: 1787181960
                };
                let secret = env::var("jwtsecret").expect("jwtsecret must be set");

                let token = encode(&Header::default(), &payload, &EncodingKey::from_secret(secret.as_bytes())).unwrap();

                let reponse = SigninResponse{
                    token
                };
                HttpResponse::Ok().json(reponse)
            } else {
                HttpResponse::BadRequest().body("wrong password please try again")
            }
        } ,
        None => HttpResponse::BadRequest().body("no user exist please signup")
    } 
}
