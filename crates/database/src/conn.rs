use crate::models::{NewScanEvent, ScanEvent};
use anyhow::Context;
use autopulse_utils::sify;
use diesel::connection::SimpleConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::SelectableHelper;
use diesel::{Connection, RunQueryDsl};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde::Deserialize;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

#[doc(hidden)]
#[cfg(feature = "postgres")]
const POSTGRES_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/postgres");

#[doc(hidden)]
#[cfg(feature = "sqlite")]
const SQLITE_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/sqlite");

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
}

impl DatabaseType {
    pub fn default_url(&self) -> String {
        match self {
            #[cfg(feature = "sqlite")]
            Self::Sqlite => "sqlite://data/autopulse.db".to_string(),
            #[cfg(feature = "postgres")]
            Self::Postgres => "postgres://autopulse:autopulse@localhost:5432/autopulse".to_string(),
        }
    }
}

/// Represents a connection to either a `PostgreSQL` or `SQLite` database.
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
    // Mysql(diesel::MysqlConnection),
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
                    }
                }
                #[cfg(feature = "postgres")]
                AnyConnection::Postgresql(_) => {}
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

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let probe = parent.join(format!(
                ".autopulse-db-write-test-{}-{timestamp}",
                std::process::id()
            ));

            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&probe)
                .with_context(|| {
                    format!("database directory is not writable: {}", parent.display())
                })?;
            drop(file);

            std::fs::remove_file(&probe).with_context(|| {
                format!(
                    "failed to remove database directory write test file: {}",
                    probe.display()
                )
            })?;
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
        // Preserve `e.source()` chain via anyhow::Error::from_boxed; the
        // previous `anyhow!("...{e}")` flattened it to Display only.
        .map_err(|e| anyhow::Error::from_boxed(e).context("failed to run migrations"))?;

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
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => {
                // Should cleanup spare wal/shm files
                conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE);")
                    .context("failed to checkpoint WAL")?;
            }
        }

        Ok(())
    }

    pub fn update_found(
        &mut self,
        previous: &ScanEvent,
        status: &str,
        at: chrono::NaiveDateTime,
    ) -> anyhow::Result<Option<ScanEvent>> {
        use crate::schema::scan_events::dsl::*;
        use diesel::prelude::*;
        use diesel::sql_types::Bool;

        let query = diesel::update(
            scan_events
                .find(&previous.id)
                .filter(process_status.eq("pending"))
                .filter(found_status.eq(&previous.found_status))
                .filter(
                    file_hash.eq(&previous.file_hash).or(file_hash
                        .is_null()
                        .and(previous.file_hash.is_none().into_sql::<Bool>())),
                ),
        )
        .set((
            found_status.eq(status),
            found_at.eq(Some(at)),
            updated_at.eq(at),
        ));
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => query.returning(ScanEvent::as_returning()).get_result(conn),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => query.returning(ScanEvent::as_returning()).get_result(conn),
        }
        .optional()
        .map_err(Into::into)
    }

    pub fn update_process(
        &mut self,
        previous: &ScanEvent,
        updated: &ScanEvent,
    ) -> anyhow::Result<Option<ScanEvent>> {
        use crate::schema::scan_events::dsl::*;
        use diesel::prelude::*;
        use diesel::sql_types::Bool;

        // Dedupe and file checks own other columns and must survive target awaits.
        let query = diesel::update(
            scan_events
                .find(&previous.id)
                .filter(process_status.eq(&previous.process_status))
                .filter(process_status.eq_any(["pending", "retry"]))
                .filter(failed_times.eq(previous.failed_times))
                .filter(targets_hit.eq(&previous.targets_hit))
                .filter(
                    next_retry_at.eq(previous.next_retry_at).or(next_retry_at
                        .is_null()
                        .and(previous.next_retry_at.is_none().into_sql::<Bool>())),
                ),
        )
        .set((
            process_status.eq(&updated.process_status),
            failed_times.eq(updated.failed_times),
            next_retry_at.eq(updated.next_retry_at),
            targets_hit.eq(&updated.targets_hit),
            processed_at.eq(updated.processed_at),
            updated_at.eq(updated.updated_at),
        ));
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => query.returning(ScanEvent::as_returning()).get_result(conn),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => query.returning(ScanEvent::as_returning()).get_result(conn),
        }
        .optional()
        .map_err(Into::into)
    }

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
        }
    }

    /// Inserts a queued event, or updates the existing pending/retry row for the path.
    pub fn upsert_pending(
        &mut self,
        ev: &NewScanEvent,
        now: chrono::NaiveDateTime,
    ) -> anyhow::Result<ScanEvent> {
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgresql(conn) => upsert_pending_pg(conn, ev, now),
            #[cfg(feature = "sqlite")]
            Self::Sqlite(conn) => upsert_pending_sqlite(conn, ev, now),
        }
    }
}

