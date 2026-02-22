use crate::models::{NewScanEvent, ScanEvent};
use anyhow::Context;
use autopulse_utils::sify;
use diesel::connection::SimpleConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
#[cfg(feature = "mysql")]
use diesel::QueryDsl;
use diesel::{Connection, RunQueryDsl};
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use diesel::{SaveChangesDsl, SelectableHelper};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde::Deserialize;
use std::path::PathBuf;
use tracing::info;

#[doc(hidden)]
#[cfg(feature = "postgres")]
const POSTGRES_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/postgres");

#[doc(hidden)]
#[cfg(feature = "sqlite")]
const SQLITE_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/sqlite");

#[doc(hidden)]
#[cfg(feature = "mysql")]
const MYSQL_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/mysql");

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum DatabaseType {
    #[cfg(feature = "sqlite")]
    #[cfg_attr(feature = "sqlite", default)]
    Sqlite,
    #[cfg(feature = "postgres")]
    #[cfg_attr(not(feature = "sqlite"), default)]
    Postgres,
    #[cfg(feature = "mysql")]
    #[cfg_attr(not(any(feature = "sqlite", feature = "postgres")), default)]
    Mysql,
}

impl DatabaseType {
    pub fn default_url(&self) -> String {
        match self {
            #[cfg(feature = "sqlite")]
            Self::Sqlite => "sqlite://data/autopulse.db".to_string(),
            #[cfg(feature = "postgres")]
            Self::Postgres => "postgres://autopulse:autopulse@localhost:5432/autopulse".to_string(),
            #[cfg(feature = "mysql")]
            Self::Mysql => "mysql://autopulse:autopulse@127.0.0.1:3306/autopulse".to_string(),
        }
    }
}

/// Represents a connection to a `PostgreSQL`, `SQLite`, or `MySQL` database.
#[derive(diesel::MultiConnection)]
pub enum AnyConnection {
    /// A connection to a `PostgreSQL` database.
    ///
    /// This is used when the `database_url` is a `PostgreSQL` URL.
    ///
    /// # Example
    ///
    /// ```md
    /// postgres://user:password@localhost:5432/database
    /// ```
    #[cfg(feature = "postgres")]
    Postgresql(diesel::PgConnection),
    /// A connection to a `MySQL` database.
    ///
    /// This is used when the `database_url` is a `MySQL` URL.
    ///
    /// # Example
    ///
    /// ```md
    /// mysql://user:password@localhost:3306/database
    /// ```
    #[cfg(feature = "mysql")]
    Mysql(diesel::MysqlConnection),
    /// A connection to a `SQLite` database.
    ///
    /// This is used when the `database_url` is a `SQLite` URL.
    ///
    /// Note: The directory where the database is stored will also be populated with a WAL file and a journal file.
    ///
    /// # Example
    ///
    /// ```bash
    /// # Relative path
    /// sqlite://database.db
    /// sqlite://data/database.db
    ///
    /// # Absolute path
    /// sqlite:///data/database.db
    ///
    /// # In-memory database
    /// sqlite://:memory: # In-memory database
    /// ```
    #[cfg(feature = "sqlite")]
    Sqlite(diesel::SqliteConnection),
}

#[doc(hidden)]
#[derive(Debug, Default)]
pub struct AcquireHook {
    pub setup: bool,
}

