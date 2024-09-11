use crate::db::models::{NewScanEvent, ScanEvent};
use anyhow::Context;
use diesel::connection::SimpleConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::{Connection, ConnectionError, QueryResult, RunQueryDsl};
use diesel::{SaveChangesDsl, SelectableHelper};

#[derive(diesel::MultiConnection)]
pub enum AnyConnection {
    Postgresql(diesel::PgConnection),
    // Mysql(diesel::MysqlConnection),
    Sqlite(diesel::SqliteConnection),
}

impl AnyConnection {
    pub fn init(&mut self) -> anyhow::Result<()> {
        match self {
            Self::Sqlite(conn) => {
                conn.batch_execute("
                    PRAGMA journal_mode = WAL;          -- better write-concurrency
                    PRAGMA synchronous = NORMAL;        -- fsync only in critical moments
                    PRAGMA wal_autocheckpoint = 1000;   -- write WAL changes back every 1000 pages, for an in average 1MB WAL file. May affect readers if number is increased
                    PRAGMA wal_checkpoint(TRUNCATE);    -- free some space by truncating possibly massive WAL files from the last run.
                    PRAGMA busy_timeout = 250;          -- sleep if the database is busy
                    PRAGMA foreign_keys = ON;           -- enforce foreign keys
                ").map_err(ConnectionError::CouldntSetupConfiguration)?;
            }
            _ => {}
        }

        Ok(())
    }

    pub fn save_changes(&mut self, ev: &mut ScanEvent) -> anyhow::Result<ScanEvent> {
        let ev = match self {
            Self::Postgresql(conn) => ev.save_changes::<ScanEvent>(conn),
            // AnyConnection::Mysql(conn) => ev.save_changes::<ScanEvent>(conn),
            Self::Sqlite(conn) => ev.save_changes::<ScanEvent>(conn),
        }?;

        Ok(ev)
    }

    pub fn insert_and_return(&mut self, ev: &NewScanEvent) -> anyhow::Result<ScanEvent> {
        match self {
            Self::Postgresql(conn) => diesel::insert_into(crate::db::schema::scan_events::table)
                .values(ev)
                .returning(ScanEvent::as_returning())
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into),
            Self::Sqlite(conn) => diesel::insert_into(crate::db::schema::scan_events::table)
                .values(ev)
                .returning(ScanEvent::as_returning())
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into),
        }
    }
}

pub type DbPool = Pool<ConnectionManager<AnyConnection>>;

pub fn get_conn(
    pool: &Pool<ConnectionManager<AnyConnection>>,
) -> PooledConnection<ConnectionManager<AnyConnection>> {
    pool.get()
        .expect("Failed to get database connection from pool")
}

pub fn get_pool(database_url: String) -> anyhow::Result<Pool<ConnectionManager<AnyConnection>>> {
    let manager = ConnectionManager::<AnyConnection>::new(database_url);

    Pool::builder()
        .build(manager)
        .with_context(|| "Failed to create connection pool")
}
