use serde::{Deserialize, Serialize};
use uuid::Uuid;


#[derive(Debug)]
pub struct User {
    pub username:String,
    pub password:String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserSignup {
 pub username: String,
 pub password: String
}

#[derive(Serialize, Deserialize )]
pub struct Signupresponse {
    pub body: String
}

#[derive(Serialize, Deserialize )]
pub struct SigninResponse{
    pub token: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JwtPayload {
   pub id: Uuid,
   pub exp: u64
}


