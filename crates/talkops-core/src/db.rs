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

/// Hold this connection for the lifetime of a writer or an offline restore.
/// A dedicated connection avoids exhausting small application pools.
pub async fn operation_guard(
    database_url: &str,
    restore: bool,
) -> Result<sqlx::PgConnection, sqlx::Error> {
    use sqlx::Connection;
    let mut connection = sqlx::PgConnection::connect(database_url).await?;
    let query = if restore {
        "SELECT pg_try_advisory_lock(7231002)"
    } else {
        "SELECT pg_try_advisory_lock_shared(7231002)"
    };
    let locked: bool = sqlx::query_scalar(query).fetch_one(&mut connection).await?;
    if !locked {
        return Err(sqlx::Error::Protocol(
            "restore conflicts with an active TalkOps writer".into(),
        ));
    }
    Ok(connection)
}

/// Cheap round-trip used by readiness probes.
pub async fn ping(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Connection;
    #[sqlx::test]
    async fn restore_lock_excludes_writers_and_other_restores(pool: PgPool) {
        let base = std::env::var("DATABASE_URL").unwrap();
        // Connect with the test pool's options so the lock belongs to this test database.
        let mut writer = sqlx::PgConnection::connect_with(&pool.connect_options())
            .await
            .unwrap();
        let shared: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock_shared(7231002)")
            .fetch_one(&mut writer)
            .await
            .unwrap();
        assert!(shared);
        let mut url = url_for_test(&base, pool.connect_options().get_database().unwrap());
        assert!(operation_guard(&url, true).await.is_err());
        writer.close().await.unwrap();
        let guard = operation_guard(&url, true).await.unwrap();
        assert!(operation_guard(&url, false).await.is_err());
        assert!(operation_guard(&url, true).await.is_err());
        guard.close().await.unwrap();
        let guard = operation_guard(&url, false).await.unwrap();
        guard.close().await.unwrap();
        url.clear();
    }
    fn url_for_test(base: &str, name: &str) -> String {
        let Some((prefix, _)) = base.rsplit_once('/') else {
            panic!("test URL");
        };
        format!("{prefix}/{name}")
    }
}
