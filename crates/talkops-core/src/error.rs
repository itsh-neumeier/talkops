//! Error type for domain operations.

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid input: {0}")]
    Validation(String),
    #[error(transparent)]
    Crypto(#[from] crate::crypto::CryptoError),
    #[error("database error: {0}")]
    Db(sqlx::Error),
}

impl From<sqlx::Error> for CoreError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => CoreError::NotFound,
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                CoreError::Conflict(db.constraint().unwrap_or("unique constraint").to_owned())
            }
            sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
                CoreError::Validation(format!(
                    "referenced entity does not exist ({})",
                    db.constraint().unwrap_or("foreign key")
                ))
            }
            sqlx::Error::Database(db) if db.is_check_violation() => CoreError::Validation(format!(
                "value rejected by {}",
                db.constraint().unwrap_or("check constraint")
            )),
            _ => CoreError::Db(err),
        }
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
