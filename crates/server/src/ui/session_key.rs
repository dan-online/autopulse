use actix_web::cookie::Key;
use anyhow::bail;
use anyhow::Context;
use autopulse_database::{
    conn::{get_conn, DbPool},
    diesel::{self, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl},
    models::AppState,
    schema::app_state::{self, dsl::app_state as app_state_tbl},
};

const KEY_NAME: &str = "ui_session_key_v1";
const KEY_LEN: usize = 64;

/// Rotate: `DELETE FROM app_state WHERE key = 'ui_session_key_v1'`
/// then restart. All existing sessions are invalidated.
pub fn load_or_create(pool: &DbPool) -> anyhow::Result<Key> {
    if let Some(row) = app_state_tbl
        .find(KEY_NAME)
        .first::<AppState>(&mut get_conn(pool)?)
        .optional()
        .context("load existing session key")?
    {
        // `Key::from` panics on blobs shorter than 64 bytes; a corrupted
        // or hand-edited row would otherwise crash the server at startup.
        if row.value.len() < KEY_LEN {
            bail!(
                "stored session key '{KEY_NAME}' is {} bytes, expected >= {KEY_LEN}; \
                 delete the row to regenerate",
                row.value.len()
            );
        }
        return Ok(Key::from(&row.value));
    }

    let mut bytes = [0u8; KEY_LEN];
    getrandom::fill(&mut bytes).context("generate session key bytes")?;

    diesel::insert_into(app_state::table)
        .values((app_state::key.eq(KEY_NAME), app_state::value.eq(&bytes[..])))
        .execute(&mut get_conn(pool)?)
        .context("persist new session key")?;

    Ok(Key::from(&bytes))
}
