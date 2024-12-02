use crate::db::models::{NewScanEvent, ScanEvent};
use crate::utils::sify::sify;
use anyhow::Context;
use diesel::connection::SimpleConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::{Connection, ConnectionError, QueryResult, RunQueryDsl};
use diesel::{SaveChangesDsl, SelectableHelper};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tracing::info;

#[doc(hidden)]
#[cfg(feature = "postgres")]
const POSTGRES_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/postgres");

#[doc(hidden)]
#[cfg(feature = "sqlite")]
const SQLITE_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/sqlite");

/// Represents a connection to either a PostgreSQL or SQLite database.
#[derive(diesel::MultiConnection)]
pub enum AnyConnection {
    /// A connection to a PostgreSQL database.
    ///
    /// This is used when the `database_url` is a PostgreSQL URL.
    ///
    /// # Example
    ///
    /// ```
    /// postgres://user:password@localhost:5432/database
    /// ```
    #[cfg(feature = "postgres")]
    Postgresql(diesel::PgConnection),
    // Mysql(diesel::MysqlConnection),
    /// A connection to a SQLite database.
    ///
    /// This is used when the `database_url` is a SQLite URL.
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

impl AnyConnection {
    pub fn pre_init(database_url: &str) -> anyhow::Result<()> {
        if database_url.starts_with("sqlite://") && !database_url.contains(":memory:") {
            let path = database_url.split("sqlite://").collect::<Vec<&str>>()[1];
            let path = PathBuf::from(path);
            let parent = path.parent().unwrap();

            if !std::path::Path::new(&path).exists() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create database directory: {}", parent.display())
                })?;
            }

            if path.file_name().map(|x| x.to_str()) != Some(path.to_str()) {
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o777))
                    .with_context(|| {
                        format!(
                            "Failed to set permissions on database directory: {}",
                            parent.display()
                        )
                    })?;
            }
        }

        Ok(())
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        #[cfg(feature = "sqlite")]
        if let Self::Sqlite(conn) = self {
            conn.batch_execute("
                PRAGMA journal_mode = WAL;          -- better write-concurrency
                PRAGMA synchronous = NORMAL;        -- fsync only in critical moments
                PRAGMA wal_autocheckpoint = 1000;   -- write WAL changes back every 1000 pages, for an in average 1MB WAL file. May affect readers if number is increased
                PRAGMA wal_checkpoint(TRUNCATE);    -- free some space by truncating possibly massive WAL files from the last run.
                PRAGMA busy_timeout = 30000;        -- sleep if the database is busy
                PRAGMA foreign_keys = ON;           -- enforce foreign keys
            ").map_err(ConnectionError::CouldntSetupConfiguration)?;
        }

        Ok(())
    }

    pub fn migrate(&mut self) -> anyhow::Result<()> {
        let migrations_applied = match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => conn.run_pending_migrations(POSTGRES_MIGRATIONS),
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

    pub fn save_changes(&mut self, ev: &mut ScanEvent) -> anyhow::Result<ScanEvent> {
        let ev = match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => ev.save_changes::<ScanEvent>(conn),
            // #[cfg(feature = "mysql")]
            // AnyConnection::Mysql(conn) => ev.save_changes::<ScanEvent>(conn),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => ev.save_changes::<ScanEvent>(conn),
        }?;

        Ok(ev)
    }

    pub fn insert_and_return(&mut self, ev: &NewScanEvent) -> anyhow::Result<ScanEvent> {
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => diesel::insert_into(crate::db::schema::scan_events::table)
                .values(ev)
                .returning(ScanEvent::as_returning())
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => diesel::insert_into(crate::db::schema::scan_events::table)
                .values(ev)
                .returning(ScanEvent::as_returning())
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into),
        }
    }
}

#[doc(hidden)]
pub type DbPool = Pool<ConnectionManager<AnyConnection>>;

#[doc(hidden)]
pub fn get_conn(
    pool: &Pool<ConnectionManager<AnyConnection>>,
) -> PooledConnection<ConnectionManager<AnyConnection>> {
    pool.get()
        .expect("Failed to get database connection from pool")
}

#[doc(hidden)]
pub fn get_pool(database_url: String) -> anyhow::Result<Pool<ConnectionManager<AnyConnection>>> {
    let manager = ConnectionManager::<AnyConnection>::new(database_url);

    Pool::builder()
        .build(manager)
        .with_context(|| "Failed to connect to database")
}
