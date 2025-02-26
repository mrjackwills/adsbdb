use sqlx::{ConnectOptions, PgPool, postgres::PgPoolOptions};

mod model_aircraft;
mod model_airline;
mod model_airport;
mod model_flightroute;

pub use model_aircraft::ModelAircraft;
pub use model_airline::ModelAirline;
pub use model_airport::ModelAirport;
pub use model_flightroute::ModelFlightroute;

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
