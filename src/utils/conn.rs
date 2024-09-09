use crate::db::models::{NewScanEvent, ScanEvent};
use anyhow::Context;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::{Connection, QueryResult, RunQueryDsl};
use diesel::{SaveChangesDsl, SelectableHelper};

#[derive(diesel::MultiConnection)]
pub enum AnyConnection {
    Postgresql(diesel::PgConnection),
    // Mysql(diesel::MysqlConnection),
    Sqlite(diesel::SqliteConnection),
}

impl AnyConnection {
    pub fn save_changes(&mut self, ev: &mut ScanEvent) {
        match self {
            Self::Postgresql(conn) => {
                ev.save_changes::<ScanEvent>(conn).unwrap();
            }
            // AnyConnection::Mysql(conn) => {
            //     ev.save_changes::<ScanEvent>(conn).unwrap();
            // }
            Self::Sqlite(conn) => {
                ev.save_changes::<ScanEvent>(conn).unwrap();
            }
        }
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