impl diesel::r2d2::CustomizeConnection<AnyConnection, diesel::r2d2::Error> for AcquireHook {
    fn on_acquire(&self, conn: &mut AnyConnection) -> Result<(), diesel::r2d2::Error> {
        (|| {
            match conn {
                #[cfg(feature = "sqlite")]
                AnyConnection::Sqlite(ref mut conn) => {
                    conn.batch_execute("PRAGMA busy_timeout = 5000")?;
                    conn.batch_execute("PRAGMA synchronous = NORMAL;")?;
                    conn.batch_execute("PRAGMA wal_autocheckpoint = 1000;")?;
                    conn.batch_execute("PRAGMA foreign_keys = ON;")?;

                    if self.setup {
                        conn.batch_execute("PRAGMA journal_mode = WAL;")?;
                        conn.batch_execute("VACUUM")?;
                    }
                }
                #[cfg(feature = "postgres")]
                AnyConnection::Postgresql(ref mut conn) => {
                    if self.setup {
                        conn.batch_execute("VACUUM ANALYZE")?;
                    }
                }
                #[cfg(feature = "mysql")]
                AnyConnection::Mysql(ref mut conn) => {
                    conn.batch_execute("SET SESSION wait_timeout = 28800")?;
                    conn.batch_execute("SET SESSION interactive_timeout = 28800")?;
                }
            }
            Ok(())
        })()
        .map_err(diesel::r2d2::Error::QueryError)
    }
}

impl AnyConnection {
    pub fn pre_init(database_url: &str) -> anyhow::Result<()> {
        if database_url.starts_with("sqlite://") && !database_url.contains(":memory:") {
            let path = database_url
                .strip_prefix("sqlite://")
                .expect("already checked prefix");

            let path = PathBuf::from(path);

            let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
                return Ok(());
            };

            // Create directory if it doesn't exist
            if !parent.exists() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create database directory: {}", parent.display())
                })?;
            }
        }

        Ok(())
    }

    pub fn migrate(&mut self) -> anyhow::Result<()> {
        let migrations_applied = match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => conn.run_pending_migrations(POSTGRES_MIGRATIONS),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => conn.run_pending_migrations(MYSQL_MIGRATIONS),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => conn.run_pending_migrations(SQLITE_MIGRATIONS),
        }
        .expect("Could not run migrations");

        if !migrations_applied.is_empty() {
            info!(
                "Applied {} migration{}",
                migrations_applied.len(),
                sify(&migrations_applied)
            );
        }

        Ok(())
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(_) => {}
            #[cfg(feature = "mysql")]
            Self::Mysql(_) => {}
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => {
                // Should cleanup spare wal/shm files
                conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE);")
                    .context("failed to checkpoint WAL")?;
            }
        }

        Ok(())
    }

    /// Save changes to an existing scan event.
    ///
    /// For MySQL, this performs an UPDATE followed by a SELECT since MySQL
    /// does not support RETURNING clauses.
    pub fn save_changes(&mut self, ev: &mut ScanEvent) -> anyhow::Result<ScanEvent> {
        let ev = match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => ev.save_changes::<ScanEvent>(conn),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => ev.save_changes::<ScanEvent>(conn),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                use crate::schema::scan_events::dsl::scan_events;
                diesel::update(scan_events.find(&ev.id))
                    .set(&*ev)
                    .execute(conn)?;
                scan_events.find(&ev.id).first::<ScanEvent>(conn)
            }
        }?;

        Ok(ev)
    }

    /// Insert a new scan event and return the inserted row.
    ///
    /// For MySQL, this performs an INSERT followed by a SELECT since MySQL
    /// does not support RETURNING clauses.
    pub fn insert_and_return(&mut self, ev: &NewScanEvent) -> anyhow::Result<ScanEvent> {
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => diesel::insert_into(crate::schema::scan_events::table)
                .values(ev)
                .returning(ScanEvent::as_returning())
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => diesel::insert_into(crate::schema::scan_events::table)
                .values(ev)
                .returning(ScanEvent::as_returning())
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                use crate::schema::scan_events::dsl::scan_events;
                diesel::insert_into(crate::schema::scan_events::table)
                    .values(ev)
                    .execute(conn)?;
                scan_events
                    .find(&ev.id)
                    .first::<ScanEvent>(conn)
                    .map_err(Into::into)
            }
        }
    }

    /// Update a scan event's `updated_at` and `can_process` fields, returning the updated row.
    ///
    /// This is extracted from `PulseManager::add_event` to support MySQL which
    /// does not have RETURNING clause support.
    pub fn update_event_timestamps(
        &mut self,
        ev: &ScanEvent,
        new_updated_at: chrono::NaiveDateTime,
        new_can_process: chrono::NaiveDateTime,
    ) -> anyhow::Result<ScanEvent> {
        use crate::schema::scan_events::dsl::*;
        use diesel::{ExpressionMethods, QueryDsl};

        match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => diesel::update(scan_events.find(&ev.id))
                .set((
                    updated_at.eq(new_updated_at),
                    can_process.eq(new_can_process),
                ))
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => diesel::update(scan_events.find(&ev.id))
                .set((
                    updated_at.eq(new_updated_at),
                    can_process.eq(new_can_process),
                ))
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into),
            #[cfg(feature = "mysql")]
            Self::Mysql(conn) => {
                diesel::update(scan_events.find(&ev.id))
                    .set((
                        updated_at.eq(new_updated_at),
                        can_process.eq(new_can_process),
                    ))
                    .execute(conn)?;
                scan_events
                    .find(&ev.id)
                    .first::<ScanEvent>(conn)
                    .map_err(Into::into)
            }
        }
    }
}

