use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
  id           TEXT PRIMARY KEY,
  display_name TEXT,
  created_at   INTEGER NOT NULL
);

-- One Cybion API key per user. The key is sealed with the deployment's local
-- secret (see crypto::SecretBox) so a copy of the database alone reveals none.
-- Both sides of a Ticket use one: reporters to run the investigation, owners to
-- push a resolver Thread.
CREATE TABLE IF NOT EXISTS cybion_accounts (
  user_id         TEXT PRIMARY KEY,
  api_key_cipher  TEXT NOT NULL,
  key_hint        TEXT NOT NULL,
  key_id          TEXT,
  created_at      INTEGER NOT NULL,
  last_checked_at INTEGER,
  last_error      TEXT
);

-- A project carries no resolver configuration at all: no Worker, no key, no
-- schedule. Being submitted is enough — the Ticket is pushed to the owner's own
-- Cybion account, which picks its machine like any other Thread.
CREATE TABLE IF NOT EXISTS projects (
  id                     TEXT PRIMARY KEY,
  owner_user_id          TEXT NOT NULL,
  name                   TEXT NOT NULL,
  description            TEXT NOT NULL DEFAULT '',
  -- New projects start private; `public` is an explicit owner decision.
  visibility             TEXT NOT NULL DEFAULT 'private' CHECK(visibility IN ('public','private')),
  created_at             INTEGER NOT NULL,
  updated_at             INTEGER NOT NULL,
  ticket_counter         INTEGER NOT NULL DEFAULT 0
);

-- Who may see a private project besides its owner. A public project ignores
-- these rows, but keeps them so switching back to private does not lose them.
CREATE TABLE IF NOT EXISTS project_grants (
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  user_id    TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(project_id, user_id)
);

-- A Ticket carries exactly three pieces of text: what the reporter saw
-- (`body`), what the reporter Agent measured (`evidence`), and what the
-- resolver Agent did (`resolution`). There is no comment timeline.
CREATE TABLE IF NOT EXISTS tickets (
  id                          TEXT PRIMARY KEY,
  project_id                  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  number                      INTEGER NOT NULL,
  reporter_user_id            TEXT NOT NULL,
  title                       TEXT NOT NULL,
  body                        TEXT NOT NULL,
  status                      TEXT NOT NULL CHECK(status IN ('draft','open','closed')),
  created_at                  INTEGER NOT NULL,
  updated_at                  INTEGER NOT NULL,
  submitted_at                INTEGER,
  closed_at                   INTEGER,
  closed_by_kind              TEXT,
  closed_by_user_id           TEXT,
  evidence                    TEXT,
  evidence_at                 INTEGER,
  resolution                  TEXT,
  resolution_at               INTEGER,
  reporter_thread_id          TEXT,
  reporter_thread_owner       TEXT,
  reporter_agent_token_hash   TEXT,
  reporter_agent_token_hint   TEXT,
  reporter_thread_error       TEXT,
  resolver_thread_id          TEXT,
  resolver_thread_owner       TEXT,
  resolver_agent_token_hash   TEXT,
  resolver_agent_token_hint   TEXT,
  resolver_thread_error       TEXT,
  resolver_dispatched_at      INTEGER,
  UNIQUE(project_id, number)
);

CREATE INDEX IF NOT EXISTS idx_project_grants_user ON project_grants(user_id);
CREATE INDEX IF NOT EXISTS idx_tickets_project   ON tickets(project_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_tickets_reporter  ON tickets(reporter_user_id, updated_at DESC);
"#;

pub async fn connect(database_path: &Path) -> anyhow::Result<sqlx::SqlitePool> {
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite://{}", database_path.to_string_lossy()))?
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(10));

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;

    sqlx::raw_sql(SCHEMA).execute(&pool).await?;
    migrate(&pool).await?;
    Ok(pool)
}

/// Brings a database created by an older build up to the current model.
///
/// `CREATE TABLE IF NOT EXISTS` leaves an existing database untouched, so new
/// columns are added one by one and the columns of the removed polling
/// protocol — queue watermarks, the project-scoped resolver key, the Ultimate
/// Machine link, the comment timeline — are dropped. Dropping them is
/// intentional: their contents describe a design the code no longer has.
async fn migrate(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    for (table, column, definition) in [
        ("tickets", "evidence", "TEXT"),
        ("tickets", "evidence_at", "INTEGER"),
        ("tickets", "resolution", "TEXT"),
        ("tickets", "resolution_at", "INTEGER"),
        ("tickets", "resolver_thread_id", "TEXT"),
        ("tickets", "resolver_thread_owner", "TEXT"),
        ("tickets", "resolver_agent_token_hash", "TEXT"),
        ("tickets", "resolver_agent_token_hint", "TEXT"),
        ("tickets", "resolver_thread_error", "TEXT"),
        ("tickets", "resolver_dispatched_at", "INTEGER"),
    ] {
        ensure_column(pool, table, column, definition).await?;
    }

    // `visibility` arrived after the first release: existing projects were
    // readable by everyone, so they migrate as public while fresh rows take the
    // `private` default above.
    ensure_column(
        pool,
        "projects",
        "visibility",
        "TEXT NOT NULL DEFAULT 'public'",
    )
    .await?;

    sqlx::raw_sql("DROP INDEX IF EXISTS idx_tickets_resolver")
        .execute(pool)
        .await?;
    sqlx::raw_sql("DROP TABLE IF EXISTS comments")
        .execute(pool)
        .await?;
    for (table, column) in [
        ("tickets", "last_reporter_at"),
        ("tickets", "last_resolver_at"),
        ("projects", "resolver_key_hash"),
        ("projects", "resolver_key_hint"),
        ("projects", "resolver_key_created_at"),
        ("projects", "resolver_thread_url"),
        ("projects", "resolver_machine_id"),
    ] {
        drop_column(pool, table, column).await?;
    }
    Ok(())
}

async fn ensure_column(
    pool: &sqlx::SqlitePool,
    table: &str,
    column: &str,
    definition: &str,
) -> anyhow::Result<()> {
    if column_exists(pool, table, column).await? {
        return Ok(());
    }
    sqlx::raw_sql(&format!(
        "ALTER TABLE {table} ADD COLUMN {column} {definition}"
    ))
    .execute(pool)
    .await?;
    Ok(())
}

async fn drop_column(pool: &sqlx::SqlitePool, table: &str, column: &str) -> anyhow::Result<()> {
    if !column_exists(pool, table, column).await? {
        return Ok(());
    }
    sqlx::raw_sql(&format!("ALTER TABLE {table} DROP COLUMN {column}"))
        .execute(pool)
        .await?;
    Ok(())
}

async fn column_exists(pool: &sqlx::SqlitePool, table: &str, column: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar(&format!(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name=?)"
    ))
    .bind(column)
    .fetch_one(pool)
    .await?)
}

/// Reads one deployment setting, falling back to the compiled default.
pub async fn setting(pool: &sqlx::SqlitePool, key: &str, default: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key=?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| default.to_owned())
}

pub async fn put_setting(pool: &sqlx::SqlitePool, key: &str, value: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}
