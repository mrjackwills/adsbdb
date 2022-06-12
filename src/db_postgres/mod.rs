use anyhow::Result;
use async_trait::async_trait;
use sqlx::{ConnectOptions, PgPool};

mod migrations;
mod model_aircraft;
mod model_airport;
mod model_flightroute;

pub use model_aircraft::ModelAircraft;
pub use model_airport::ModelAirport;
pub use model_flightroute::ModelFlightroute;

use crate::{api::AppError, parse_env::AppEnv};

#[async_trait]
pub trait Model<T> {
    async fn get(db: &PgPool, input: &str) -> Result<Option<T>, AppError>;
}

pub async fn db_pool(app_env: &AppEnv) -> Result<PgPool, AppError> {
    let mut options = sqlx::postgres::PgConnectOptions::new()
        .host(&app_env.pg_host)
        .port(app_env.pg_port)
        .database(&app_env.pg_database)
        .username(&app_env.pg_user)
        .password(&app_env.pg_pass);

    if !app_env.debug && !app_env.trace {
        options.disable_statement_logging();
    }

    let pool = sqlx::PgPool::connect_with(options).await?;
    migrations::migrations(&pool).await?;
    Ok(pool)
}
