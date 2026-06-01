use actix_web::cookie::Key;
use anyhow::bail;
use anyhow::Context;
use autopulse_database::{
    conn::{get_conn, DbPool},
    diesel::{
        self,
        result::{DatabaseErrorKind, Error as DieselError},
        ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl,
    },
    models::AppState,
    schema::app_state::{self, dsl::app_state as app_state_tbl},
};

const KEY_NAME: &str = "ui_session_key_v1";
const KEY_LEN: usize = 64;

/// `Key::from` panics on blobs shorter than 64 bytes; a corrupted or
/// hand-edited row would otherwise crash the server at startup.
fn key_from_row(row: &AppState) -> anyhow::Result<Key> {
    if row.value.len() < KEY_LEN {
        bail!(
            "stored session key '{KEY_NAME}' is {} bytes, expected >= {KEY_LEN}; \
             delete the row to regenerate",
            row.value.len()
        );
    }
    Ok(Key::from(&row.value))
}

/// Rotate: `DELETE FROM app_state WHERE key = 'ui_session_key_v1'`
/// then restart. All existing sessions are invalidated.
pub fn load_or_create(pool: &DbPool) -> anyhow::Result<Key> {
    if let Some(row) = app_state_tbl
        .find(KEY_NAME)
        .first::<AppState>(&mut get_conn(pool)?)
        .optional()
        .context("load existing session key")?
    {
        return key_from_row(&row);
    }

    let mut bytes = [0u8; KEY_LEN];
    getrandom::fill(&mut bytes).context("generate session key bytes")?;

    match diesel::insert_into(app_state::table)
        .values((app_state::key.eq(KEY_NAME), app_state::value.eq(&bytes[..])))
        .execute(&mut get_conn(pool)?)
    {
        Ok(_) => Ok(Key::from(&bytes)),
        // Another process raced us between the SELECT above and this INSERT
        // and won. Reload and use *their* key so both processes sign sessions
        // with the same material; using our locally-generated `bytes` here
        // would silently invalidate all sessions issued by the other replica.
        Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
            let row = app_state_tbl
                .find(KEY_NAME)
                .first::<AppState>(&mut get_conn(pool)?)
                .context("reload session key after concurrent insert")?;
            key_from_row(&row)
        }
        Err(e) => Err(e).context("persist new session key"),
    }
}