#[cfg(feature = "postgres")]
fn upsert_pending_pg(
    conn: &mut diesel::PgConnection,
    ev: &NewScanEvent,
    now: chrono::NaiveDateTime,
) -> anyhow::Result<ScanEvent> {
    use crate::models::ProcessStatus;
    use crate::schema::scan_events::dsl::{
        can_process, file_hash, file_path, process_status, updated_at,
    };
    use diesel::dsl::case_when;
    use diesel::upsert::{excluded, DecoratableTarget};
    use diesel::ExpressionMethods;

    // Keep this predicate aligned with the partial index; Postgres checks that
    // match at runtime, and the smoke test covers it.
    let pending: String = ProcessStatus::Pending.into();
    let retry: String = ProcessStatus::Retry.into();

    diesel::insert_into(crate::schema::scan_events::table)
        .values(ev)
        .on_conflict(file_path)
        .filter_target(process_status.eq_any([pending, retry]))
        .do_update()
        .set((
            updated_at.eq(now),
            can_process.eq(
                case_when(can_process.lt(excluded(can_process)), excluded(can_process))
                    .otherwise(can_process),
            ),
            file_hash.eq(case_when(file_hash.is_null(), excluded(file_hash)).otherwise(file_hash)),
        ))
        .returning(ScanEvent::as_returning())
        .get_result::<ScanEvent>(conn)
        .map_err(Into::into)
}

#[cfg(feature = "sqlite")]
fn upsert_pending_sqlite(
    conn: &mut diesel::SqliteConnection,
    ev: &NewScanEvent,
    now: chrono::NaiveDateTime,
) -> anyhow::Result<ScanEvent> {
    use crate::models::ProcessStatus;
    use crate::schema::scan_events::dsl::{
        can_process, file_hash, file_path, process_status, scan_events, updated_at,
    };
    use diesel::{ExpressionMethods, QueryDsl};
    use diesel::{OptionalExtension, SelectableHelper};

    // Acquire the write lock before reading, including across independent pools.
    conn.immediate_transaction(|conn| {
        let pending: String = ProcessStatus::Pending.into();
        let retry: String = ProcessStatus::Retry.into();

        let existing: Option<ScanEvent> = scan_events
            .filter(file_path.eq(&ev.file_path))
            .filter(process_status.eq_any([pending, retry]))
            .select(ScanEvent::as_select())
            .first::<ScanEvent>(conn)
            .optional()?;

        if let Some(existing) = existing {
            let later_can_process = std::cmp::max(existing.can_process, ev.can_process);
            let file_hash_value = existing.file_hash.clone().or_else(|| ev.file_hash.clone());
            diesel::update(&existing)
                .set((
                    updated_at.eq(now),
                    can_process.eq(later_can_process),
                    file_hash.eq(file_hash_value),
                ))
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into)
        } else {
            diesel::insert_into(crate::schema::scan_events::table)
                .values(ev)
                .returning(ScanEvent::as_returning())
                .get_result::<ScanEvent>(conn)
                .map_err(Into::into)
        }
    })
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
    match pool.get() {
        Ok(mut conn) => {
            if let Err(e) = conn.close() {
                warn!("failed to close database connection cleanly: {e}");
            }
        }
        Err(e) => {
            warn!("failed to get connection for pool shutdown: {e}");
        }
    }
}

