use std::collections::HashSet;

use futures::TryStreamExt;
use lucille_core::uuid::Uuid;
use sqlx::{QueryBuilder, Sqlite};

use crate::{parse_uuid, Database, DatabaseError};

impl Database {
    pub async fn assoc_index_with_srts(
        &self,
        index_uuid: Uuid,
        srts: HashSet<i64>,
    ) -> Result<(), DatabaseError> {
        log::debug!(
            "associating {} srt files with search index {}",
            srts.len(),
            index_uuid
        );
        let uuid = index_uuid.to_string();
        let id = sqlx::query!(
            r#"
                    INSERT INTO search_index (uuid)
                    VALUES ( ?1 )
                    "#,
            uuid
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        let mut insert_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new(r#"INSERT INTO search_assoc (search_index_id, srt_id)"#);

        insert_builder.push_values(srts.iter(), |mut b, srt| {
            b.push_bind(id).push_bind(srt);
        });
        let query = insert_builder.build();

        query.execute(&self.pool).await?;

        Ok(())
    }

    pub async fn get_search_indexes(&self) -> Result<Vec<Uuid>, DatabaseError> {
        let mut rows = sqlx::query!(
            r#"
                SELECT 
                    uuid
                FROM search_index
                ORDER BY
                    id
         "#,
        )
        .map(|r| r.uuid)
        .fetch(&self.pool);

        let mut results = Vec::new();
        while let Some(row) = rows.try_next().await.unwrap() {
            let uuid = parse_uuid(&row)?;
            results.push(uuid);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod test {}
