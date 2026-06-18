use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "playlists")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub playlist_id: String,
    pub name: String,
    pub source_type: String,
    pub source_path: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
