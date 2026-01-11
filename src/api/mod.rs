use fred::clients::Pool;
use router::ApiRoutes;
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

use axum::{
    Router,
    extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts, State},
    http::{HeaderMap, Request},
    middleware::{self, Next},
    response::Response,
    routing::{get, patch},
};
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::LazyLock,
    time::Instant,
};
use tokio::signal;
use tower::ServiceBuilder;
use tracing::info;

mod app_error;
mod input;
mod response;
mod router;
mod update_routes;

use crate::{
    S,
    db_postgres::{MsgIncomingRequest, UriMethod},
    db_redis::ratelimit::RateLimit,
    parse_env::AppEnv,
    scraper::MsgScraper,
};
pub use app_error::*;
pub use input::{AircraftSearch, AirlineCode, Callsign, ModeS, NNumber, Registration, Validate};
pub use response::{ResponseAircraft, Stats, StatsEntry};

const X_REAL_IP: &str = "x-real-ip";
const X_FORWARDED_FOR: &str = "x-forwarded-for";

#[derive(Clone)]
pub struct ApplicationState {
    postgres: PgPool,
    redis: Pool,
    uptime: Instant,
    scraper_tx: async_channel::Sender<MsgScraper>,
    stats_tx: async_channel::Sender<MsgIncomingRequest>,
    url_prefix: String,
}

impl ApplicationState {
    pub fn new(
        app_env: &AppEnv,
        postgres: PgPool,
        redis: Pool,
        scraper_tx: async_channel::Sender<MsgScraper>,
        stats_tx: async_channel::Sender<MsgIncomingRequest>,
    ) -> Self {
        Self {
            postgres,
            redis,
            uptime: Instant::now(),
            scraper_tx,
            stats_tx,
            url_prefix: app_env.url_photo_prefix.clone(),
        }
    }
}

/// extract `x-forwarded-for` header
fn x_forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get(X_FORWARDED_FOR)
        .and_then(|x| x.to_str().ok())
        .and_then(|s| s.split(',').find_map(|s| s.trim().parse::<IpAddr>().ok()))
}

/// extract the `x-real-ip` header
fn x_real_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get(X_REAL_IP)
        .and_then(|x| x.to_str().ok())
        .and_then(|s| s.parse::<IpAddr>().ok())
}

/// Get a users ip address, application should always be behind an nginx reverse proxy
/// so header x-forwarded-for should always be valid, but if not, then try x-real-ip
/// if neither headers work, use the optional socket address from axum
pub fn get_ip(headers: &HeaderMap, addr: ConnectInfo<SocketAddr>) -> IpAddr {
    x_forwarded_for(headers)
        .or_else(|| x_real_ip(headers))
        .unwrap_or_else(|| addr.0.ip())
}

/// Limit the users request based on ip address, using redis as mem store
async fn rate_limiting(
    State(state): State<ApplicationState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    let (mut parts, body) = req.into_parts();
    let addr = ConnectInfo::<SocketAddr>::from_request_parts(&mut parts, &state).await?;

    state
        .stats_tx
        .send(MsgIncomingRequest::Url(UriMethod::from(&parts)))
        .await
        .ok();
    let ip = get_ip(&parts.headers, addr);
    RateLimit::new(ip).check(&state.redis).await?;
    Ok(next.run(Request::from_parts(parts, body)).await)
}

static API_VERSION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "/v{}",
        env!("CARGO_PKG_VERSION")
            .split('.')
            .take(1)
            .collect::<String>()
    )
});

/// Create api routes from a given ident and path
#[macro_export]
macro_rules! define_routes {
    ($enum_name:ident, $($variant:ident => $route:expr),*) => {
        enum $enum_name {
            $($variant,)*
        }

        impl $enum_name {
            fn addr(&self) -> String {
                let route_name = match self {
                    $(Self::$variant => $route,)*
                };
                format!("/{}", route_name)
            }
        }
    };
}

define_routes!(
    Routes,
    AircraftRandom => "aircraft/random",
    Aircraft => "aircraft/{mode_s}",
    AirlineRandom => "airline/random",
    Airline => "airline/{airline}",
    CallsignRandom => "callsign/random",
    Callsign => "callsign/{callsign}",
    Online => "online",
    NNumber => "n-number/{n-number}",
    ModeS => "mode-s/{mode_s}",
    Stats => "stats"

);

