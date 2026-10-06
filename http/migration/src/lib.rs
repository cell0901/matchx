pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20261006_000002_create_trade_and_order_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20261006_000002_create_trade_and_order_tables::Migration),
        ]
    }
}
