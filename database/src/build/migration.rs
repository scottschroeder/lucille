use sqlx::{
    migrate::{Migrate, Migrator},
    Pool, Sqlite,
};

use crate::DatabaseError;

static MIGRATOR: Migrator = sqlx::migrate!();

#[derive(Debug)]
pub struct LucileMigrationManager {
    pub(crate) pool: Pool<Sqlite>,
    pub(crate) done: bool,
}

impl LucileMigrationManager {
    pub fn new(pool: Pool<Sqlite>) -> LucileMigrationManager {
        LucileMigrationManager { pool, done: false }
    }

    pub async fn run(&mut self) -> Result<(), DatabaseError> {
        log::trace!("check/run migrations against database");
        MIGRATOR.run(&self.pool).await?;
        self.done = true;
        Ok(())
    }

    pub async fn get_db_migration_status(&self) -> Result<Vec<MigrationRecord>, DatabaseError> {
        get_db_migration_status(&self.pool).await
    }
}

pub(crate) async fn get_db_migration_status(
    pool: &Pool<Sqlite>,
) -> Result<Vec<MigrationRecord>, DatabaseError> {
    let resolved = get_resolved_migrations();
    let applied = get_applied_migrations(pool).await?;
    let results = collate_migration_status(resolved.into_iter(), applied.into_iter());
    Ok(results)
}

fn get_resolved_migrations() -> Vec<(i64, String)> {
    let mut m = MIGRATOR
        .migrations
        .iter()
        .map(|m| (m.version, m.description.to_string()))
        .collect::<Vec<_>>();
    m.sort_unstable_by_key(|x| x.0);
    m
}

async fn get_applied_migrations(pool: &Pool<Sqlite>) -> Result<Vec<i64>, DatabaseError> {
    let mut c = pool.acquire().await?;
    c.ensure_migrations_table().await?;
    let applied_migrations = c.list_applied_migrations().await?;
    let mut m = applied_migrations
        .into_iter()
        .map(|m| m.version)
        .collect::<Vec<_>>();
    m.sort_unstable();
    Ok(m)
}

#[derive(Debug, PartialEq)]
pub struct MigrationRecord {
    pub id: i64,
    pub description: Option<String>,
    pub resolved: bool,
    pub applied: bool,
}

impl MigrationRecord {
    fn new(plan: Option<(i64, String)>, present: Option<i64>) -> MigrationRecord {
        let is_planned = plan.is_some();
        let is_present = present.is_some();

        let id = present
            .or_else(|| plan.as_ref().map(|p| p.0))
            .expect("both plan & present were None");

        MigrationRecord {
            id,
            description: plan.map(|p| p.1),
            resolved: is_planned,
            applied: is_present,
        }
    }
}

fn collate_migration_status(
    mut resolved: impl Iterator<Item = (i64, String)>,
    mut applied: impl Iterator<Item = i64>,
) -> Vec<MigrationRecord> {
    let mut result = Vec::new();

    let mut next_resolved = resolved.next();
    let mut next_applied = applied.next();

    while next_applied.is_some() || next_resolved.is_some() {
        let cid = next_resolved.as_ref().map(|c| c.0);
        let (r_resolved, r_applied) = match compare_ids(cid, next_applied) {
            std::cmp::Ordering::Less => (next_resolved.take(), None),
            std::cmp::Ordering::Equal => (next_resolved.take(), next_applied.take()),
            std::cmp::Ordering::Greater => (None, next_applied.take()),
        };
        if next_resolved.is_none() {
            next_resolved = resolved.next();
        }
        if next_applied.is_none() {
            next_applied = applied.next();
        }
        let record = MigrationRecord::new(r_resolved, r_applied);
        result.push(record);
    }
    result
}