#[doc(hidden)]
pub type DbPool = Pool<ConnectionManager<AnyConnection>>;

#[doc(hidden)]
pub fn get_conn(
    pool: &Pool<ConnectionManager<AnyConnection>>,
) -> anyhow::Result<PooledConnection<ConnectionManager<AnyConnection>>> {
    pool.get().context("failed to get connection from pool")
}

pub fn close_pool(pool: &Pool<ConnectionManager<AnyConnection>>) {
    if let Ok(mut conn) = pool.get() {
        let _ = conn.close();
    }
}

#[doc(hidden)]
pub fn get_pool(database_url: &String) -> anyhow::Result<Pool<ConnectionManager<AnyConnection>>> {
    let manager = ConnectionManager::<AnyConnection>::new(database_url);

    let pool = Pool::builder()
        .max_size(1)
        .connection_customizer(Box::new(AcquireHook { setup: true }))
        .build(manager)
        .context("failed to create pool");

    drop(pool);

    let manager = ConnectionManager::<AnyConnection>::new(database_url);

    Pool::builder()
        .connection_customizer(Box::new(AcquireHook::default()))
        .build(manager)
        .context("failed to create pool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_pre_init_memory_db_skipped() {
        let result = AnyConnection::pre_init("sqlite://:memory:");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pre_init_creates_directory() {
        let tmp = tempdir().unwrap();
        let db_path = tmp.path().join("subdir").join("test.db");
        let url = format!("sqlite://{}", db_path.display());

        let result = AnyConnection::pre_init(&url);
        assert!(result.is_ok());
        assert!(db_path.parent().unwrap().exists());
    }

    #[test]
    fn test_pre_init_no_parent_directory() {
        let result = AnyConnection::pre_init("sqlite://test.db");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pre_init_writable_directory_succeeds() {
        let tmp = tempdir().unwrap();
        let subdir = tmp.path().join("writable");
        fs::create_dir(&subdir).unwrap();

        let db_path = subdir.join("test.db");
        let url = format!("sqlite://{}", db_path.display());

        let result = AnyConnection::pre_init(&url);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pre_init_postgres_skipped() {
        let result = AnyConnection::pre_init("postgres://localhost/test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_pre_init_mysql_skipped() {
        let result = AnyConnection::pre_init("mysql://localhost/test");
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_close_pool_cleans_up_wal_files() {
        let tmp = tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let url = format!("sqlite://{}", db_path.display());

        AnyConnection::pre_init(&url).unwrap();
        let pool = get_pool(&url).unwrap();

        // Get a connection to trigger WAL mode and create the db
        {
            let mut conn = get_conn(&pool).unwrap();
            conn.migrate().unwrap();
        }

        // WAL files may exist at this point
        close_pool(&pool);
        drop(pool);

        // Verify no WAL files remain
        let wal_path = tmp.path().join("test.db-wal");
        let shm_path = tmp.path().join("test.db-shm");
        assert!(!wal_path.exists(), "WAL file should be cleaned up");
        assert!(!shm_path.exists(), "SHM file should be cleaned up");
    }
}