/// Get an useable axum address, from app_env:host+port
fn get_addr(app_env: &AppEnv) -> Result<SocketAddr, AppError> {
    match (app_env.api_host.clone(), app_env.api_port).to_socket_addrs() {
        Ok(i) => i
            .take(1)
            .collect::<Vec<SocketAddr>>()
            .first()
            .map_or_else(|| Err(AppError::Internal(S!("No addr"))), |addr| Ok(*addr)),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

/// Serve the app!
#[allow(clippy::cognitive_complexity)]
pub async fn serve(
    app_env: AppEnv,
    postgres: PgPool,
    redis: Pool,
    tx_scraper: async_channel::Sender<MsgScraper>,
    tx_stats: async_channel::Sender<MsgIncomingRequest>,
) -> Result<(), AppError> {
    let application_state = ApplicationState::new(&app_env, postgres, redis, tx_scraper, tx_stats);

    let mut api_router = Router::new()
        .route(
            &Routes::AircraftRandom.addr(),
            get(ApiRoutes::aircraft_random_get),
        )
        .route(&Routes::Aircraft.addr(), get(ApiRoutes::aircraft_get))
        .route(
            &Routes::AirlineRandom.addr(),
            get(ApiRoutes::airline_random_get),
        )
        .route(&Routes::Airline.addr(), get(ApiRoutes::airline_get))
        .route(
            &Routes::CallsignRandom.addr(),
            get(ApiRoutes::callsign_random_get),
        )
        .route(&Routes::Callsign.addr(), get(ApiRoutes::callsign_get))
        .route(&Routes::Online.addr(), get(ApiRoutes::online_get))
        .route(&Routes::NNumber.addr(), get(ApiRoutes::n_number_get))
        .route(&Routes::ModeS.addr(), get(ApiRoutes::mode_s_get))
        .route(&Routes::Stats.addr(), get(ApiRoutes::stats_get));

    // If .env flag is set, enable update routes
    let mut allowed_methods = vec![axum::http::Method::GET];
    if let Some(update_hash) = &app_env.allow_update {
        api_router = api_router
            .route(
                &Routes::Callsign.addr(),
                patch(update_routes::callsign_patch).layer(middleware::from_fn_with_state(
                    update_hash.clone(),
                    update_routes::auth_header,
                )),
            )
            .route(
                &Routes::Aircraft.addr(),
                patch(update_routes::aircraft_patch).layer(middleware::from_fn_with_state(
                    update_hash.clone(),
                    update_routes::auth_header,
                )),
            );
        allowed_methods.push(axum::http::Method::PATCH);
    }

    let cors = CorsLayer::new()
        .allow_headers(Any)
        .allow_methods(allowed_methods)
        .allow_origin(Any);

    let app = Router::new()
        .nest(API_VERSION.as_str(), api_router)
        .fallback(ApiRoutes::fallback)
        .with_state(application_state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(DefaultBodyLimit::max(1024))
                .layer(cors)
                .layer(middleware::from_fn_with_state(
                    application_state,
                    rate_limiting,
                )),
        );

    let addr = get_addr(&app_env)?;
    info!("{} - {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    info!("starting server @ {addr}{}", API_VERSION.as_str());
    info!(
        "scrape_flightroute: {}, scrape_photo: {}",
        app_env.allow_scrape_flightroute.is_some(),
        app_env.allow_scrape_photo.is_some()
    );
    info!("updater: {}", app_env.allow_update.is_some(),);

    match axum::serve(
        tokio::net::TcpListener::bind(&addr).await?,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    {
        Ok(()) => Ok(()),
        Err(_) => Err(AppError::Internal(S!("api_server"))),
    }
}

#[allow(clippy::expect_used)]
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    info!("signal received, starting graceful shutdown",);
}

/// http tests - ran via actual requests to a (local) server
/// cargo watch -q -c -w src/ -x 'test http_mod -- --test-threads=1 --nocapture'
#[cfg(test)]
#[allow(clippy::pedantic, clippy::unwrap_used)]
pub mod tests {

    use super::*;

    use crate::db_postgres;
    use crate::db_redis;
    use crate::parse_env;
    use crate::start_incoming_requests;
    use crate::start_scraper;

    use fred::interfaces::ClientLike;
    use fred::interfaces::KeysInterface;
    use fred::prelude::HashesInterface;
    use reqwest::StatusCode;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use tokio::task::JoinHandle;

    /// Delete all entites in temp_incoming_request table, and incoming_request & incoming_rewuest_url which are younger than 12 hours old
    pub async fn delete_incoming_request(db: &PgPool) {
        sqlx::query("DELETE FROM incoming_request WHERE timestamp >= (CURRENT_TIMESTAMP - INTERVAL '12 hours')")
            .execute(db)
            .await
            .unwrap();
        sqlx::query("DELETE FROM temp_incoming_request")
            .execute(db)
            .await
            .unwrap();
        sqlx::query("DELETE FROM incoming_request_url WHERE timestamp >= (CURRENT_TIMESTAMP - INTERVAL '12 hours')")
            .execute(db)
            .await
            .unwrap();
    }

    pub static CLIENT: LazyLock<reqwest::Client> =
        LazyLock::new(|| reqwest::ClientBuilder::new().build().unwrap());

    pub struct TestSetup {
        pub _handle: Option<JoinHandle<()>>,
        pub app_env: AppEnv,
        pub postgres: PgPool,
        pub redis: Pool,
    }

    impl TestSetup {
        pub async fn flush_redis(&self) {
            self.redis.flushall::<()>(true).await.unwrap();
        }
    }

    // Get basic api params, also flushes all redis keys
    pub async fn test_setup() -> TestSetup {
        let app_env = parse_env::AppEnv::get_env();
        let (postgres, redis) = tokio::try_join!(
            db_postgres::get_pool(&app_env),
            db_redis::get_pool(&app_env)
        )
        .unwrap();
        delete_incoming_request(&postgres).await;
        let setup = TestSetup {
            _handle: None,
            app_env,
            postgres,
            redis,
        };
        setup.flush_redis().await;
        setup
    }

    #[macro_export]
    /// Sleep for a given number of milliseconds, is an async fn.
    /// If no parameter supplied, defaults to 1000ms
    macro_rules! sleep {
        () => {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        };
        ($ms:expr) => {
            tokio::time::sleep(std::time::Duration::from_millis($ms)).await;
        };
    }

    async fn start_server() -> TestSetup {
        let setup = test_setup().await;

        let postgres = setup.postgres.clone();
        let app_env = setup.app_env.clone();
        let redis = setup.redis.clone();

        // need to set up scrapers here
        let tx_scraper = start_scraper(&app_env).await.unwrap();
        let tx_stats = start_incoming_requests(&app_env).await.unwrap();
        let handle = tokio::spawn(async {
            serve(app_env, postgres, redis, tx_scraper, tx_stats)
                .await
                .unwrap();
        });
        // just sleep to make sure the server is running - 1ms is enough
        // Will seed that stats in redis, so now need 100ms
        sleep!(100);
        TestSetup {
            _handle: Some(handle),
            app_env: setup.app_env,
            postgres: setup.postgres,
            redis: setup.redis,
        }
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    struct TestResponseValue {
        response: Value,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    struct TestResponseT<T> {
        response: T,
    }

    #[test]
    fn http_mod_get_api_version() {
        assert_eq!(API_VERSION.as_str(), S!("/v0"));
    }

    /// Make request to all endpoints to seed incoming_request entries
    async fn test_seed_stats() {
        let urls = [
            "/aircraft/test",
            "/airline/test",
            "/callsign/test",
            "/mode-s/test",
            "/n-number/test",
            "/online",
        ];
        for i in urls.iter().enumerate() {
            let url = format!("http://127.0.0.1:8282{}{}", API_VERSION.as_str(), i.1);
            for _ in 0..=i.0 {
                CLIENT.get(&url).send().await.unwrap();
                // Sleep to allow the insert_request thread to correctly insert/update entries
                sleep!(50);
            }
        }
    }

    async fn assert_empty_stats_cache(redis: &Pool) {
        let stats_cache: Option<String> = redis.hget("stats", "data").await.unwrap();
        assert!(stats_cache.is_some());
        let cache = serde_json::from_str::<Stats>(&stats_cache.unwrap()).unwrap();

        assert_eq!(cache.daily.aircraft, vec![]);
        assert_eq!(cache.daily.aircraft, vec![]);
        assert_eq!(cache.daily.callsign, vec![]);
        assert_eq!(cache.daily.n_number, vec![]);
        assert_eq!(cache.daily.mode_s, vec![]);
        assert_eq!(cache.daily.online, vec![]);
        assert_eq!(cache.daily.stats, vec![]);
        assert_eq!(cache.daily.aggregate, 0);
    }

    #[tokio::test]
    /// Test the stats endpoint, can only really test the "daily" key, as the "total" key will have real data in it
    /// Check that the stats cache is correctly populated and ttl/expire working as expected
    async fn http_mod_get_stats() {
        let setup = start_server().await;
        assert_empty_stats_cache(&setup.redis).await;

        let url = format!("http://127.0.0.1:8282{}/stats", API_VERSION.as_str(),);
        let result = CLIENT
            .get(&url)
            .send()
            .await
            .unwrap()
            .json::<TestResponseT<Stats>>()
            .await
            .unwrap();

        sleep!(1500);
        assert_eq!(result.response.daily.aircraft, vec![]);
        assert_eq!(result.response.daily.aircraft, vec![]);
        assert_eq!(result.response.daily.callsign, vec![]);
        assert_eq!(result.response.daily.mode_s, vec![]);
        assert_eq!(result.response.daily.n_number, vec![]);
        assert_eq!(result.response.daily.online, vec![]);
        assert_eq!(result.response.daily.stats, vec![]);
        assert_eq!(result.response.daily.aggregate, 0);

        test_seed_stats().await;

        // Cache only gets update every ten minutes, so should still be empty here
        assert_empty_stats_cache(&setup.redis).await;
        setup.flush_redis().await;

        let result = CLIENT
            .get(&url)
            .send()
            .await
            .unwrap()
            .json::<TestResponseT<Stats>>()
            .await
            .unwrap();

        assert_eq!(
            serde_json::to_string(&result.response.daily).unwrap(),
            r#"{"aircraft":[{"url":"/v0/aircraft/test","count":1}],"airline":[{"url":"/v0/airline/test","count":2}],"callsign":[{"url":"/v0/callsign/test","count":3}],"mode_s":[{"url":"/v0/mode-s/test","count":4}],"n_number":[{"url":"/v0/n-number/test","count":5}],"online":[{"url":"/v0/online","count":6}],"stats":[{"url":"/v0/stats","count":2}],"aggregate":23}"#
        );

        let ttl: i64 = setup.redis.ttl("stats").await.unwrap();
        assert!((296..=300).contains(&ttl));
    }

    #[tokio::test]
    /// Check that stats older than 1 day get removed on any interaction with the api
    async fn http_mod_get_stats_old_removed() {
        let setup = start_server().await;
        let url = format!("http://127.0.0.1:8282{}/stats", API_VERSION.as_str(),);
        assert_empty_stats_cache(&setup.redis).await;
        test_seed_stats().await;
        setup.flush_redis().await;

        sqlx::query!(
            "UPDATE temp_incoming_request SET timestamp = (CURRENT_TIMESTAMP - INTERVAL '25 hours')"
        )
        .execute(&setup.postgres)
        .await
        .unwrap();

        setup.flush_redis().await;
        CLIENT
            .get(&url)
            .send()
            .await
            .unwrap()
            .json::<TestResponseT<Stats>>()
            .await
            .unwrap();
        sleep!(1500);
        let result = CLIENT
            .get(&url)
            .send()
            .await
            .unwrap()
            .json::<TestResponseT<Stats>>()
            .await
            .unwrap();

        assert_eq!(result.response.daily.aircraft, vec![]);
        assert_eq!(result.response.daily.aircraft, vec![]);
        assert_eq!(result.response.daily.callsign, vec![]);
        assert_eq!(result.response.daily.mode_s, vec![]);
        assert_eq!(result.response.daily.n_number, vec![]);
        assert_eq!(result.response.daily.online, vec![]);
        assert_eq!(result.response.daily.aggregate, 1);
    }

    #[tokio::test]
    async fn http_mod_get_icao_callsign() {
        start_server().await;
        let callsign = "ACA959";
        let url = format!(
            "http://127.0.0.1:8282{}/callsign/{}",
            API_VERSION.as_str(),
            callsign
        );
        let resp = CLIENT.get(&url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        let result = result.get("flightroute").unwrap();

        assert!(result.get("aircraft").is_none());

        assert_eq!(result["callsign"], callsign.to_uppercase());
        assert_eq!(result["callsign_icao"], callsign.to_owned());
        assert_eq!(result["callsign_iata"], "AC959".to_uppercase());
        assert_eq!(result["origin"]["country_name"], "Canada");
        assert_eq!(result["origin"]["elevation"], 118);
        assert_eq!(result["origin"]["country_iso_name"], "CA");
        assert_eq!(result["origin"]["iata_code"], "YUL");
        assert_eq!(result["origin"]["icao_code"], "CYUL");
        assert_eq!(result["origin"]["latitude"], 45.470_600_128_2);
        assert_eq!(result["origin"]["longitude"], -73.740_798_950_2,);
        assert_eq!(result["origin"]["municipality"], "Montréal");
        assert_eq!(
            result["origin"]["name"],
            "Montreal / Pierre Elliott Trudeau International Airport"
        );

        assert!(result.get("midpoint").is_none());

        assert_eq!(result["destination"]["country_iso_name"], "CR");
        assert_eq!(result["destination"]["country_name"], "Costa Rica");
        assert_eq!(result["destination"]["elevation"], 3021);
        assert_eq!(result["destination"]["iata_code"], "SJO");
        assert_eq!(result["destination"]["icao_code"], "MROC");
        assert_eq!(result["destination"]["latitude"], 9.993_86);
        assert_eq!(result["destination"]["longitude"], -84.208801);
        assert_eq!(result["destination"]["municipality"], "San José (Alajuela)");
        assert_eq!(
            result["destination"]["name"],
            "Juan Santamaría International Airport"
        );
    }

    #[tokio::test]
    async fn http_mod_get_iata_callsign() {
        start_server().await;
        let callsign = "AC959";
        let url = format!(
            "http://127.0.0.1:8282{}/callsign/{}",
            API_VERSION.as_str(),
            callsign
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        let result = result.get("flightroute").unwrap();

        assert!(result.get("aircraft").is_none());

        assert_eq!(result["callsign"], callsign.to_uppercase());
        assert_eq!(result["callsign_icao"], S!("ACA959"));
        assert_eq!(result["callsign_iata"], callsign.to_uppercase());
        assert_eq!(result["origin"]["country_name"], "Canada");
        assert_eq!(result["origin"]["elevation"], 118);
        assert_eq!(result["origin"]["country_iso_name"], "CA");
        assert_eq!(result["origin"]["iata_code"], "YUL");
        assert_eq!(result["origin"]["icao_code"], "CYUL");
        assert_eq!(result["origin"]["latitude"], 45.470_600_128_2);
        assert_eq!(result["origin"]["longitude"], -73.740_798_950_2,);
        assert_eq!(result["origin"]["municipality"], "Montréal");
        assert_eq!(
            result["origin"]["name"],
            "Montreal / Pierre Elliott Trudeau International Airport"
        );

        assert!(result.get("midpoint").is_none());

        assert_eq!(result["destination"]["country_iso_name"], "CR");
        assert_eq!(result["destination"]["country_name"], "Costa Rica");
        assert_eq!(result["destination"]["elevation"], 3021);
        assert_eq!(result["destination"]["iata_code"], "SJO");
        assert_eq!(result["destination"]["icao_code"], "MROC");
        assert_eq!(result["destination"]["latitude"], 9.993_86);
        assert_eq!(result["destination"]["longitude"], -84.208801);
        assert_eq!(result["destination"]["municipality"], "San José (Alajuela)");
        assert_eq!(
            result["destination"]["name"],
            "Juan Santamaría International Airport"
        );
    }

    #[tokio::test]
    async fn http_mod_get_icao_callsign_with_midpoint() {
        start_server().await;
        let callsign = "QFA31";
        let url = format!(
            "http://127.0.0.1:8282{}/callsign/{}",
            API_VERSION.as_str(),
            callsign
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        let result = result.get("flightroute").unwrap();

        assert!(result.get("aircraft").is_none());

        assert_eq!(result["callsign"], callsign.to_uppercase());
        assert_eq!(result["callsign_iata"], "QF31".to_uppercase());
        assert_eq!(result["callsign_icao"], callsign.to_uppercase());
        assert_eq!(result["origin"]["country_iso_name"], "AU".to_uppercase());
        assert_eq!(result["origin"]["country_name"], "Australia");
        assert_eq!(result["origin"]["elevation"], 21);
        assert_eq!(result["origin"]["iata_code"], "SYD");
        assert_eq!(result["origin"]["icao_code"], "YSSY");
        assert_eq!(result["origin"]["latitude"], -33.946_098_327_636_72);
        assert_eq!(result["origin"]["longitude"], 151.177_001_953_125);
        assert_eq!(result["origin"]["municipality"], "Sydney");
        assert_eq!(
            result["origin"]["name"],
            "Sydney Kingsford Smith International Airport"
        );

        assert_eq!(result["midpoint"]["country_iso_name"], "SG".to_uppercase());
        assert_eq!(result["midpoint"]["country_name"], "Singapore");
        assert_eq!(result["midpoint"]["elevation"], 22);
        assert_eq!(result["midpoint"]["iata_code"], "SIN");
        assert_eq!(result["midpoint"]["icao_code"], "WSSS");
        assert_eq!(result["midpoint"]["latitude"], 1.35019);
        assert_eq!(result["midpoint"]["longitude"], 103.994_003);
        assert_eq!(result["midpoint"]["municipality"], "Singapore");
        assert_eq!(result["midpoint"]["name"], "Singapore Changi Airport");

        assert_eq!(result["destination"]["country_iso_name"], "GB");
        assert_eq!(result["destination"]["country_name"], "United Kingdom");
        assert_eq!(result["destination"]["elevation"], 83);
        assert_eq!(result["destination"]["iata_code"], "LHR");
        assert_eq!(result["destination"]["icao_code"], "EGLL");
        assert_eq!(result["destination"]["latitude"], 51.4706);
        assert_eq!(result["destination"]["longitude"], -0.461_941);
        assert_eq!(result["destination"]["municipality"], "London");
        assert_eq!(result["destination"]["name"], "London Heathrow Airport");
    }

    #[tokio::test]
    async fn http_mod_get_iata_callsign_with_midpoint() {
        start_server().await;
        let callsign = "QF31";
        let url = format!(
            "http://127.0.0.1:8282{}/callsign/{}",
            API_VERSION.as_str(),
            callsign
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        let result = result.get("flightroute").unwrap();

        assert!(result.get("aircraft").is_none());

        assert_eq!(result["callsign"], callsign.to_uppercase());
        assert_eq!(result["callsign_iata"], callsign.to_uppercase());
        assert_eq!(result["callsign_icao"], "QFA31");
        assert_eq!(result["origin"]["country_iso_name"], "AU".to_uppercase());
        assert_eq!(result["origin"]["country_name"], "Australia");
        assert_eq!(result["origin"]["elevation"], 21);
        assert_eq!(result["origin"]["iata_code"], "SYD");
        assert_eq!(result["origin"]["icao_code"], "YSSY");
        assert_eq!(result["origin"]["latitude"], -33.946_098_327_636_72);
        assert_eq!(result["origin"]["longitude"], 151.177_001_953_125);
        assert_eq!(result["origin"]["municipality"], "Sydney");
        assert_eq!(
            result["origin"]["name"],
            "Sydney Kingsford Smith International Airport"
        );

        assert_eq!(result["midpoint"]["country_iso_name"], "SG".to_uppercase());
        assert_eq!(result["midpoint"]["country_name"], "Singapore");
        assert_eq!(result["midpoint"]["elevation"], 22);
        assert_eq!(result["midpoint"]["iata_code"], "SIN");
        assert_eq!(result["midpoint"]["icao_code"], "WSSS");
        assert_eq!(result["midpoint"]["latitude"], 1.35019);
        assert_eq!(result["midpoint"]["longitude"], 103.994_003);
        assert_eq!(result["midpoint"]["municipality"], "Singapore");
        assert_eq!(result["midpoint"]["name"], "Singapore Changi Airport");

        assert_eq!(result["destination"]["country_iso_name"], "GB");
        assert_eq!(result["destination"]["country_name"], "United Kingdom");
        assert_eq!(result["destination"]["elevation"], 83);
        assert_eq!(result["destination"]["iata_code"], "LHR");
        assert_eq!(result["destination"]["icao_code"], "EGLL");
        assert_eq!(result["destination"]["latitude"], 51.4706);
        assert_eq!(result["destination"]["longitude"], -0.461_941);
        assert_eq!(result["destination"]["municipality"], "London");
        assert_eq!(result["destination"]["name"], "London Heathrow Airport");
    }

    #[tokio::test]
    async fn http_mod_get_callsign_unknown() {
        start_server().await;
        let callsign = "ABABAB";
        let url = format!(
            "http://127.0.0.1:8282{}/callsign/{}",
            API_VERSION.as_str(),
            callsign
        );
        let response = CLIENT.get(url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let result = response.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "unknown callsign");
    }

    #[tokio::test]
    /// /callsign/random, check response, check cache, clear cache then check against a /callsign/{callsign} response
    async fn http_mod_get_callsign_random() {
        let setup = start_server().await;
        let url = format!(
            "http://127.0.0.1:8282{}/callsign/random",
            API_VERSION.as_str(),
        );
        let response = CLIENT.get(url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let random_test_response = response.json::<TestResponseValue>().await.unwrap();

        let test_response = random_test_response.clone();

        let result = random_test_response.response;
        assert!(result.get("flightroute").is_some());
        assert!(result.get("aircraft").is_none());
        let result = result.get("flightroute").unwrap();

        assert!(result["callsign"].is_string());
        assert!(result["callsign_icao"].is_string());

        if let Some(x) = result.get("callsign_iata") {
            assert!(x.is_string())
        }
        assert!(result["origin"]["country_name"].is_string());
        assert!(result["origin"]["elevation"].is_i64());
        assert!(result["origin"]["country_iso_name"].is_string());
        assert!(result["origin"]["iata_code"].is_string());
        assert!(result["origin"]["icao_code"].is_string());
        assert!(result["origin"]["latitude"].is_f64());
        assert!(result["origin"]["longitude"].is_f64());
        assert!(result["origin"]["municipality"].is_string());

        assert!(result["destination"]["country_name"].is_string());
        assert!(result["destination"]["elevation"].is_i64());
        assert!(result["destination"]["country_iso_name"].is_string());
        assert!(result["destination"]["iata_code"].is_string());
        assert!(result["destination"]["icao_code"].is_string());
        assert!(result["destination"]["latitude"].is_f64());
        assert!(result["destination"]["longitude"].is_f64());
        assert!(result["destination"]["municipality"].is_string());

        let callsign = result["callsign"].as_str().unwrap().to_owned();
        let callsign_icao = result["callsign_icao"].as_str().unwrap().to_owned();

        let callsign_cache: Option<String> = setup
            .redis
            .hget(format!("callsign::{callsign}"), "data")
            .await
            .unwrap();
        assert!(callsign_cache.is_some());
        assert!(!callsign_cache.unwrap().is_empty());

        let callsign_icao_cache: Option<String> = setup
            .redis
            .hget(format!("callsign::{callsign_icao}"), "data")
            .await
            .unwrap();
        assert!(callsign_icao_cache.is_some());
        assert!(!callsign_icao_cache.unwrap().is_empty());

        if let Some(callsign_iata) = result.get("callsign_iata") {
            let callsign_iata = callsign_iata.as_str().unwrap().to_owned();
            let callsign_iata_cache: Option<String> = setup
                .redis
                .hget(format!("callsign::{callsign_iata}"), "data")
                .await
                .unwrap();
            assert!(callsign_iata_cache.is_some());
            assert!(!callsign_iata_cache.unwrap().is_empty());
        }

        setup.flush_redis().await;

        let url = format!(
            "http://127.0.0.1:8282{}/callsign/{}",
            API_VERSION.as_str(),
            callsign
        );
        let resp = CLIENT
            .get(url)
            .send()
            .await
            .unwrap()
            .json::<TestResponseValue>()
            .await
            .unwrap();

        assert_eq!(test_response, resp);

        setup.flush_redis().await;

        let url = format!(
            "http://127.0.0.1:8282{}/callsign/{}",
            API_VERSION.as_str(),
            callsign_icao
        );
        let resp = CLIENT
            .get(url)
            .send()
            .await
            .unwrap()
            .json::<TestResponseValue>()
            .await
            .unwrap();
        assert_eq!(test_response, resp);

        if let Some(callsign_iata) = result.get("callsign_iata") {
            setup.flush_redis().await;

            let url = format!(
                "http://127.0.0.1:8282{}/callsign/{}",
                API_VERSION.as_str(),
                callsign_iata.as_str().unwrap()
            );
            let resp = CLIENT
                .get(url)
                .send()
                .await
                .unwrap()
                .json::<TestResponseValue>()
                .await
                .unwrap();

            let mut resp = resp
                .response
                .as_object()
                .unwrap()
                .get("flightroute")
                .unwrap()
                .as_object()
                .unwrap()
                .to_owned();
            resp.remove("callsign");

            let mut test_resp = test_response
                .response
                .as_object()
                .unwrap()
                .get("flightroute")
                .unwrap()
                .as_object()
                .unwrap()
                .to_owned();
            test_resp.remove("callsign");

            assert_eq!(test_resp, resp);
        }
    }

    #[tokio::test]
    async fn http_mod_get_aircraft() {
        start_server().await;
        let mode_s = "4CABD2";
        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}",
            API_VERSION.as_str(),
            mode_s
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        assert!(result.get("flightroute").is_none());
        let result = result.get("aircraft").unwrap();

        assert_eq!(result["icao_type"], "A21N");
        assert_eq!(result["manufacturer"], "Airbus");
        assert_eq!(result["mode_s"], mode_s);
        assert_eq!(result["registration"], "EI-LRF");
        assert_eq!(result["registered_owner"], "Aer Lingus");
        assert_eq!(result["registered_owner_country_iso_name"], "IE");
        assert_eq!(result["registered_owner_country_name"], "Ireland");
        assert_eq!(result["registered_owner_operator_flag_code"], "EIN");
        assert_eq!(result["type"], "A321 253NXSL");
        assert_eq!(result["url_photo"].to_string(), "null");
        assert_eq!(result["url_photo_thumbnail"].to_string(), "null");
    }

    #[tokio::test]
    // search via registration when theres no flag
    async fn http_mod_get_aircraft_registration() {
        start_server().await;
        let registration = "G-HMGE";
        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}",
            API_VERSION.as_str(),
            registration
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        assert!(result.get("flightroute").is_none());
        let result = result.get("aircraft").unwrap();

        assert_eq!(result["icao_type"], "DA62");
        assert_eq!(result["manufacturer"], "Diamond");
        assert_eq!(result["mode_s"], "407ED9");
        assert_eq!(result["registration"], registration);
        assert_eq!(result["registered_owner"], "AMPA LTD");
        assert_eq!(result["registered_owner_country_iso_name"], "GB");
        assert_eq!(result["registered_owner_country_name"], "United Kingdom");
        assert_eq!(
            result["registered_owner_operator_flag_code"].to_string(),
            "null"
        );
        assert_eq!(result["type"], "DA 62");
        assert_eq!(result["url_photo"].to_string(), "null");
        assert_eq!(result["url_photo_thumbnail"].to_string(), "null");
    }

    #[tokio::test]
    async fn http_mod_get_aircraft_and_icao_callsign() {
        start_server().await;
        let mode_s = "A6D27B";
        let callsign = "ACA959";
        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}?callsign={}",
            API_VERSION.as_str(),
            mode_s,
            callsign
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        let aircraft_result = result.get("aircraft").unwrap();

        assert_eq!(aircraft_result["icao_type"], "CRJ7");
        assert_eq!(aircraft_result["manufacturer"], "Bombardier");
        assert_eq!(aircraft_result["mode_s"], mode_s);
        assert_eq!(aircraft_result["registration"], "N539GJ");
        assert_eq!(aircraft_result["registered_owner"], "United Express");
        assert_eq!(aircraft_result["registered_owner_country_iso_name"], "US");
        assert_eq!(
            aircraft_result["registered_owner_country_name"],
            "United States"
        );
        assert_eq!(
            aircraft_result["registered_owner_operator_flag_code"],
            "GJS"
        );
        assert_eq!(aircraft_result["type"], "CRJ 700 702");

        let flightroute_result = result.get("flightroute");
        assert!(flightroute_result.is_some());
        let flightroute_result = flightroute_result.unwrap();
        assert_eq!(flightroute_result["callsign"], callsign.to_uppercase());
        assert_eq!(flightroute_result["callsign_iata"], "AC959");
        assert_eq!(flightroute_result["callsign_icao"], callsign.to_uppercase());

        assert_eq!(flightroute_result["airline"]["name"], "Air Canada");
        assert_eq!(flightroute_result["airline"]["icao"], "ACA");
        assert_eq!(flightroute_result["airline"]["iata"], "AC");
        assert_eq!(flightroute_result["airline"]["callsign"], "AIR CANADA");
        assert_eq!(flightroute_result["airline"]["country"], "Canada");
        assert_eq!(flightroute_result["airline"]["country_iso"], "CA");

        assert_eq!(flightroute_result["origin"]["country_name"], "Canada");
        assert_eq!(flightroute_result["origin"]["elevation"], 118);
        assert_eq!(flightroute_result["origin"]["country_iso_name"], "CA");
        assert_eq!(flightroute_result["origin"]["iata_code"], "YUL");
        assert_eq!(flightroute_result["origin"]["icao_code"], "CYUL");
        assert_eq!(flightroute_result["origin"]["latitude"], 45.470_600_128_2);
        assert_eq!(flightroute_result["origin"]["longitude"], -73.740_798_950_2,);
        assert_eq!(flightroute_result["origin"]["municipality"], "Montréal");
        assert_eq!(
            flightroute_result["origin"]["name"],
            "Montreal / Pierre Elliott Trudeau International Airport"
        );

        assert!(result.get("midpoint").is_none());

        assert_eq!(flightroute_result["destination"]["country_iso_name"], "CR");
        assert_eq!(
            flightroute_result["destination"]["country_name"],
            "Costa Rica"
        );
        assert_eq!(flightroute_result["destination"]["elevation"], 3021);
        assert_eq!(flightroute_result["destination"]["iata_code"], "SJO");
        assert_eq!(flightroute_result["destination"]["icao_code"], "MROC");
        assert_eq!(flightroute_result["destination"]["latitude"], 9.993_86);
        assert_eq!(flightroute_result["destination"]["longitude"], -84.208801);
        assert_eq!(
            flightroute_result["destination"]["municipality"],
            "San José (Alajuela)"
        );
        assert_eq!(
            flightroute_result["destination"]["name"],
            "Juan Santamaría International Airport"
        );
    }

    #[tokio::test]
    async fn http_mod_get_aircraft_and_iata_callsign() {
        start_server().await;
        let mode_s = "A6D27B";
        let callsign = "AC959";
        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}?callsign={}",
            API_VERSION.as_str(),
            mode_s,
            callsign
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        let aircraft_result = result.get("aircraft").unwrap();

        assert_eq!(aircraft_result["icao_type"], "CRJ7");
        assert_eq!(aircraft_result["manufacturer"], "Bombardier");
        assert_eq!(aircraft_result["mode_s"], mode_s);
        assert_eq!(aircraft_result["registration"], "N539GJ");
        assert_eq!(aircraft_result["registered_owner"], "United Express");
        assert_eq!(aircraft_result["registered_owner_country_iso_name"], "US");
        assert_eq!(
            aircraft_result["registered_owner_country_name"],
            "United States"
        );
        assert_eq!(
            aircraft_result["registered_owner_operator_flag_code"],
            "GJS"
        );
        assert_eq!(aircraft_result["type"], "CRJ 700 702");

        let flightroute_result = result.get("flightroute");
        assert!(flightroute_result.is_some());
        let flightroute_result = flightroute_result.unwrap();
        assert_eq!(flightroute_result["callsign"], callsign.to_uppercase());
        assert_eq!(flightroute_result["callsign_iata"], callsign);
        assert_eq!(flightroute_result["callsign_icao"], "ACA959".to_uppercase());

        assert_eq!(flightroute_result["airline"]["name"], "Air Canada");
        assert_eq!(flightroute_result["airline"]["icao"], "ACA");
        assert_eq!(flightroute_result["airline"]["iata"], "AC");
        assert_eq!(flightroute_result["airline"]["callsign"], "AIR CANADA");
        assert_eq!(flightroute_result["airline"]["country"], "Canada");
        assert_eq!(flightroute_result["airline"]["country_iso"], "CA");

        assert_eq!(flightroute_result["origin"]["country_name"], "Canada");
        assert_eq!(flightroute_result["origin"]["elevation"], 118);
        assert_eq!(flightroute_result["origin"]["country_iso_name"], "CA");
        assert_eq!(flightroute_result["origin"]["iata_code"], "YUL");
        assert_eq!(flightroute_result["origin"]["icao_code"], "CYUL");
        assert_eq!(flightroute_result["origin"]["latitude"], 45.470_600_128_2);
        assert_eq!(flightroute_result["origin"]["longitude"], -73.740_798_950_2,);
        assert_eq!(flightroute_result["origin"]["municipality"], "Montréal");
        assert_eq!(
            flightroute_result["origin"]["name"],
            "Montreal / Pierre Elliott Trudeau International Airport"
        );

        assert!(result.get("midpoint").is_none());

        assert_eq!(flightroute_result["destination"]["country_iso_name"], "CR");
        assert_eq!(
            flightroute_result["destination"]["country_name"],
            "Costa Rica"
        );
        assert_eq!(flightroute_result["destination"]["elevation"], 3021);
        assert_eq!(flightroute_result["destination"]["iata_code"], "SJO");
        assert_eq!(flightroute_result["destination"]["icao_code"], "MROC");
        assert_eq!(flightroute_result["destination"]["latitude"], 9.993_86);
        assert_eq!(flightroute_result["destination"]["longitude"], -84.208801);
        assert_eq!(
            flightroute_result["destination"]["municipality"],
            "San José (Alajuela)"
        );
        assert_eq!(
            flightroute_result["destination"]["name"],
            "Juan Santamaría International Airport"
        );
    }

    #[tokio::test]
    async fn http_mod_get_aircraft_and_midpoint_icao_callsign() {
        start_server().await;
        let mode_s = "A6D27B";
        let callsign = "QFA31";
        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}?callsign={}",
            API_VERSION.as_str(),
            mode_s,
            callsign
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        let aircraft_result = result.get("aircraft").unwrap();

        assert_eq!(aircraft_result["icao_type"], "CRJ7");
        assert_eq!(aircraft_result["manufacturer"], "Bombardier");
        assert_eq!(aircraft_result["mode_s"], mode_s);
        assert_eq!(aircraft_result["registration"], "N539GJ");
        assert_eq!(aircraft_result["registered_owner"], "United Express");
        assert_eq!(aircraft_result["registered_owner_country_iso_name"], "US");
        assert_eq!(
            aircraft_result["registered_owner_country_name"],
            "United States"
        );
        assert_eq!(
            aircraft_result["registered_owner_operator_flag_code"],
            "GJS"
        );
        assert_eq!(aircraft_result["type"], "CRJ 700 702");

        let flightroute_result = result.get("flightroute");
        assert!(flightroute_result.is_some());
        let flightroute_result = flightroute_result.unwrap();

        assert_eq!(flightroute_result["airline"]["name"], "Qantas");
        assert_eq!(flightroute_result["airline"]["icao"], "QFA");
        assert_eq!(flightroute_result["airline"]["iata"], "QF");
        assert_eq!(flightroute_result["airline"]["callsign"], "QANTAS");
        assert_eq!(flightroute_result["airline"]["country"], "Australia");
        assert_eq!(flightroute_result["airline"]["country_iso"], "AU");

        assert_eq!(flightroute_result["callsign"], callsign.to_uppercase());
        assert_eq!(flightroute_result["callsign_icao"], callsign);
        assert_eq!(flightroute_result["callsign_iata"], "QF31");
        assert_eq!(
            flightroute_result["origin"]["country_iso_name"],
            "AU".to_uppercase()
        );
        assert_eq!(flightroute_result["origin"]["country_name"], "Australia");
        assert_eq!(flightroute_result["origin"]["elevation"], 21);
        assert_eq!(flightroute_result["origin"]["iata_code"], "SYD");
        assert_eq!(flightroute_result["origin"]["icao_code"], "YSSY");
        assert_eq!(
            flightroute_result["origin"]["latitude"],
            -33.946_098_327_636_72
        );
        assert_eq!(
            flightroute_result["origin"]["longitude"],
            151.177_001_953_125
        );
        assert_eq!(flightroute_result["origin"]["municipality"], "Sydney");
        assert_eq!(
            flightroute_result["origin"]["name"],
            "Sydney Kingsford Smith International Airport"
        );

        assert_eq!(
            flightroute_result["midpoint"]["country_iso_name"],
            "SG".to_uppercase()
        );
        assert_eq!(flightroute_result["midpoint"]["country_name"], "Singapore");
        assert_eq!(flightroute_result["midpoint"]["elevation"], 22);
        assert_eq!(flightroute_result["midpoint"]["iata_code"], "SIN");
        assert_eq!(flightroute_result["midpoint"]["icao_code"], "WSSS");
        assert_eq!(flightroute_result["midpoint"]["latitude"], 1.35019);
        assert_eq!(flightroute_result["midpoint"]["longitude"], 103.994_003);
        assert_eq!(flightroute_result["midpoint"]["municipality"], "Singapore");
        assert_eq!(
            flightroute_result["midpoint"]["name"],
            "Singapore Changi Airport"
        );

        assert_eq!(flightroute_result["destination"]["country_iso_name"], "GB");
        assert_eq!(
            flightroute_result["destination"]["country_name"],
            "United Kingdom"
        );
        assert_eq!(flightroute_result["destination"]["elevation"], 83);
        assert_eq!(flightroute_result["destination"]["iata_code"], "LHR");
        assert_eq!(flightroute_result["destination"]["icao_code"], "EGLL");
        assert_eq!(flightroute_result["destination"]["latitude"], 51.4706);
        assert_eq!(flightroute_result["destination"]["longitude"], -0.461_941);
        assert_eq!(flightroute_result["destination"]["municipality"], "London");
        assert_eq!(
            flightroute_result["destination"]["name"],
            "London Heathrow Airport"
        );
    }

    #[tokio::test]
    async fn http_mod_get_aircraft_and_midpoint_iata_callsign() {
        start_server().await;
        let mode_s = "A6D27B";
        let callsign = "QF31";
        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}?callsign={}",
            API_VERSION.as_str(),
            mode_s,
            callsign
        );
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        let aircraft_result = result.get("aircraft").unwrap();

        assert_eq!(aircraft_result["icao_type"], "CRJ7");
        assert_eq!(aircraft_result["manufacturer"], "Bombardier");
        assert_eq!(aircraft_result["mode_s"], mode_s);
        assert_eq!(aircraft_result["registration"], "N539GJ");
        assert_eq!(aircraft_result["registered_owner"], "United Express");
        assert_eq!(aircraft_result["registered_owner_country_iso_name"], "US");
        assert_eq!(
            aircraft_result["registered_owner_country_name"],
            "United States"
        );
        assert_eq!(
            aircraft_result["registered_owner_operator_flag_code"],
            "GJS"
        );
        assert_eq!(aircraft_result["type"], "CRJ 700 702");

        let flightroute_result = result.get("flightroute");
        assert!(flightroute_result.is_some());
        let flightroute_result = flightroute_result.unwrap();

        assert_eq!(flightroute_result["airline"]["name"], "Qantas");
        assert_eq!(flightroute_result["airline"]["icao"], "QFA");
        assert_eq!(flightroute_result["airline"]["iata"], "QF");
        assert_eq!(flightroute_result["airline"]["callsign"], "QANTAS");
        assert_eq!(flightroute_result["airline"]["country"], "Australia");
        assert_eq!(flightroute_result["airline"]["country_iso"], "AU");

        assert_eq!(flightroute_result["callsign"], callsign.to_uppercase());
        assert_eq!(flightroute_result["callsign_iata"], callsign.to_uppercase());
        assert_eq!(flightroute_result["callsign_icao"], "QFA31");
        assert_eq!(
            flightroute_result["origin"]["country_iso_name"],
            "AU".to_uppercase()
        );
        assert_eq!(flightroute_result["origin"]["country_name"], "Australia");
        assert_eq!(flightroute_result["origin"]["elevation"], 21);
        assert_eq!(flightroute_result["origin"]["iata_code"], "SYD");
        assert_eq!(flightroute_result["origin"]["icao_code"], "YSSY");
        assert_eq!(
            flightroute_result["origin"]["latitude"],
            -33.946_098_327_636_72
        );
        assert_eq!(
            flightroute_result["origin"]["longitude"],
            151.177_001_953_125
        );
        assert_eq!(flightroute_result["origin"]["municipality"], "Sydney");
        assert_eq!(
            flightroute_result["origin"]["name"],
            "Sydney Kingsford Smith International Airport"
        );

        assert_eq!(
            flightroute_result["midpoint"]["country_iso_name"],
            "SG".to_uppercase()
        );
        assert_eq!(flightroute_result["midpoint"]["country_name"], "Singapore");
        assert_eq!(flightroute_result["midpoint"]["elevation"], 22);
        assert_eq!(flightroute_result["midpoint"]["iata_code"], "SIN");
        assert_eq!(flightroute_result["midpoint"]["icao_code"], "WSSS");
        assert_eq!(flightroute_result["midpoint"]["latitude"], 1.35019);
        assert_eq!(flightroute_result["midpoint"]["longitude"], 103.994_003);
        assert_eq!(flightroute_result["midpoint"]["municipality"], "Singapore");
        assert_eq!(
            flightroute_result["midpoint"]["name"],
            "Singapore Changi Airport"
        );

        assert_eq!(flightroute_result["destination"]["country_iso_name"], "GB");
        assert_eq!(
            flightroute_result["destination"]["country_name"],
            "United Kingdom"
        );
        assert_eq!(flightroute_result["destination"]["elevation"], 83);
        assert_eq!(flightroute_result["destination"]["iata_code"], "LHR");
        assert_eq!(flightroute_result["destination"]["icao_code"], "EGLL");
        assert_eq!(flightroute_result["destination"]["latitude"], 51.4706);
        assert_eq!(flightroute_result["destination"]["longitude"], -0.461_941);
        assert_eq!(flightroute_result["destination"]["municipality"], "London");
        assert_eq!(
            flightroute_result["destination"]["name"],
            "London Heathrow Airport"
        );
    }

    #[tokio::test]
    async fn http_mod_get_aircraft_unknown() {
        start_server().await;
        let mode_s = "ABABAB";
        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}",
            API_VERSION.as_str(),
            mode_s
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "unknown aircraft");
    }

    #[tokio::test]
    /// /aircraft/random, check response, check cache, clear cache then check against a /aircraft/{mode_s/registration} response
    async fn http_mod_get_aircraft_random() {
        let setup = start_server().await;
        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/random",
            API_VERSION.as_str(),
        );
        let response = CLIENT.get(url).send().await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let response = response.json::<TestResponseValue>().await.unwrap();

        let aircraft_result = response
            .response
            .as_object()
            .unwrap()
            .get("aircraft")
            .unwrap();

        assert!(aircraft_result.get("icao_type").unwrap().is_string());
        assert!(aircraft_result.get("manufacturer").unwrap().is_string());
        assert!(aircraft_result.get("mode_s").unwrap().is_string());
        assert!(aircraft_result.get("registration").unwrap().is_string());
        assert!(aircraft_result.get("registered_owner").unwrap().is_string());
        assert!(
            aircraft_result
                .get("registered_owner_country_iso_name")
                .unwrap()
                .is_string()
        );

        assert!(
            aircraft_result
                .get("registered_owner_operator_flag_code")
                .is_some()
        );
        assert!(aircraft_result.get("url_photo").is_some());
        assert!(aircraft_result.get("url_photo_thumbnail").is_some());

        let mode_s = aircraft_result["mode_s"].as_str().unwrap().to_owned();

        let cache: Option<String> = setup
            .redis
            .hget(format!("mode_s::{mode_s}"), "data")
            .await
            .unwrap();
        assert!(cache.is_some());
        assert!(!cache.unwrap().is_empty());

        let registration = aircraft_result["registration"].as_str().unwrap().to_owned();
        let cache: Option<String> = setup
            .redis
            .hget(format!("registration::{registration}"), "data")
            .await
            .unwrap();
        assert!(cache.is_some());
        assert!(!cache.unwrap().is_empty());

        setup.flush_redis().await;

        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}",
            API_VERSION.as_str(),
            mode_s
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        let result = result.as_object().unwrap().get("aircraft").unwrap();

        assert_eq!(aircraft_result, result);

        setup.flush_redis().await;

        let url = format!(
            "http://127.0.0.1:8282{}/aircraft/{}",
            API_VERSION.as_str(),
            registration
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        let result = result.as_object().unwrap().get("aircraft").unwrap();

        assert_eq!(aircraft_result, result);
    }

    #[tokio::test]
    async fn http_mod_get_airline_ok() {
        start_server().await;
        let airline = "ba";
        let url = format!(
            "http://127.0.0.1:8282{}/airline/{}",
            API_VERSION.as_str(),
            airline
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        assert!(result.is_array());
        let result = result.as_array().unwrap();
        assert!(result.len() == 1);
        let result = result.first().unwrap();

        assert_eq!(
            result.get("name").unwrap().as_str().unwrap(),
            "British Airways"
        );
        assert_eq!(result.get("icao").unwrap().as_str().unwrap(), "BAW");
        assert_eq!(result.get("iata").unwrap().as_str().unwrap(), "BA");
        assert_eq!(
            result.get("country").unwrap().as_str().unwrap(),
            "United Kingdom"
        );
        assert_eq!(result.get("country_iso").unwrap().as_str().unwrap(), "GB");
        assert_eq!(
            result.get("callsign").unwrap().as_str().unwrap(),
            "SPEEDBIRD"
        );
    }

    #[tokio::test]
    // /airline/random, no cache
    async fn http_mod_get_airline_random() {
        start_server().await;
        let url = format!(
            "http://127.0.0.1:8282{}/airline/random",
            API_VERSION.as_str(),
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        assert!(result.is_array());
        let result = result.as_array().unwrap();
        assert!(result.len() == 1);
        let result = result.first().unwrap();

        assert!(result.get("name").unwrap().is_string());
        assert!(result.get("icao").unwrap().is_string());
        assert!(result.get("country").unwrap().is_string());
        assert!(result.get("country_iso").unwrap().is_string());
        assert!(result.get("iata").is_some());
        assert!(result.get("callsign").is_some());
    }

    #[tokio::test]
    async fn http_mod_get_airline_err() {
        start_server().await;
        let airline = "kn";
        let url = format!(
            "http://127.0.0.1:8282{}/airline/{}",
            API_VERSION.as_str(),
            airline
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "unknown airline");
    }

    #[tokio::test]
    async fn http_mod_get_n_number_ok() {
        start_server().await;
        let n_number = "n1235f";
        let url = format!(
            "http://127.0.0.1:8282{}/n-number/{}",
            API_VERSION.as_str(),
            n_number
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "A061E4");
    }

    #[tokio::test]
    async fn http_mod_get_n_number_err() {
        start_server().await;
        let n_number = "a1235f";
        let url = format!(
            "http://127.0.0.1:8282{}/n-number/{}",
            API_VERSION.as_str(),
            n_number
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "invalid n_number: A1235F");
    }

    #[tokio::test]
    async fn http_mod_get_mode_s_ok() {
        start_server().await;
        let mode_s = "ACD2D3";
        let url = format!(
            "http://127.0.0.1:8282{}/mode-s/{}",
            API_VERSION.as_str(),
            mode_s
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "N925XJ");
    }

    #[tokio::test]
    async fn http_mod_get_mode_s_ok_empty() {
        start_server().await;
        let mode_s = "CCD2D3";
        let url = format!(
            "http://127.0.0.1:8282{}/mode-s/{}",
            API_VERSION.as_str(),
            mode_s
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn http_mod_get_mode_s_err() {
        start_server().await;
        let mode_s = "JCD2D3";
        let url = format!(
            "http://127.0.0.1:8282{}/mode-s/{}",
            API_VERSION.as_str(),
            mode_s
        );
        let resp = CLIENT.get(url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "invalid modeS: JCD2D3");
    }

    #[tokio::test]
    async fn http_mod_get_online() {
        start_server().await;
        let url = format!("http://127.0.0.1:8282{}/online", API_VERSION.as_str());
        sleep!();
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result["api_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(result["uptime"], 1);
    }

    #[tokio::test]
    // 404 response
    async fn http_mod_get_unknown() {
        start_server().await;
        let version = API_VERSION.as_str();

        let rand_route = "asdasjkaj9ahsddasdasd";
        let url = format!("http://127.0.0.1:8282{version}/{rand_route}");
        let resp = CLIENT.get(url).send().await.unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let result = resp.json::<TestResponseValue>().await.unwrap().response;

        assert_eq!(result, format!("unknown endpoint: {version}/{rand_route}"));
    }

    #[tokio::test]
    // Not rate limited, but rate limit points = number of requests, and ttl 60
    async fn http_mod_rate_limit() {
        let setup = start_server().await;

        let url = format!("http://127.0.0.1:8282{}/online", API_VERSION.as_str());
        for _ in 1..=45 {
            CLIENT.get(&url).send().await.unwrap();
        }

        let ttl: usize = setup.redis.ttl("ratelimit::127.0.0.1").await.unwrap();
        let count: usize = setup.redis.get("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(count, 45);
        assert_eq!(ttl, 60);
    }

    #[tokio::test]
    async fn http_mod_rate_limit_small() {
        let setup = start_server().await;

        let url = format!("http://127.0.0.1:8282{}/online", API_VERSION.as_str());
        for _ in 1..=511 {
            CLIENT.get(&url).send().await.unwrap();
        }

        // 512th request is fine
        let resp = CLIENT.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result["api_version"], env!("CARGO_PKG_VERSION"));
        assert!(result.get("uptime").is_some());

        // 512th+ request is rate limited
        let resp = CLIENT.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "rate limited for 60 seconds");

        let ttl: usize = setup.redis.ttl("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(ttl, 60);

        sleep!(1000);

        // TTL reduces by 1 after 1 second
        let ttl: usize = setup.redis.ttl("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(ttl, 59);
        sleep!(1000);

        // TTL doesn't get reset on further requwest
        let resp = CLIENT.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "rate limited for 58 seconds");
        let ttl: usize = setup.redis.ttl("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(ttl, 58);

        // points increased
        let points: usize = setup.redis.get("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(points, 514);
    }

    #[tokio::test]
    async fn http_mod_rate_limit_big() {
        let setup = start_server().await;

        let url = format!("http://127.0.0.1:8282{}/online", API_VERSION.as_str());
        for _ in 1..=1023 {
            CLIENT.get(&url).send().await.unwrap();
        }

        // 1023rd request is rate limited
        let resp = CLIENT.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        println!("{result:?}");
        let ans = ["rate limited for 60 seconds", "rate limited for 59 seconds"];
        assert!(ans.contains(&result.as_str().unwrap()));

        // 1024th + request is rate limited for 300 seconds
        let resp = CLIENT.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "rate limited for 300 seconds");

        let ttl: usize = setup.redis.ttl("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(ttl, 300);

        sleep!(1000);

        // TTL reduces by 1 after 1 second
        let ttl: usize = setup.redis.ttl("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(ttl, 299);

        // TTL is reset to 300 on one more request
        let resp = CLIENT.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponseValue>().await.unwrap().response;
        assert_eq!(result, "rate limited for 300 seconds");
        let ttl: usize = setup.redis.ttl("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(ttl, 300);

        // points increased
        let points: usize = setup.redis.get("ratelimit::127.0.0.1").await.unwrap();
        assert_eq!(points, 1026);
    }
}
