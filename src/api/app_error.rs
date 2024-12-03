use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use fred::error::ErrorKind;
use std::{fmt, num::ParseIntError};
use thiserror::Error;
use tracing::error;

use super::response::ResponseJson;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnknownAC {
    Aircraft,
    Callsign,
    Airline,
}

impl fmt::Display for UnknownAC {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let disp = match self {
            Self::Aircraft => "aircraft",
            Self::Airline => "airline",
            Self::Callsign => "callsign",
        };
        write!(f, "{disp}")
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid modeS or registration:")]
    AircraftSearch(String),
    #[error("invalid airline:")]
    Airline(String),
    #[error("Axum")]
    AxumExtension(#[from] axum::extract::rejection::ExtensionRejection),
    #[error("invalid callsign:")]
    Callsign(String),
    #[error("internal error:")]
    Internal(String),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("invalid modeS:")]
    ModeS(String),
    #[error("invalid n_number:")]
    NNumber(String),
    #[error("parse int")]
    ParseInt(#[from] ParseIntError),
    #[error("rate limited for")]
    RateLimited(i64),
    #[error("redis error")]
    RedisError(#[from] fred::error::Error),
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

/// Return the internal server error, with a basic { response: "$prefix" }
macro_rules! internal {
    ($prefix:expr) => {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseJson::new($prefix),
        )
    };
}

impl IntoResponse for AppError {
    #[allow(clippy::cognitive_complexity)]
    fn into_response(self) -> Response {
        let exit = || {
            error!("EXITING");
            std::process::exit(1);
        };

        let prefix = self.to_string();
        let (status, body) = match self {
            Self::AxumExtension(e) => {
                error!("{e:?}");
                internal!(prefix)
            }
            Self::Callsign(err)
            | Self::AircraftSearch(err)
            | Self::Airline(err)
            | Self::ModeS(err)
            | Self::NNumber(err)
            | Self::Registration(err) => (
                StatusCode::BAD_REQUEST,
                ResponseJson::new(format!("{prefix} {err}")),
            ),

            Self::Internal(e) => {
                error!("internal: {e:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(format!("{prefix} {e}")),
                )
            }
            Self::Io(e) => {
                error!("io: {e:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson::new(format!("{prefix} {e}")),
                )
            }
            Self::ParseInt(e) => {
                error!("parseint: {e:?}");
                internal!(prefix)
            }
            Self::RateLimited(limit) => (
                StatusCode::TOO_MANY_REQUESTS,
                ResponseJson::new(format!("{prefix} {limit} seconds")),
            ),
            Self::RedisError(e) => {
                error!("{e:?}");
                if e.kind() == &ErrorKind::IO {
                    exit();
                }
                internal!(prefix)
            }
            Self::Reqwest(e) => {
                error!("{e:?}");
                internal!(prefix)
            }
            Self::SerdeJson(e) => {
                error!("serde: {e:?}");
                internal!(prefix)
            }
            Self::SqlxError(e) => {
                error!("{e:?}");
                match e {
                    sqlx::Error::Io(_) | sqlx::Error::PoolClosed | sqlx::Error::PoolTimedOut => {
                        exit();
                    }
                    _ => (),
                };
                internal!(prefix)
            }

            Self::UnknownInDb(variety) => (
                StatusCode::NOT_FOUND,
                ResponseJson::new(format!("{prefix} {variety}")),
            ),
        };

        (status, body).into_response()
    }
}
