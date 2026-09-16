use reqwest::Client;
use rust_cex::{MessageFromEngine, routes::Asset, types::{DepositSchema, OnrampSchema, UserSignup, message_from_engine::Code::{DepositSuccess, OnrampSuccess}}};
use serde::Deserialize;
use uuid::Uuid;

const BACKEND_URL:&str= "http://localhost:8080";
#[derive(Deserialize)]
pub struct SigninRes {
    pub token: String
}

async fn create_user() -> String {
    let client = Client::new();

    let username = format!("test_{}", Uuid::now_v7());
    let password = "1234".to_string();

    let data = UserSignup {
        username,
        password,
    };

    let signup = client
        .post(format!("{}/signup", BACKEND_URL))
        .json(&data)
        .send()
        .await
        .unwrap();

    assert!(signup.status().is_success());

    let signin = client
        .post(format!("{}/signin", BACKEND_URL))
        .json(&data)
        .send()
        .await
        .unwrap();

    assert!(signup.status().is_success());
    let user: SigninRes = signin.json().await.unwrap();

    user.token
}

#[tokio::test]
async fn onramp() {
    let client = Client::new();

    let user_token  = create_user().await;

    let payload = OnrampSchema {
        amount: "500".to_string()
    };

    let res = client.post(format!("{}/balance/onramp", BACKEND_URL)).json(&payload).header("Authorization" , &user_token).send().await.expect("error while sending post reqwest");

    assert!(res.status().is_success());
    
    let data: MessageFromEngine = res.json().await.expect("error while Deserialize");

    match data {
        MessageFromEngine::OnrampResponse(val) => {
            assert_eq!(val.code, OnrampSuccess);
        }
        _=> panic!("Expected OnrampResponse, got other"),
    }
}

#[tokio::test]
async fn deposit() {
    let client = Client::new();


    let user_token  = create_user().await;

    let payload = DepositSchema{
        asset: "SOL".to_string(),
        quantity: "5".to_string()
    };

    let res = client.post(format!("{}/balance/deposit", BACKEND_URL)).json(&payload).header("Authorization" , &user_token).send().await.expect("error while sending post reqwest");

    assert!(res.status().is_success());

    let data: MessageFromEngine = res.json().await.expect("error while Deserialize");

    match data {
        MessageFromEngine::DepositResponse(val) => {
            assert_eq!(val.code, DepositSuccess);
        }
        _=> panic!("Expected OnrampResponse, got other"),
    };
}

#[tokio::test]
async fn get_balance() {
    let client = Client::new();
    let (user_token, amount)= onramp_helper().await;

    let res = client.get(format!("{}/balance", BACKEND_URL)).query(&[("asset", "USDC")]).header("Authorization", &user_token).send().await.expect("error while sending post reqwest");
    assert!(res.status().is_success());

    
    match res.json().await.expect("error occured while Deserialize") {
        MessageFromEngine::GetBalance(val) => {
            assert_eq!(val.balance.available , amount);
        }
        _=> panic!("Expected OnrampResponse, got other"),
    };
}

#[tokio::test]
async fn get_balance_deposit() {
    let client = Client::new();
    let (user_token, amount, asset)= deposit_helper().await;

    let res = client.get(format!("{}/balance", BACKEND_URL)).query(&[("asset", &asset)]).header("Authorization", &user_token).send().await.expect("error while sending post reqwest");
    assert!(res.status().is_success());

    
    match res.json().await.expect("error occured while Deserialize") {
        MessageFromEngine::GetBalance(val) => {
            assert_eq!(val.balance.available , amount);
            assert_eq!(val.asset, asset.parse::<Asset>().unwrap());
        }
        _=> panic!("Expected OnrampResponse, got other"),
    };
}

async fn onramp_helper () -> (String, String){
    let client = Client::new();

    let user_token = create_user().await;

    let amount = "500";
    let payload = OnrampSchema {
        amount:amount.to_string()
    };

    let res = client.post(format!("{}/balance/onramp", BACKEND_URL)).json(&payload).header("Authorization" , &user_token).send().await.expect("error while sending post reqwest");

    assert!(res.status().is_success());
    
    let data: MessageFromEngine = res.json().await.expect("error while Deserialize");

    match data {
        MessageFromEngine::OnrampResponse(val) => {
            assert_eq!(val.code, OnrampSuccess);
        }
        _=> panic!("Expected OnrampResponse, got other"),
    };
    (user_token, amount.to_string())
}


async fn deposit_helper () -> (String, String, String){
    let client = Client::new();

    let user_token = create_user().await;

    let qty = "500";
    let asset = "SOL";
    let payload = DepositSchema {
        asset: asset.to_string(),
        quantity: qty.to_string()
    };

    let res = client.post(format!("{}/balance/deposit", BACKEND_URL)).json(&payload).header("Authorization" , &user_token).send().await.expect("error while sending post reqwest");

    assert!(res.status().is_success());
    
    let data: MessageFromEngine = res.json().await.expect("error while Deserialize");

    match data {
        MessageFromEngine::DepositResponse(val) => {
            assert_eq!(val.code, DepositSuccess);
        }
        _=> panic!("Expected OnrampResponse, got other"),
    };
    (user_token, qty.to_string(), asset.to_string())
}
