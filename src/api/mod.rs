use ::redis::aio::Connection;
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts, State},
    http::{HeaderMap, Request},
    middleware::{self, Next},
    response::Response,
    routing::get,
    Router,
};
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Instant,
};
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tracing::info;

mod api_routes;
mod app_error;
mod input;
mod response;

use crate::{
    db_redis::ratelimit::RateLimit,
    parse_env::AppEnv,
    scraper::{Scraper, ScraperThreadMap},
};
pub use app_error::{AppError, UnknownAC};
pub use input::{AircraftSearch, AirlineCode, Callsign, ModeS, NNumber, Registration, Validate};

const X_REAL_IP: &str = "x-real-ip";
const X_FORWARDED_FOR: &str = "x-forwarded-for";

pub struct ApplicationState {
    postgres: PgPool,
    redis: Arc<Mutex<Connection>>,
    uptime: Instant,
    url_prefix: String,
    scraper: Scraper,
    scraper_threads: Arc<Mutex<ScraperThreadMap>>,
}

impl ApplicationState {
    pub fn new(
        postgres: PgPool,
        redis: Arc<Mutex<Connection>>,
        app_env: &AppEnv,
        scraper_threads: Arc<Mutex<ScraperThreadMap>>,
    ) -> Self {
        Self {
            postgres,
            redis,
            uptime: Instant::now(),
            scraper: Scraper::new(app_env),
            url_prefix: app_env.url_photo_prefix.clone(),
            scraper_threads,
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
        .map_or(addr.0.ip(), |ip_addr| ip_addr)
}

/// Limit the users request based on ip address, using redis as mem store
async fn rate_limiting(
    State(state): State<Arc<ApplicationState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    let (mut parts, body) = req.into_parts();
    let addr = ConnectInfo::<SocketAddr>::from_request_parts(&mut parts, &state).await?;
    let ip = get_ip(&parts.headers, addr);
    RateLimit::new(ip).check(&state.redis).await?;
    Ok(next.run(Request::from_parts(parts, body)).await)
}

/// Create a /v[x] prefix for all api routes, where x is the current major version
fn get_api_version() -> String {
    format!(
        "/v{}",
        env!("CARGO_PKG_VERSION")
            .split('.')
            .take(1)
            .collect::<String>()
    )
}
#[macro_export]
macro_rules! define_routes {
    ($enum_name:ident, $base_path:expr, $($variant:ident => $route:expr),*) => {
        enum $enum_name {
            $($variant,)*
        }

        impl $enum_name {
            fn addr(&self) -> String {
                let route_name = match self {
                    $(Self::$variant => $route,)*
                };
                format!("{}{}", $base_path, route_name)
            }
        }
    };
}

define_routes!(
    Routes,
    "/",
    Aircraft => "aircraft/:mode_s",
    Airline => "airline/:airline",
    Callsign => "callsign/:callsign",
    Online => "online",
    NNumber => "n-number/:n-number",
    ModeS => "mode-s/:mode_s"

);

/// Get an useable axum address, from app_env:host+port
fn get_addr(app_env: &AppEnv) -> Result<SocketAddr, AppError> {
    match (app_env.api_host.clone(), app_env.api_port).to_socket_addrs() {
        Ok(i) => i.take(1).collect::<Vec<SocketAddr>>().get(0).map_or_else(
            || Err(AppError::Internal("No addr".to_string())),
            |addr| Ok(*addr),
        ),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

/// Serve the app!
pub async fn serve(
    app_env: AppEnv,
    postgres: PgPool,
    redis: Arc<Mutex<Connection>>,
) -> Result<(), AppError> {
    let scraper_threads = Arc::new(Mutex::new(ScraperThreadMap::new()));
    let application_state = Arc::new(ApplicationState::new(
        postgres,
        redis,
        &app_env,
        scraper_threads,
    ));

    let api_routes = Router::new()
        .route(&Routes::Aircraft.addr(), get(api_routes::aircraft_get))
        .route(&Routes::Airline.addr(), get(api_routes::airline_get))
        .route(&Routes::Callsign.addr(), get(api_routes::callsign_get))
        .route(&Routes::Online.addr(), get(api_routes::online_get))
        .route(&Routes::NNumber.addr(), get(api_routes::n_number_get))
        .route(&Routes::ModeS.addr(), get(api_routes::mode_s_get));

    let prefix = get_api_version();

    let cors = CorsLayer::new()
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_origin(Any);

    let app = Router::new()
        .nest(&prefix, api_routes)
        .fallback(api_routes::fallback)
        .with_state(Arc::clone(&application_state))
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
    info!("starting server @ {addr}{prefix}");

    match axum::serve(
        tokio::net::TcpListener::bind(&addr).await?,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(_) => Err(AppError::Internal("api_server".to_owned())),
    }
}

/// http tests - ran via actual requests to a (local) server
/// cargo watch -q -c -w src/ -x 'test http_mod -- --test-threads=1 --nocapture'
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
pub mod tests {
    use super::*;

    use crate::db_postgres;
    use crate::db_redis;
    use crate::parse_env;

    use redis::AsyncCommands;
    use reqwest::StatusCode;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use tokio::task::JoinHandle;

    pub struct TestSetup {
        pub handle: Option<JoinHandle<()>>,
        pub app_env: AppEnv,
        pub postgres: PgPool,
        pub redis: Arc<Mutex<Connection>>,
    }

    // Get basic api params, also flushes all redis keys
    pub async fn test_setup() -> TestSetup {
        let app_env = parse_env::AppEnv::get_env();
        let postgres = db_postgres::db_pool(&app_env).await.unwrap();
        let mut redis = db_redis::get_connection(&app_env).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<_, ()>(&mut redis)
            .await
            .unwrap();
        TestSetup {
            handle: None,
            app_env,
            postgres,
            redis: Arc::new(Mutex::new(redis)),
        }
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

        let redis = Arc::clone(&setup.redis);
        let postgres = setup.postgres.clone();
        let app_env = setup.app_env.clone();
        let handle = tokio::spawn(async {
            serve(app_env, postgres, redis).await.unwrap();
        });
        // just sleep to make sure the server is running - 1ms is enough
        sleep!(1);
        TestSetup {
            handle: Some(handle),
            app_env: setup.app_env,
            postgres: setup.postgres,
            redis: setup.redis,
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestResponse {
        response: Value,
    }

    #[test]
    fn http_mod_get_api_version() {
        let prefix = get_api_version();
        assert_eq!(prefix, "/v0".to_owned());
    }

    #[tokio::test]
    async fn http_mod_get_icao_callsign() {
        start_server().await;
        let callsign = "ACA959";
        let url = format!(
            "http://127.0.0.1:8100{}/callsign/{}",
            get_api_version(),
            callsign
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;
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
            "http://127.0.0.1:8100{}/callsign/{}",
            get_api_version(),
            callsign
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;
        let result = result.get("flightroute").unwrap();

        assert!(result.get("aircraft").is_none());

        assert_eq!(result["callsign"], callsign.to_uppercase());
        assert_eq!(result["callsign_icao"], "ACA959".to_owned());
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
            "http://127.0.0.1:8100{}/callsign/{}",
            get_api_version(),
            callsign
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;
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
            "http://127.0.0.1:8100{}/callsign/{}",
            get_api_version(),
            callsign
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;
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
            "http://127.0.0.1:8100{}/callsign/{}",
            get_api_version(),
            callsign
        );
        let response = reqwest::get(url).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let result = response.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "unknown callsign");
    }

    #[tokio::test]
    async fn http_mod_get_aircraft() {
        start_server().await;
        let mode_s = "A6D27B";
        let url = format!(
            "http://127.0.0.1:8100{}/aircraft/{}",
            get_api_version(),
            mode_s
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;

        assert!(result.get("flightroute").is_none());
        let result = result.get("aircraft").unwrap();

        assert_eq!(result["icao_type"], "CRJ7");
        assert_eq!(result["manufacturer"], "Bombardier");
        assert_eq!(result["mode_s"], mode_s);
        assert_eq!(result["registration"], "N539GJ");
        assert_eq!(result["registered_owner"], "United Express");
        assert_eq!(result["registered_owner_country_iso_name"], "US");
        assert_eq!(result["registered_owner_country_name"], "United States");
        assert_eq!(result["registered_owner_operator_flag_code"], "GJS");
        assert_eq!(result["type"], "CRJ 700 702");
        assert_eq!(result["url_photo"].to_string(), "null");
        assert_eq!(result["url_photo_thumbnail"].to_string(), "null");
    }

    #[tokio::test]
    async fn http_mod_get_aircraft_and_icao_callsign() {
        start_server().await;
        let mode_s = "A6D27B";
        let callsign = "ACA959";
        let url = format!(
            "http://127.0.0.1:8100{}/aircraft/{}?callsign={}",
            get_api_version(),
            mode_s,
            callsign
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;

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
            "http://127.0.0.1:8100{}/aircraft/{}?callsign={}",
            get_api_version(),
            mode_s,
            callsign
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;

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
            "http://127.0.0.1:8100{}/aircraft/{}?callsign={}",
            get_api_version(),
            mode_s,
            callsign
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;

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
            "http://127.0.0.1:8100{}/aircraft/{}?callsign={}",
            get_api_version(),
            mode_s,
            callsign
        );
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let result = resp.json::<TestResponse>().await.unwrap().response;

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
            "http://127.0.0.1:8100{}/aircraft/{}",
            get_api_version(),
            mode_s
        );
        let resp = reqwest::get(url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "unknown aircraft");
    }

    #[tokio::test]
    async fn http_mod_get_n_number_ok() {
        start_server().await;
        let n_number = "n1235f";
        let url = format!(
            "http://127.0.0.1:8100{}/n-number/{}",
            get_api_version(),
            n_number
        );
        let resp = reqwest::get(url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "A061E4");
    }

    #[tokio::test]
    async fn http_mod_get_n_number_err() {
        start_server().await;
        let n_number = "a1235f";
        let url = format!(
            "http://127.0.0.1:8100{}/n-number/{}",
            get_api_version(),
            n_number
        );
        let resp = reqwest::get(url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "invalid n_number: A1235F");
    }

    #[tokio::test]
    async fn http_mod_get_mode_s_ok() {
        start_server().await;
        let mode_s = "ACD2D3";
        let url = format!(
            "http://127.0.0.1:8100{}/mode-s/{}",
            get_api_version(),
            mode_s
        );
        let resp = reqwest::get(url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "N925XJ");
    }

    #[tokio::test]
    async fn http_mod_get_mode_s_ok_empty() {
        start_server().await;
        let mode_s = "CCD2D3";
        let url = format!(
            "http://127.0.0.1:8100{}/mode-s/{}",
            get_api_version(),
            mode_s
        );
        let resp = reqwest::get(url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn http_mod_get_mode_s_err() {
        start_server().await;
        let mode_s = "JCD2D3";
        let url = format!(
            "http://127.0.0.1:8100{}/mode-s/{}",
            get_api_version(),
            mode_s
        );
        let resp = reqwest::get(url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "invalid modeS: JCD2D3");
    }

    #[tokio::test]
    async fn http_mod_get_online() {
        start_server().await;
        let url = format!("http://127.0.0.1:8100{}/online", get_api_version());
        sleep!();
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result["api_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(result["uptime"], 1);
    }

    #[tokio::test]
    // 404 response
    async fn http_mod_get_unknown() {
        start_server().await;

        let version = get_api_version();
        let rand_route = "asdasjkaj9ahsddasdasd";
        let url = format!("http://127.0.0.1:8100{version}/{rand_route}");
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let result = resp.json::<TestResponse>().await.unwrap().response;

        assert_eq!(result, format!("unknown endpoint: {version}/{rand_route}"));
    }

    #[tokio::test]
    // Not rate limited, but rate limit points = number of requests, and ttl 60
    async fn http_mod_rate_limit() {
        let test_setup = start_server().await;

        let url = format!("http://127.0.0.1:8100{}/online", get_api_version());
        for _ in 1..=45 {
            reqwest::get(&url).await.unwrap();
        }

        let count: usize = test_setup
            .redis
            .lock()
            .await
            .get("ratelimit::127.0.0.1")
            .await
            .unwrap();
        let ttl: usize = test_setup
            .redis
            .lock()
            .await
            .ttl("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(count, 45);
        assert_eq!(ttl, 60);
    }

    #[tokio::test]
    async fn http_mod_rate_limit_small() {
        let setup = start_server().await;

        let url = format!("http://127.0.0.1:8100{}/online", get_api_version());
        for _ in 1..=511 {
            reqwest::get(&url).await.unwrap();
        }

        // 512th request is fine
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result["api_version"], env!("CARGO_PKG_VERSION"));
        assert!(result.get("uptime").is_some());

        // 512th+ request is rate limited
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "rate limited for 60 seconds");

        let ttl: usize = setup
            .redis
            .lock()
            .await
            .ttl("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(ttl, 60);

        sleep!(1000);

        // TTL reduces by 1 after 1 second
        let ttl: usize = setup
            .redis
            .lock()
            .await
            .ttl("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(ttl, 59);
		sleep!(1000);

		// TTL doesn't get reset on further requwest
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "rate limited for 58 seconds");
        let ttl: usize = setup
            .redis
            .lock()
            .await
            .ttl("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(ttl, 58);

        // points increased
        let points: usize = setup
            .redis
            .lock()
            .await
            .get("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(points, 514);

    }

    #[tokio::test]
    async fn http_mod_rate_limit_big() {
        let setup = start_server().await;

        let url = format!("http://127.0.0.1:8100{}/online", get_api_version());
        for _ in 1..=1023 {
            reqwest::get(&url).await.unwrap();
        }

        // 1023rd request is rate limited
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        let ans = ["rate limited for 60 seconds", "rate limited for 59 seconds"];
        assert!(ans.contains(&result.as_str().unwrap()));

        // 1024th + request is rate limited for 300 seconds
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "rate limited for 300 seconds");

        let ttl: usize = setup
            .redis
            .lock()
            .await
            .ttl("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(ttl, 300);

        sleep!(1000);

        // TTL reduces by 1 after 1 second
        let ttl: usize = setup
            .redis
            .lock()
            .await
            .ttl("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(ttl, 299);

        // TTL is reset to 300 on one more request
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "rate limited for 300 seconds");
        let ttl: usize = setup
            .redis
            .lock()
            .await
            .ttl("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(ttl, 300);

        // points increased
        let points: usize = setup
            .redis
            .lock()
            .await
            .get("ratelimit::127.0.0.1")
            .await
            .unwrap();
        assert_eq!(points, 1026);
    }
}
