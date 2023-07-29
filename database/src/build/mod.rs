use std::path::PathBuf;

mod connect;
mod migration;

pub use connect::LucilleDbConnectOptions;
pub(crate) use migration::get_db_migration_status;
pub use migration::{LucilleMigrationManager, MigrationRecord};

use crate::{drop_everything_sqlx, Database, DatabaseError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatabaseConnectState {
    Init,
    Configured,
    Connected,
    Ready,
}

#[derive(Debug, Clone, Default)]
pub struct DatabaseBuider {
    opts: Option<LucilleDbConnectOptions>,
    migration: Option<LucilleMigrationManager>,
    src: Option<DatabaseSource>,
    migration_results: Option<Vec<MigrationRecord>>,
}

impl DatabaseBuider {
    pub fn current_state(&self) -> DatabaseConnectState {
        let rdy = self.migration.as_ref().map(|m| m.done).unwrap_or(false);
        if self.opts.is_none() {
            DatabaseConnectState::Init
        } else if self.migration.is_none() {
            DatabaseConnectState::Configured
        } else if self.migration.is_some() && !rdy {
            DatabaseConnectState::Connected
        } else if rdy && self.src.is_some() {
            DatabaseConnectState::Ready
        } else {
            unreachable!("state error in database builder")
        }
    }
    pub fn get_migration_results(&self) -> Option<&[MigrationRecord]> {
        self.migration_results.as_deref()
    }
    pub fn add_opts(&mut self, opts: LucilleDbConnectOptions) -> Result<(), DatabaseError> {
        if self.opts.is_some() {
            return Err(DatabaseError::ConnectStateError(
                "database builder already has options",
            ));
        }
        self.opts = Some(opts);
        Ok(())
    }
    pub async fn connect(&mut self) -> Result<(), DatabaseError> {
        if self.migration.is_some() {
            return Err(DatabaseError::ConnectStateError(
                "database builder is already connected",
            ));
        }
        let opts = self.opts.as_ref().ok_or_else(|| {
            DatabaseError::ConnectStateError("database builder does not have opts")
        })?;
        let (pool, src) = opts.create_pool().await?;
        self.migration = Some(LucilleMigrationManager::new(pool));
        self.src = Some(src);
        Ok(())
    }
    pub async fn migrate(&mut self) -> Result<(), DatabaseError> {
        if self.migration.as_ref().map(|m| m.done).unwrap_or(false) {
            return Err(DatabaseError::ConnectStateError(
                "database builder is already ready",
            ));
        }
        let mgr = self.migration.as_mut().ok_or_else(|| {
            DatabaseError::ConnectStateError("database builder does not have connection")
        })?;
        match mgr.run().await {
            Ok(()) => Ok(()),
            Err(e) => {
                // if we were unable to migrate, try to log the migration status
                //
                // ideally the caller will check/report this themselves
                match mgr.get_db_migration_status().await {
                    Ok(hist) => {
                        log::warn!("migration history: {:#?}", hist);
                        self.migration_results = Some(hist)
                    }
                    Err(e) => {
                        log::error!("could not get migration history: {:?}", e)
                    }
                };
                Err(e)
            }
        }
    }

    /// Will completely destroy any data.
    pub async fn drop_database(&mut self) -> Result<(), DatabaseError> {
        if let Some(pool) = self.migration.as_ref().map(|m| &m.pool) {
            super::drop_everything(pool)
                .await
                .map(|_| self.migration = None)
        } else if let Some(opts) = &self.opts {
            drop_everything_sqlx(&opts.source.to_url()).await
        } else {
            Err(DatabaseError::NoDatabaseSpecified)
        }
    }

    pub fn into_parts(self) -> Result<(Database, DatabaseSource), DatabaseError> {
        let mgr = self
            .migration
            .ok_or_else(|| DatabaseError::ConnectStateError("database is not connected"))?;
        let src = self
            .src
            .ok_or_else(|| DatabaseError::ConnectStateError("no source for database"))?;
        if !mgr.done {
            return Err(DatabaseError::ConnectStateError(
                "database must verify migrations",
            ));
        }
        let pool = mgr.pool;
        Ok((Database { pool }, src))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseSource {
    Memory,
    Url(String),
    Path(PathBuf),
}
impl DatabaseSource {
    fn to_url(&self) -> String {
        match self {
            DatabaseSource::Memory => "sqlite::memory:".to_owned(),
            DatabaseSource::Url(u) => u.clone(),
            DatabaseSource::Path(p) => format!("sqlite:{}", p.display()),
        }
    }
}
