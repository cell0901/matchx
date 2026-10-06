use std::{collections::HashMap, env};

use chrono::Utc;
use redis::{
    AsyncCommands, AsyncConnectionConfig, Client, Value,
    streams::{StreamReadOptions, StreamReadReply},
};
use sea_orm::{Database, DatabaseConnection, EntityTrait, Set, sea_query::OnConflict};
use uuid::Uuid;

mod entities;

const STREAM: &str = "engine_events_stream";
const GROUP: &str = "db_filler_group";
const CONSUMER: &str = "db_filler_1";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().expect("no .env exist");

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not found in environment");
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL not found in environment");
    let db = Database::connect(database_url).await?;
    let client = Client::open(redis_url)?;

    // This connection is dedicated to XREADGROUP, whose block timeout is below
    // redis-rs's default response timeout.
    let config = AsyncConnectionConfig::new(); // config used to set options while setting up
    // connection of redis
    let mut conn = client
        .get_multiplexed_async_connection_with_config(&config)
        .await?;

    match conn.xgroup_create_mkstream(STREAM, GROUP, "$").await {
        Ok::<(), _>(()) => println!("created Redis consumer group {GROUP}"),
        Err(err) if err.to_string().contains("BUSYGROUP") => {}
        Err(err) => return Err(err.into()),
    }

    let options = StreamReadOptions::default()
        .group(GROUP, CONSUMER)
        .count(10)
        .block(200);
    let pending_options = StreamReadOptions::default()
        .group(GROUP, CONSUMER)
        .count(10);

    loop {
        // Redis keeps unacknowledged events in the consumer's pending list. Read
        // those first so a DB failure is retried before accepting new events.
        let pending: StreamReadReply = conn
            .xread_options(&[STREAM], &["0"], &pending_options)
            .await?;
        let reply: StreamReadReply = if pending.keys.is_empty() {
            conn.xread_options(&[STREAM], &[">"], &options).await?
        } else {
            pending
        };

        for stream in reply.keys {
            for entry in stream.ids {
                let fields = match fields_to_strings(&entry.map) {
                    Ok(fields) => fields,
                    Err(err) => {
                        eprintln!("invalid event {}: {err}", entry.id);
                        continue;
                    }
                };

                match write_event(&db, &fields).await {
                    Ok(()) => {
                        let _: i64 = conn.xack(STREAM, GROUP, &[entry.id]).await?;
                    }
                    Err(err) => {
                        eprintln!("database write failed for event {}: {err}", entry.id);
                    }
                }
            }
        }
    }
}

fn fields_to_strings(
    values: &HashMap<String, Value>,
) -> Result<HashMap<String, String>, redis::RedisError> {
    values
        .iter()
        .map(|(key, value)| Ok((key.clone(), redis::from_redis_value(value.clone())?)))
        .collect()
}

async fn write_event(
    db: &DatabaseConnection,
    fields: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let event_type = required(fields, "event_type")?;
    match event_type.as_str() {
        "trade_executed" => insert_trade(db, fields).await?,
        "order_status_changed" => upsert_order(db, fields).await?,
        event_type => return Err(format!("unknown event_type {event_type}").into()),
    }
    Ok(())
}

async fn insert_trade(
    db: &DatabaseConnection,
    fields: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let active = entities::trades::ActiveModel {
        trade_id: Set(parse_i64(fields, "trade_id")?),
        market: Set(required(fields, "market")?),
        price: Set(parse_i64(fields, "price")?),
        quantity: Set(parse_i64(fields, "quantity")?),
        buyer_user_id: Set(parse_uuid(fields, "buyer_user_id")?),
        seller_user_id: Set(parse_uuid(fields, "seller_user_id")?),
        taker_side: Set(required(fields, "taker_side")?),
        executed_at: Set(Utc::now().fixed_offset()),
    };
    entities::trades::Entity::insert(active)
        .on_conflict(
            OnConflict::columns([
                entities::trades::Column::TradeId,
                entities::trades::Column::Market,
            ])
            .do_nothing() // if already a trade_id, for Market then do nothing. this is imp because
                          // service could stop before XACK
            .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

async fn upsert_order(
    db: &DatabaseConnection,
    fields: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = Utc::now().fixed_offset();
    let active = entities::orders::ActiveModel {
        order_id: Set(parse_uuid(fields, "order_id")?),
        user_id: Set(parse_uuid(fields, "user_id")?),
        market: Set(required(fields, "market")?),
        side: Set(required(fields, "side")?),
        order_type: Set(required(fields, "order_type")?),
        price: Set(parse_i64(fields, "price")?),
        quantity: Set(parse_i64(fields, "quantity")?),
        filled_quantity: Set(parse_i64(fields, "filled_quantity")?),
        status: Set(required(fields, "status")?),
        created_at: Set(now),
        updated_at: Set(now),
    };

    entities::orders::Entity::insert(active)
        .on_conflict( // if alreay exists
            OnConflict::column(entities::orders::Column::OrderId)
                .update_columns([ // then update with these fields
                    entities::orders::Column::FilledQuantity,
                    entities::orders::Column::Status,
                    entities::orders::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

fn required(
    fields: &HashMap<String, String>,
    name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    fields
        .get(name)
        .cloned()
        .ok_or_else(|| format!("missing field {name}").into())
}

fn parse_uuid(
    fields: &HashMap<String, String>,
    name: &str,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    let value = required(fields, name)?; // required finds the field as string 
    let number = value.parse::<Uuid>()?; // parse the string to uuid or error result if error
    Ok(number)
}

fn parse_i64(
    fields: &HashMap<String, String>,
    name: &str,
) -> Result<i64, Box<dyn std::error::Error>> {
    let value = required(fields, name)?; // required finds the field as string 
    let number = value.parse::<i64>()?; // parse to i64 or error result 
    Ok(number)
}
