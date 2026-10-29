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
CREATE TABLE IF NOT EXISTS cybion_accounts (
  user_id         TEXT PRIMARY KEY,
  api_key_cipher  TEXT NOT NULL,
  key_hint        TEXT NOT NULL,
  key_id          TEXT,
  created_at      INTEGER NOT NULL,
  last_checked_at INTEGER,
  last_error      TEXT
);

CREATE TABLE IF NOT EXISTS projects (
  id                     TEXT PRIMARY KEY,
  owner_user_id          TEXT NOT NULL,
  name                   TEXT NOT NULL,
  description            TEXT NOT NULL DEFAULT '',
  -- New projects start private; `public` is an explicit owner decision.
  visibility             TEXT NOT NULL DEFAULT 'private' CHECK(visibility IN ('public','private')),
  created_at             INTEGER NOT NULL,
  updated_at             INTEGER NOT NULL,
  ticket_counter         INTEGER NOT NULL DEFAULT 0,
  resolver_key_hash      TEXT,
  resolver_key_hint      TEXT,
  resolver_key_created_at INTEGER,
  resolver_thread_url    TEXT,
  resolver_machine_id    TEXT
);

-- Who may see a private project besides its owner. A public project ignores
-- these rows, but keeps them so switching back to private does not lose them.
CREATE TABLE IF NOT EXISTS project_grants (
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  user_id    TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(project_id, user_id)
);

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
  last_reporter_at            INTEGER NOT NULL DEFAULT 0,
  last_resolver_at            INTEGER NOT NULL DEFAULT 0,
  reporter_thread_id          TEXT,
  reporter_thread_owner       TEXT,
  reporter_agent_token_hash   TEXT,
  reporter_agent_token_hint   TEXT,
  reporter_thread_error       TEXT,
  UNIQUE(project_id, number)
);

CREATE TABLE IF NOT EXISTS comments (
  id             TEXT PRIMARY KEY,
  ticket_id      TEXT NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
  kind           TEXT NOT NULL,
  author_kind    TEXT NOT NULL,
  author_user_id TEXT,
  author_label   TEXT,
  body           TEXT NOT NULL,
  created_at     INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_comments_ticket   ON comments(ticket_id, created_at);
CREATE INDEX IF NOT EXISTS idx_project_grants_user ON project_grants(user_id);
CREATE INDEX IF NOT EXISTS idx_tickets_project   ON tickets(project_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_tickets_reporter  ON tickets(reporter_user_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_tickets_resolver  ON tickets(project_id, status, last_resolver_at, last_reporter_at);
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
    // Columns added after the first release: `CREATE TABLE IF NOT EXISTS` leaves
    // an existing database untouched, so each one is added explicitly.
    ensure_column(&pool, "cybion_accounts", "key_id", "TEXT").await?;
    ensure_column(&pool, "projects", "resolver_machine_id", "TEXT").await?;
    // Projects used to be readable by every signed-in user, so existing rows are
    // migrated as public. Fresh databases get the `private` default above; the
    // application always binds the value explicitly on insert.
    ensure_column(
        &pool,
        "projects",
        "visibility",
        "TEXT NOT NULL DEFAULT 'public'",
    )
    .await?;
    Ok(pool)
}

async fn ensure_column(
    pool: &sqlx::SqlitePool,
    table: &str,
    column: &str,
    definition: &str,
) -> anyhow::Result<()> {
    let exists: bool = sqlx::query_scalar(&format!(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name=?)"
    ))
    .bind(column)
    .fetch_one(pool)
    .await?;
    if !exists {
        sqlx::raw_sql(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))
        .execute(pool)
        .await?;
    }
    Ok(())
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