#[doc(hidden)]
pub fn get_pool(database_url: &String) -> anyhow::Result<Pool<ConnectionManager<AnyConnection>>> {
    // First pool fires `AcquireHook { setup: true }` once (VACUUM/WAL), then dropped.
    let manager = ConnectionManager::<AnyConnection>::new(database_url);

    let setup_pool = Pool::builder()
        .max_size(1)
        .connection_customizer(Box::new(AcquireHook { setup: true }))
        .build(manager)
        .context("failed to create setup pool")?;

    drop(setup_pool);

    let manager = ConnectionManager::<AnyConnection>::new(database_url);

    let builder = Pool::builder().connection_customizer(Box::new(AcquireHook::default()));

    #[cfg(feature = "sqlite")]
    let builder = if database_url.starts_with("sqlite://") {
        builder.max_size(1)
    } else {
        builder
    };

    builder.build(manager).context("failed to create pool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    #[cfg(feature = "sqlite")]
    fn independent_sqlite_pools_coalesce_concurrent_arrivals() {
        use diesel::QueryDsl;
        use std::sync::{Arc, Barrier};
        let dir = tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("concurrent.db").display());
        let first = get_pool(&url).unwrap();
        get_conn(&first).unwrap().migrate().unwrap();
        let pools = (0..8).map(|_| get_pool(&url).unwrap()).collect::<Vec<_>>();
        let barrier = Arc::new(Barrier::new(pools.len()));
        let start = chrono::Utc::now().naive_utc();
        let handles = pools
            .into_iter()
            .enumerate()
            .map(|(index, pool)| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    get_conn(&pool)
                        .unwrap()
                        .upsert_pending(
                            &NewScanEvent {
                                can_process: start + chrono::Duration::seconds(index as i64),
                                file_hash: (index == 3).then(|| "hash".into()),
                                ..Default::default()
                            },
                            start,
                        )
                        .unwrap()
                        .id
                })
            })
            .collect::<Vec<_>>();
        let mut ids = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 1);
        let saved = crate::schema::scan_events::table
            .find(&ids[0])
            .first::<ScanEvent>(&mut get_conn(&first).unwrap())
            .unwrap();
        assert_eq!(saved.can_process, start + chrono::Duration::seconds(7));
        assert_eq!(saved.file_hash.as_deref(), Some("hash"));
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn found_update_preserves_dedupe_timer() {
        let mut conn = AnyConnection::establish(":memory:").unwrap();
        conn.migrate().unwrap();
        let event = NewScanEvent::default();
        let snapshot = conn.insert_and_return(&event).unwrap();
        let later = NewScanEvent {
            can_process: event.can_process + chrono::Duration::seconds(60),
            ..event
        };
        conn.upsert_pending(&later, chrono::Utc::now().naive_utc())
            .unwrap();
        let saved = conn
            .update_found(&snapshot, "found", chrono::Utc::now().naive_utc())
            .unwrap()
            .unwrap();
        assert_eq!(saved.can_process, later.can_process);
        assert_eq!(saved.found_status, "found");
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn terminal_update_clears_retry_timestamp() {
        use crate::schema::scan_events::dsl::*;
        use diesel::{ExpressionMethods, QueryDsl};

        let mut conn = AnyConnection::establish(":memory:").unwrap();
        conn.migrate().unwrap();
        let event = conn.insert_and_return(&NewScanEvent::default()).unwrap();
        diesel::update(scan_events.find(&event.id))
            .set((
                process_status.eq("retry"),
                next_retry_at.eq(Some(event.created_at)),
            ))
            .execute(&mut conn)
            .unwrap();
        let mut snapshot = scan_events
            .find(&event.id)
            .first::<ScanEvent>(&mut conn)
            .unwrap();
        let previous = snapshot.clone();
        snapshot.process_status = "failed".into();
        snapshot.next_retry_at = None;
        let saved = conn.update_process(&previous, &snapshot).unwrap().unwrap();
        assert_eq!(saved.process_status, "failed");
        assert_eq!(saved.next_retry_at, None);
    }

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

    #[cfg(unix)]
    #[test]
    fn test_pre_init_existing_unwritable_directory_fails_with_context() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempdir().unwrap();
        let subdir = tmp.path().join("readonly");
        fs::create_dir(&subdir).unwrap();
        fs::set_permissions(&subdir, fs::Permissions::from_mode(0o555)).unwrap();

        let db_path = subdir.join("test.db");
        let url = format!("sqlite://{}", db_path.display());

        let result = AnyConnection::pre_init(&url);

        fs::set_permissions(&subdir, fs::Permissions::from_mode(0o755)).unwrap();
        let err = result.expect_err("unwritable database directory should fail pre-init");
        let err = err.to_string();
        assert!(err.contains("database directory is not writable"));
        assert!(err.contains(&subdir.display().to_string()));
    }

    #[test]
    fn test_pre_init_postgres_skipped() {
        let result = AnyConnection::pre_init("postgres://localhost/test");
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

    #[test]
    #[cfg(feature = "sqlite")]
    fn dedupe_migration_merges_max_can_process_into_survivor() {
        use crate::models::ProcessStatus;
        use crate::schema::scan_events::dsl::{file_path, process_status, scan_events};
        use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
        use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

        let tmp = tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let url = format!("sqlite://{}", db_path.display());

        AnyConnection::pre_init(&url).unwrap();
        let pool = get_pool(&url).unwrap();
        let mut conn = get_conn(&pool).unwrap();

        conn.batch_execute(
            r#"
            CREATE TABLE scan_events (
                id TEXT PRIMARY KEY NOT NULL,
                event_source TEXT NOT NULL,
                event_timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
                file_path TEXT NOT NULL,
                file_hash TEXT,
                process_status TEXT NOT NULL DEFAULT 'pending',
                found_status TEXT NOT NULL DEFAULT 'not_found',
                failed_times INTEGER DEFAULT 0 NOT NULL,
                next_retry_at TIMESTAMP,
                targets_hit TEXT DEFAULT '' NOT NULL,
                found_at TIMESTAMP,
                processed_at TIMESTAMP,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
                can_process TIMESTAMP NOT NULL DEFAULT "2024-10-14T12:00:00.000"
            );

            CREATE TABLE __diesel_schema_migrations (
                version VARCHAR(50) PRIMARY KEY NOT NULL,
                run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            INSERT INTO __diesel_schema_migrations (version) VALUES
                ('20240829125750'),
                ('20240905143749'),
                ('20240906161345'),
                ('20241012130403'),
                ('20241205114327'),
                ('20241205115656'),
                ('202512300005460000'),
                ('20260519000001');

            INSERT INTO scan_events (
                id, event_source, file_path, file_hash, process_status,
                updated_at, created_at, event_timestamp, can_process
            ) VALUES
                (
                    'older-long-wait', 'sonarr', '/media/migrate.mkv', 'sha256:migrate', 'pending',
                    '2026-01-01 00:00:00', '2026-01-01 00:00:00',
                    '2026-01-01 00:00:00', '2026-01-01 03:00:00'
                ),
                (
                    'newer-short-wait', 'notify', '/media/migrate.mkv', NULL, 'retry',
                    '2026-01-01 01:00:00', '2026-01-01 01:00:00',
                    '2026-01-01 01:00:00', '2026-01-01 02:00:00'
                );
            "#,
        )
        .unwrap();

        conn.migrate().unwrap();

        let pending: String = ProcessStatus::Pending.into();
        let retry: String = ProcessStatus::Retry.into();
        let rows = scan_events
            .filter(file_path.eq("/media/migrate.mkv"))
            .filter(process_status.eq_any([pending, retry]))
            .load::<ScanEvent>(&mut conn)
            .unwrap();

        assert_eq!(rows.len(), 1, "migration should leave one non-terminal row");
        assert_eq!(rows[0].id, "newer-short-wait", "newest row should survive");
        assert_eq!(
            rows[0].can_process,
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
            ),
            "survivor should inherit the duplicate group's longest wait"
        );
        assert_eq!(
            rows[0].file_hash,
            Some("sha256:migrate".to_string()),
            "survivor should inherit a duplicate's hash when it has none"
        );
    }
}
