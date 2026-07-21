use serde::Serialize;

/// App-wide error. Serializes to a plain string so it surfaces cleanly in the
/// frontend `invoke(...).catch()`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("erro de rede: {0}")]
    Http(#[from] reqwest::Error),
    #[error("erro de I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("erro de serialização: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Provider(String),
    #[error("{0}")]
    Msg(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
