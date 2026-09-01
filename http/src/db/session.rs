use std::error::Error;

use sea_orm::{Database, DatabaseConnection};

pub struct DbSession{
    session: DatabaseConnection
}

impl DbSession {
   async fn connect (&mut self) -> Result<DatabaseConnection, Box<dyn Error> >{
       self.session = Database::connect("protocol://username:password@host/database").await?; 
        Ok(self.session.clone())
    }
}
