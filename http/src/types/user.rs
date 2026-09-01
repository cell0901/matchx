use serde::{Deserialize, Serialize};


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
   pub username:String,
   pub exp: usize
}


