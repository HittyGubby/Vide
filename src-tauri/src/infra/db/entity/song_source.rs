use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "song_sources")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub song_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub platform: String,
    pub source_id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
