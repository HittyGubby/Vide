use sea_orm::{ColumnTrait, DbErr, EntityTrait, FromQueryResult, QueryFilter as _, Set};

use crate::infra::db::Database;
use crate::models::{MetaTag, SongId, Tag};

/// Add a tag to a song. Ignores duplicates.
pub async fn add_tag(db: &Database, song_id: &SongId, namespace: &str, value: &str) -> Result<(), DbErr> {
    use crate::infra::db::entity::tag;

    let active = tag::ActiveModel {
        song_id: Set(song_id.0.clone()),
        namespace: Set(namespace.to_string()),
        value: Set(value.to_string()),
    };

    tag::Entity::insert(active)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns([
                tag::Column::SongId,
                tag::Column::Namespace,
                tag::Column::Value,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(&db.conn)
        .await?;

    Ok(())
}

/// Remove a specific tag from a song.
pub async fn remove_tag(db: &Database, song_id: &SongId, namespace: &str, value: &str) -> Result<(), DbErr> {
    use crate::infra::db::entity::tag;

    tag::Entity::delete_many()
        .filter(tag::Column::SongId.eq(&song_id.0))
        .filter(tag::Column::Namespace.eq(namespace))
        .filter(tag::Column::Value.eq(value))
        .exec(&db.conn)
        .await?;

    Ok(())
}

/// Get all tags for a specific song.
pub async fn get_song_tags(db: &Database, song_id: &SongId) -> Result<Vec<Tag>, DbErr> {
    use crate::infra::db::entity::tag;
    use sea_orm::QueryFilter;

    let rows = tag::Entity::find()
        .filter(tag::Column::SongId.eq(&song_id.0))
        .all(&db.conn)
        .await?;

    Ok(rows
        .into_iter()
        .map(|t| Tag {
            namespace: t.namespace,
            value: t.value,
        })
        .collect())
}

/// Get all distinct tag namespaces across the entire library.
pub async fn get_namespaces(db: &Database) -> Result<Vec<String>, DbErr> {
    use crate::infra::db::entity::tag;
    use sea_orm::FromQueryResult;
    use sea_orm::QuerySelect;

    #[derive(FromQueryResult)]
    struct NamespaceRow {
        namespace: String,
    }

    let rows = tag::Entity::find()
        .select_only()
        .column(tag::Column::Namespace)
        .distinct()
        .into_model::<NamespaceRow>()
        .all(&db.conn)
        .await?;

    Ok(rows.into_iter().map(|r| r.namespace).collect())
}

/// Build meta-tag tree: aggregate tags by namespace, return MetaTag list.
/// These are used for UI organization and are not queryable.
pub async fn get_meta_tags(db: &Database) -> Result<Vec<MetaTag>, DbErr> {
    use crate::infra::db::entity::tag;
    use sea_orm::QuerySelect;
    use std::collections::HashMap;

    // Fetch all distinct (namespace, value) pairs
    #[derive(FromQueryResult)]
    struct TagRow {
        namespace: String,
        value: String,
    }

    let rows = tag::Entity::find()
        .select_only()
        .column(tag::Column::Namespace)
        .column(tag::Column::Value)
        .distinct()
        .into_model::<TagRow>()
        .all(&db.conn)
        .await?;

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        groups.entry(row.namespace).or_default().push(row.value);
    }

    let meta_tags: Vec<MetaTag> = groups
        .into_iter()
        .map(|(namespace, mut children)| {
            children.sort();
            MetaTag {
                display_name: namespace.clone(),
                namespace,
                children,
            }
        })
        .collect();

    Ok(meta_tags)
}