fn compare_ids(a: Option<i64>, b: Option<i64>) -> std::cmp::Ordering {
    match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(a), Some(b)) => a.cmp(&b),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::build::connect::LucileDbConnectOptions;

    /// These tests rely on the current migration sql files
    /// which can change. It should be sufficient to test using just
    /// the existing ones at this time, and not have things break if
    /// more are added.
    const THREE_MIGRATIONS: &[(i64, &str)] = &[
        (20220828143729, "create"),
        (20230224122153, "media segment unique seq"),
        (20230224134432, "enable cascade delete"),
    ];
    const LEN_MIGRATIONS: usize = THREE_MIGRATIONS.len();

    fn ref_migrations(data: &[(i64, String)]) -> Vec<(i64, &str)> {
        data.iter()
            .map(|(idx, desc)| (*idx, desc.as_str()))
            .collect()
    }

    async fn empty_migration_manager() -> LucileMigrationManager {
        let (pool, _) = LucileDbConnectOptions::memory()
            .create_pool()
            .await
            .unwrap();

        LucileMigrationManager::new(pool)
    }

    #[test]
    fn get_migration_configuration_for_tests() {
        let data = get_resolved_migrations();
        assert_eq!(
            &ref_migrations(&data).as_slice()[0..LEN_MIGRATIONS],
            THREE_MIGRATIONS
        )
    }

    #[tokio::test]
    async fn new_db_has_no_migrations() {
        let mgr = empty_migration_manager().await;
        let ids = get_applied_migrations(&mgr.pool).await.unwrap();
        assert!(ids.is_empty())
    }

    #[tokio::test]
    async fn migration_completes() {
        let mut mgr = empty_migration_manager().await;
        mgr.run().await.unwrap();
    }

    #[tokio::test]
    async fn new_db_has_migrations() {
        let mut mgr = empty_migration_manager().await;
        mgr.run().await.unwrap();
        let ids = get_applied_migrations(&mgr.pool).await.unwrap();
        let configured_migrations = get_resolved_migrations()
            .into_iter()
            .map(|x| x.0)
            .collect::<Vec<_>>();
        assert_eq!(ids, configured_migrations);
    }

    #[tokio::test]
    async fn new_db_history_is_complete() {
        let mut mgr = empty_migration_manager().await;
        mgr.run().await.unwrap();
        let status = mgr.get_db_migration_status().await.unwrap();
        let ids = get_applied_migrations(&mgr.pool).await.unwrap();
        assert_eq!(ids.len(), status.len());
        for (idx, record) in ids.iter().zip(status.iter()) {
            assert_eq!(record.id, *idx);
            assert!(record.resolved);
            assert!(record.applied);
        }
    }

    #[tokio::test]
    async fn blank_db_history_is_all_new() {
        let mgr = empty_migration_manager().await;
        let status = mgr.get_db_migration_status().await.unwrap();
        for record in status {
            assert!(record.resolved);
            assert!(!record.applied);
        }
    }

    fn test_collation(config: &[i64], db: &[i64]) -> Vec<MigrationRecord> {
        let migrations = config
            .iter()
            .map(|id| (*id, id.to_string()))
            .collect::<Vec<_>>();
        let existing = db.to_vec();
        collate_migration_status(migrations.into_iter(), existing.into_iter())
    }

    #[test]
    fn collate_empty() {
        assert_eq!(test_collation(&[], &[]), vec![]);
    }

    #[test]
    fn collate_only_new() {
        assert_eq!(
            test_collation(&[1, 2], &[]),
            vec![
                MigrationRecord {
                    id: 1,
                    description: Some(1.to_string()),
                    resolved: true,
                    applied: false,
                },
                MigrationRecord {
                    id: 2,
                    description: Some(2.to_string()),
                    resolved: true,
                    applied: false,
                },
            ]
        );
    }

    #[test]
    fn collate_only_existing() {
        assert_eq!(
            test_collation(&[], &[1, 2]),
            vec![
                MigrationRecord {
                    id: 1,
                    description: None,
                    resolved: false,
                    applied: true,
                },
                MigrationRecord {
                    id: 2,
                    description: None,
                    resolved: false,
                    applied: true,
                },
            ]
        );
    }

    #[test]
    fn collate_one_behind() {
        assert_eq!(
            test_collation(&[1, 2], &[1]),
            vec![
                MigrationRecord {
                    id: 1,
                    description: Some(1.to_string()),
                    resolved: true,
                    applied: true,
                },
                MigrationRecord {
                    id: 2,
                    description: Some(2.to_string()),
                    resolved: true,
                    applied: false,
                },
            ]
        );
    }

    #[test]
    fn unknown_migration_on_db() {
        assert_eq!(
            test_collation(&[2, 3], &[1]),
            vec![
                MigrationRecord {
                    id: 1,
                    description: None,
                    resolved: false,
                    applied: true,
                },
                MigrationRecord {
                    id: 2,
                    description: Some(2.to_string()),
                    resolved: true,
                    applied: false,
                },
                MigrationRecord {
                    id: 3,
                    description: Some(3.to_string()),
                    resolved: true,
                    applied: false,
                },
            ]
        );
    }

    #[test]
    fn unknown_config_in_the_middle() {
        assert_eq!(
            test_collation(&[1, 2, 3], &[1, 3]),
            vec![
                MigrationRecord {
                    id: 1,
                    description: Some(1.to_string()),
                    resolved: true,
                    applied: true,
                },
                MigrationRecord {
                    id: 2,
                    description: Some(2.to_string()),
                    resolved: true,
                    applied: false,
                },
                MigrationRecord {
                    id: 3,
                    description: Some(3.to_string()),
                    resolved: true,
                    applied: true,
                },
            ]
        );
    }
}
