use fred::clients::Pool;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod api;
mod argon;
mod db_postgres;
mod db_redis;
mod n_number;
mod parse_env;
mod scraper;

use api::AppError;
use parse_env::AppEnv;
use sqlx::PgPool;
use tracing_subscriber::{fmt, prelude::__tracing_subscriber_SubscriberExt};

use crate::{
    db_postgres::{ModelIncomingRequest, MsgIncomingRequest},
    scraper::{MsgScraper, Scraper},
};

/// Simple macro to create an empty String, or create String from a &str - to get rid of .to_owned() / String::from() etc
#[macro_export]
macro_rules! S {
    () => {
        String::new()
    };
    ($s:expr) => {
        String::from($s)
    };
}

fn setup_tracing(app_env: &AppEnv) -> Result<(), AppError> {
    let logfile = tracing_appender::rolling::weekly(&app_env.location_logs, "api.log");

    let log_fmt = fmt::Layer::default()
        .json()
        .flatten_event(true)
        .with_writer(logfile);

    match tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            .with_file(true)
            .with_line_number(true)
            .with_max_level(app_env.log_level)
            .finish()
            .with(log_fmt),
    ) {
        Ok(()) => Ok(()),
        Err(e) => {
            println!("{e:?}");
            Err(AppError::Internal(S!("Unable to start tracing")))
        }
    }
}

fn start_scraper(
    app_env: &AppEnv,
    postgres: &PgPool,
) -> Result<async_channel::Sender<MsgScraper>, AppError> {
    Ok(Scraper::start(app_env, postgres))
}

/// This initial seeding is slow, will block until complete
/// Ideally put the daily stats into redis, but would a decent amount of work
async fn start_incoming_requests(
    postgres: &PgPool,
    redis: &Pool,
) -> Result<async_channel::Sender<MsgIncomingRequest>, AppError> {
    ModelIncomingRequest::start(postgres, redis).await
}

async fn start() -> Result<(), AppError> {
    let app_env = parse_env::AppEnv::get_env();
    setup_tracing(&app_env)?;
    tracing::info!("{} - {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    let (postgres, redis) = tokio::try_join!(
        db_postgres::get_pool(&app_env),
        db_redis::get_pool(&app_env),
    )?;

    let tx_scraper = start_scraper(&app_env, &postgres)?;
    let tx_stats = start_incoming_requests(&postgres, &redis).await?;

    api::serve(app_env, postgres, redis, tx_scraper, tx_stats).await
}

#[tokio::main]
async fn main() {
    tokio::spawn(async move {
        if let Err(e) = start().await {
            tracing::error!("{e}");
        }
    })
    .await
    .ok();
}
