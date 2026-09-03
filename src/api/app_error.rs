use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use fred::error::ErrorKind;
use std::{fmt, num::ParseIntError};
use thiserror::Error;

use crate::S;

use super::response::ResponseJson;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnknownAC {
    Aircraft,
    Airline,
    Airport(String),
    Callsign,
    ModeS,
}

impl fmt::Display for UnknownAC {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Aircraft => write!(f, "aircraft"),
            Self::Airline => write!(f, "airline"),
            Self::Airport(icao) => write!(f, "airport: {icao}"),
            Self::Callsign => write!(f, "callsign"),
            Self::ModeS => write!(f, "mode-s conversion"),
        }
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid modeS or registration:")]
    AircraftSearch(String),
    #[error("invalid airline:")]
    Airline(String),
    #[error("invalid authorization")]
    Authorization,
    #[error("Axum")]
    AxumExtension(#[from] axum::extract::rejection::ExtensionRejection),
    #[error("invalid body")]
    Body(String),
    #[error("invalid callsign:")]
    Callsign(String),
    #[error("internal error:")]
    Internal(String),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("invalid mode-s:")]
    ModeS(String),
    #[error("invalid n-number:")]
    NNumber(String),
    #[error("parse int")]
    ParseInt(#[from] ParseIntError),
    #[error("rate limited for")]
    RateLimited(i64),
    #[error(transparent)]
    RedisError(#[from] fred::error::Error),
    #[error("invalid registration:")]
    Registration(String),
    #[error("Reqwest")]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    SqlxError(#[from] sqlx::Error),
    #[error("unknown")]
    UnknownInDb(UnknownAC),
}

/// Return the internal server error, with a basic { response: "$prefix" }
macro_rules! internal {
    () => {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseJson::new(S!("internal error")),
        )
    };
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
            tracing::error!("EXITING");
            std::process::exit(1);
        };

        let prefix = self.to_string();
        let (status, body) = match self {
            Self::AxumExtension(e) => {
                tracing::error!("{e:?}");
                internal!()
            }
            Self::Authorization => (
                StatusCode::UNAUTHORIZED,
                ResponseJson::new(S!("Invalid Authorization")),
            ),
            Self::Callsign(err)
            | Self::AircraftSearch(err)
            | Self::Airline(err)
            | Self::ModeS(err)
            | Self::NNumber(err)
            | Self::Body(err)
            | Self::Registration(err) => (
                StatusCode::BAD_REQUEST,
                ResponseJson::new(format!("{prefix} {err}")),
            ),

            Self::Internal(e) => {
                tracing::error!("internal: {e:?}");
                internal!()
            }
            Self::Io(e) => {
                tracing::error!("io: {e:?}");
                internal!()
            }
            Self::ParseInt(e) => {
                tracing::error!("parseint: {e:?}");
                internal!()
            }
            Self::RateLimited(limit) => (
                StatusCode::TOO_MANY_REQUESTS,
                ResponseJson::new(format!("{prefix} {limit} seconds")),
            ),
            Self::RedisError(e) => {
                tracing::error!("{e:?}");
                if e.kind() == &ErrorKind::IO {
                    exit();
                }
                internal!()
            }
            Self::Reqwest(e) => {
                tracing::error!("{e:?}");
                internal!()
            }
            Self::SerdeJson(e) => {
                tracing::error!("serde: {e:?}");
                internal!(S!("incoming json error"))
            }
            Self::SqlxError(e) => {
                tracing::error!("sqlx:: {e:?}");
                match e {
                    sqlx::Error::Io(_) | sqlx::Error::PoolClosed | sqlx::Error::PoolTimedOut => {
                        exit();
                    }
                    _ => (),
                }
                internal!(S!("database error"))
            }

            Self::UnknownInDb(variety) => (
                StatusCode::NOT_FOUND,
                ResponseJson::new(format!("{prefix} {variety}")),
            ),
        };

        (status, body).into_response()
    }
}
