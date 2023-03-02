use futures::TryStreamExt;
use lucille_core::{identifiers::CorpusId, Corpus};

use crate::{Database, DatabaseError};

impl Database {
    pub async fn add_corpus<S: Into<String>>(&self, name: S) -> Result<Corpus, DatabaseError> {
        let name = name.into();
        let id = sqlx::query!(
            r#"
                    INSERT INTO corpus (title)
                    VALUES ( ?1 )
                    "#,
            name,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        let cid = CorpusId::new(id);

        Ok(Corpus {
            id: Some(cid),
            title: name,
        })
    }

    pub async fn get_corpus_id(&self, title: &str) -> Result<Option<CorpusId>, DatabaseError> {
        let id = sqlx::query!(
            r#"
            SELECT 
                id
            FROM 
                corpus
            WHERE
                title = ?
         "#,
            title,
        )
        .map(|r| CorpusId::new(r.id))
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn get_corpus(&self, id: CorpusId) -> Result<Corpus, DatabaseError> {
        let cid = id.get();
        let corpus = sqlx::query!(
            r#"
            SELECT 
                id, title
            FROM 
                corpus
            WHERE
                id = ?
         "#,
            cid
        )
        .map(|r| Corpus {
            id: Some(CorpusId::new(r.id)),
            title: r.title,
        })
        .fetch_one(&self.pool)
        .await?;
        Ok(corpus)
    }

    pub async fn get_or_add_corpus<S: Into<String>>(
        &self,
        name: S,
    ) -> Result<Corpus, DatabaseError> {
        let name = name.into();
        Ok(match self.get_corpus_id(&name).await? {
            Some(id) => Corpus {
                id: Some(id),
                title: name,
            },
            None => self.add_corpus(name).await?,
        })
    }

    pub async fn list_corpus(&self) -> Result<Vec<Corpus>, DatabaseError> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                id, title
            FROM 
                corpus
         "#
        )
        .map(|r| Corpus {
            id: Some(CorpusId::new(r.id)),
            title: r.title,
        })
        .fetch(&self.pool);

        Ok(rows.try_collect().await?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::database_test::assert_err_is_constraint;

    #[tokio::test]
    async fn create_new_corpus() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        assert_eq!(c.id, Some(CorpusId::new(1)));
        assert_eq!(c.title, "media");
    }

    #[tokio::test]
    async fn create_new_empty_corpus() {
        let db = Database::memory().await.unwrap();
        assert_err_is_constraint(db.add_corpus("").await, "CHECK")
    }

    #[tokio::test]
    async fn fail_to_create_two_identical_corpuses() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        assert_eq!(c.id, Some(CorpusId::new(1)));
        assert_eq!(c.title, "media");
        assert_err_is_constraint(db.add_corpus("media").await, "UNIQUE")
    }

    #[tokio::test]
    async fn lookup_corpus_id_from_title() {
        let db = Database::memory().await.unwrap();
        let c1 = db.add_corpus("media").await.unwrap();
        let c2 = db.add_corpus("media2").await.unwrap();
        let cid1 = db.get_corpus_id("media").await.unwrap();
        let cid2 = db.get_corpus_id("media2").await.unwrap();
        assert_eq!(c1.id, cid1);
        assert_eq!(c2.id, cid2);
    }

    #[tokio::test]
    async fn lookup_corpus_from_id() {
        let db = Database::memory().await.unwrap();
        let c1 = db.add_corpus("media").await.unwrap();
        let c2 = db.add_corpus("media2").await.unwrap();
        let c_lookup1 = db.get_corpus(c1.id.unwrap()).await.unwrap();
        let c_lookup2 = db.get_corpus(c2.id.unwrap()).await.unwrap();
        assert_eq!(c1, c_lookup1);
        assert_eq!(c2, c_lookup2);
    }

    #[tokio::test]
    async fn get_or_add_corpus() {
        let db = Database::memory().await.unwrap();
        let c1 = db.get_or_add_corpus("media").await.unwrap();
        let c2 = db.get_or_add_corpus("media2").await.unwrap();
        let c1_2 = db.get_or_add_corpus("media").await.unwrap();
        let c2_2 = db.get_or_add_corpus("media2").await.unwrap();
        assert_eq!(c1, c1_2);
        assert_eq!(c2, c2_2);
    }

    #[tokio::test]
    async fn list_all_corpus() {
        let db = Database::memory().await.unwrap();
        assert_eq!(db.list_corpus().await.unwrap(), vec![]);
        let c1 = db.get_or_add_corpus("media").await.unwrap();
        assert_eq!(db.list_corpus().await.unwrap(), vec![c1.clone()]);
        let c2 = db.get_or_add_corpus("media2").await.unwrap();
        assert_eq!(db.list_corpus().await.unwrap(), vec![c1, c2]);
    }
}
