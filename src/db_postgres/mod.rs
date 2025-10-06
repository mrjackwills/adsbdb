use sqlx::{ConnectOptions, PgPool, postgres::PgPoolOptions};

mod model_aircraft;
mod model_airline;
mod model_airport;
mod model_flightroute;
mod model_request_stats;

pub use model_aircraft::ModelAircraft;
pub use model_airline::ModelAirline;
pub use model_airport::ModelAirport;
pub use model_flightroute::ModelFlightroute;
pub use model_request_stats::{EntryCount, ModelRequestStatistics, RequestStatMsg};

use crate::{api::AppError, parse_env::AppEnv};

pub async fn get_pool(app_env: &AppEnv) -> Result<PgPool, AppError> {
    let mut options = sqlx::postgres::PgConnectOptions::new_without_pgpass()
        .host(&app_env.pg_host)
        .port(app_env.pg_port)
        .database(&app_env.pg_database)
        .username(&app_env.pg_user)
        .password(&app_env.pg_pass);

    match app_env.log_level {
        tracing::Level::TRACE | tracing::Level::DEBUG => (),
        _ => options = options.disable_statement_logging(),
    }

    Ok(PgPoolOptions::new()
        .max_connections(32)
        .connect_with(options)
        .await?)
}

/// Generic PostgreSQL ID
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, serde::Deserialize)]
struct ID<T> {
    id: T,
}

/// This macro generates a newtype wrapper around `i64`, providing useful trait implementations for custom specific ID types
#[macro_export]
macro_rules! generic_id {
    ($struct_name:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            Hash,
            Eq,
            PartialEq,
            PartialOrd,
            Ord,
            sqlx::Decode,
            serde::Deserialize,
        )]
        pub struct $struct_name(i64);

        impl sqlx::Type<sqlx::Postgres> for $struct_name {
            fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
                <i64 as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        impl From<i64> for $struct_name {
            fn from(x: i64) -> Self {
                Self(x)
            }
        }

        impl fred::types::FromValue for $struct_name {
            fn from_value(value: fred::prelude::Value) -> Result<Self, fred::prelude::Error> {
                value.as_i64().map_or(
                    Err(fred::error::Error::new(
                        fred::error::ErrorKind::Parse,
                        format!("FromRedis: {}", stringify!($struct_name)),
                    )),
                    |i| Ok(Self(i)),
                )
            }
        }

        impl serde::Serialize for $struct_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_newtype_struct(stringify!($struct_name), &self.0)
            }
        }

        /// False alert here
        #[allow(dead_code)]
        impl $struct_name {
            pub const fn get(&self) -> i64 {
                self.0
            }
        }
    };
}
