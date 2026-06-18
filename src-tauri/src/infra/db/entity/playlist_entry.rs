use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "playlist_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub playlist_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub song_id: String,
    pub import_order: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
