mod api;
mod db_postgres;
mod db_redis;
mod parse_env;
mod scraper;
use parse_env::AppEnv;
use tracing::Level;

fn setup_tracing(app_envs: &AppEnv) {
    let level = if app_envs.trace {
        Level::TRACE
    } else if app_envs.debug {
        Level::DEBUG
    } else {
        Level::INFO
    };
    tracing_subscriber::fmt().with_max_level(level).init();
}

#[tokio::main]
async fn main() {
    let app_env = parse_env::AppEnv::get_env();
    setup_tracing(&app_env);
    let postgres = db_postgres::db_pool(&app_env).await.unwrap();
    let redis = db_redis::get_connection(&app_env).await.unwrap();
    api::serve(app_env, postgres, redis).await;
}
