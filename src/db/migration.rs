use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use crate::utils::conn::AnyConnection;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn run_db_migrations(conn: &mut PooledConnection<ConnectionManager<AnyConnection>>) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Could not run migrations");
}
