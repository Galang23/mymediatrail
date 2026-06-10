use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("utf8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("{0}")]
    Message(String),
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Message(s.to_string())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Message(s)
    }
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Message(s.into())
    }
}

pub type AppResult<T> = Result<T, AppError>;

macro_rules! bail {
    ($msg:expr) => {
        return Err($crate::error::AppError::msg($msg))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::error::AppError::msg(format!($fmt, $($arg)*)))
    };
}

pub(crate) use bail;
