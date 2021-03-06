mod api;
mod db_postgres;
mod db_redis;
mod n_number;
mod parse_env;
mod scraper;

use std::sync::Arc;

use api::AppError;
use parse_env::AppEnv;
use tokio::sync::Mutex;
use tracing::Level;
use tracing_subscriber::fmt::writer::MakeWriterExt;

fn setup_tracing(app_envs: &AppEnv) {
    let level = if app_envs.trace {
        Level::TRACE
    } else if app_envs.debug {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let logfile = tracing_appender::rolling::never(&app_envs.location_logs, "api.log");
    let stdout = std::io::stdout.with_max_level(level);

    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_file(true)
        .with_line_number(true)
        .with_writer(logfile.and(stdout))
        .init();
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let app_env = parse_env::AppEnv::get_env();
    setup_tracing(&app_env);
    let postgres = db_postgres::db_pool(&app_env).await?;
    let redis = db_redis::get_connection(&app_env).await?;
    api::serve(app_env, postgres, Arc::new(Mutex::new(redis))).await?;
    Ok(())
}
