use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "trades")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub trade_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub market: String,
    pub price: i64,
    pub quantity: i64,
    pub buyer_user_id: Uuid,
    pub seller_user_id: Uuid,
    pub taker_side: String,
    pub executed_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
