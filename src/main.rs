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
use crossfire::mpsc::Array;
use fred::clients::Pool;
use parse_env::AppEnv;
use sqlx::PgPool;
use tracing_subscriber::{fmt, prelude::*};

use crate::{
    db_postgres::{ModelIncomingRequest, MsgIncomingRequest},
    scraper::Scraper,
};
// TODO incoming request in redis, use a sorted set for all others than aren't /v0/known etc

// TODO create a custom struct for these channels, with a trait etc
pub type Tx<T> = crossfire::MAsyncTx<Array<T>>;
pub type RX<T> = crossfire::AsyncRx<Array<T>>;
pub type OneTX<T> = crossfire::oneshot::TxOneshot<T>;
pub type OneRX<T> = crossfire::oneshot::RxOneshot<T>;

/// Create a bounded async channel of x size
fn create_rx_tx<T: 'static + Send + Sync>(size: usize) -> (Tx<T>, RX<T>) {
    crossfire::mpsc::bounded_async::<T>(size)
}

/// Create a onehsot channel
fn create_oneshot<T: 'static + Send + Sync>() -> (OneTX<T>, OneRX<T>) {
    crossfire::oneshot::oneshot()
}

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

/// This initial seeding is slow, will block until complete
/// Ideally put the daily stats into redis, but would a decent amount of work
async fn start_incoming_requests(
    postgres: &PgPool,
    redis: &Pool,
) -> Result<Tx<MsgIncomingRequest>, AppError> {
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

    let tx_scraper = Scraper::start(&app_env, &postgres);
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

// TODO put common test things here, e.g. EXAMPL_PHOTO_NAME
