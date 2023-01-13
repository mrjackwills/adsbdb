use std::{fmt, num::ParseIntError};

use axum::response::{IntoResponse, Response};
use redis::RedisError;
use thiserror::Error;
use tracing::error;

use super::response::ResponseJson;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnknownAC {
    Aircraft,
    Callsign,
}

impl fmt::Display for UnknownAC {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let disp = match self {
            Self::Aircraft => "aircraft",
            Self::Callsign => "callsign",
        };
        write!(f, "{disp}")
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid modeS or registration:")]
    AircraftSearch(String),
    #[error("Axum")]
    AxumExtension(#[from] axum::extract::rejection::ExtensionRejection),
    #[error("invalid callsign:")]
    Callsign(String),
    #[error("invalid n_number:")]
    NNumber(String),
    #[error("internal error:")]
    Internal(String),
    #[error("invalid modeS:")]
    ModeS(String),
    #[error("parse int")]
    ParseInt(#[from] ParseIntError),
    #[error("rate limited for")]
    RateLimited(usize),
    #[error("redis error")]
    RedisError(#[from] RedisError),
    #[error("invalid registration:")]
    Registration(String),
    #[error("Reqwest")]
    Reqwest(#[from] reqwest::Error),
    #[error("internal error")]
    SerdeJson(#[from] serde_json::Error),
    #[error("not found")]
    SqlxError(#[from] sqlx::Error),
    #[error("unknown")]
    UnknownInDb(UnknownAC),
}

impl IntoResponse for AppError {
    #[allow(clippy::cognitive_complexity)]
    fn into_response(self) -> Response {
        let prefix = self.to_string();
        let (status, body) = match self {
            Self::AxumExtension(e) => {
                error!("{:?}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(prefix),
                )
            }
            Self::Callsign(err)
            | Self::AircraftSearch(err)
            | Self::ModeS(err)
            | Self::NNumber(err)
            | Self::Registration(err) => (
                axum::http::StatusCode::BAD_REQUEST,
                ResponseJson::new(format!("{prefix} {err}")),
            ),

            Self::Internal(e) => {
                error!("internal: {:?}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(format!("{prefix} {e}")),
                )
            }
            Self::ParseInt(e) => {
                error!("parseint: {:?}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(prefix),
                )
            }
            Self::RateLimited(limit) => (
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                ResponseJson::new(format!("{prefix} {limit} seconds")),
            ),
            Self::RedisError(e) => {
                error!("{:?}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(prefix),
                )
            }
            Self::Reqwest(e) => {
                error!("{:?}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(prefix),
                )
            }
            Self::SerdeJson(e) => {
                error!("serde: {:?}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(prefix),
                )
            }
            Self::SqlxError(e) => {
                error!("{:?}", e);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(prefix),
                )
            }

            Self::UnknownInDb(variety) => (
                axum::http::StatusCode::NOT_FOUND,
                ResponseJson::new(format!("{prefix} {variety}")),
            ),
        };

        (status, body).into_response()
    }
}
