use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
type DB = diesel::pg::Pg;

pub fn run_db_migrations(conn: &mut impl MigrationHarness<DB>) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Could not run migrations");
}
