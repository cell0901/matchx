pub mod manager;
pub mod types;

use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, get, rt, web};

use crate::manager::{UserManager, handle_connection};


#[get("/")]
async fn ws(req: HttpRequest, stream: web::Payload, data: web::Data<UserManager>) -> Result<HttpResponse, Error> {

    // current ws session and stream of incmoing messages
    let (res, session , stream) = actix_ws::handle(&req, stream)?; // handle the incmoing http request and upgrade to ws
    println!("/ hit");

    let connection_id = data.add_user(session.clone()).await; // this returns connection id for
    // handleconnection to remove on client disconnect

    // ws message doesnt arrive in one go 
    // example "hello there"
    // woould arrive in two parts and "hello" and "there". this  combines conitnouse frames for
    // that message and see in one go
    let stream = stream.aggregate_continuations().max_continuation_size(2_usize.pow(20));
    
    // we dont need the session as already storing in UserInfo 
    rt::spawn(handle_connection(data, stream, connection_id));
    Ok(res)
}


#[tokio::main]
async fn main() -> std::io::Result<()>{
    dotenvy::dotenv().expect("no .env exist in ws");

    let users = web::Data::new(UserManager::new());
    let server = HttpServer::new(move || 
        App::new().service(ws)
            .app_data(users.clone())
    )
        .bind(("localhost", 8081))?
        .run();
    println!("ws on 8081");
    server.await
}
