//! Database pool creation and schema migrations.

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

/// Embedded migrations from the repository's `migrations/` directory.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database connection failed: {0}")]
    Connect(#[source] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

/// Opens a connection pool. The pool connects lazily so that services can start
/// (and report "not ready") while Postgres is still coming up.
pub fn connect_lazy(database_url: &str, max_connections: u32) -> Result<PgPool, DbError> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect_lazy(database_url)
        .map_err(DbError::Connect)
}

/// Applies all pending migrations.
pub async fn migrate(pool: &PgPool) -> Result<(), DbError> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

/// Cheap round-trip used by readiness probes.
pub async fn ping(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await.map(|_| ())
}
