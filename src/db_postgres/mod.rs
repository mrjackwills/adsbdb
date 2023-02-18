use std::time::Duration;

// use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, PgPool};

mod model_aircraft;
mod model_airline;
mod model_airport;
mod model_flightroute;

pub use model_aircraft::ModelAircraft;
pub use model_airline::ModelAirline;
pub use model_airport::ModelAirport;
pub use model_flightroute::ModelFlightroute;

use crate::{api::AppError, parse_env::AppEnv};

pub async fn db_pool(app_env: &AppEnv) -> Result<PgPool, AppError> {
    let options = sqlx::postgres::PgConnectOptions::new()
        .host(&app_env.pg_host)
        .port(app_env.pg_port)
        .database(&app_env.pg_database)
        .username(&app_env.pg_user)
        .password(&app_env.pg_pass);

    let acquire_timeout = Duration::from_secs(5);
    let idle_timeout = Duration::from_secs(30);

    Ok(PgPoolOptions::new()
        .max_connections(20)
        .idle_timeout(idle_timeout)
        .acquire_timeout(acquire_timeout)
        .connect_with(options)
        .await?)
}
