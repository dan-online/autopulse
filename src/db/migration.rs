use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use super::conn::AnyConnection;

#[doc(hidden)]
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[doc(hidden)]
pub fn run_db_migrations(conn: &mut PooledConnection<ConnectionManager<AnyConnection>>) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Could not run migrations");
}
