use ::redis::{aio::Connection, RedisError};
use http_body::Limited;
use reqwest::Method;
use sqlx::PgPool;
use thiserror::Error;
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
};

use axum::{
    body::Body,
    extract::ConnectInfo,
    handler::Handler,
    http::{HeaderMap, Request},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Extension, Router,
};
use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs},
    num::ParseIntError,
    sync::Arc,
    time::Instant,
};
use tokio::{signal, sync::Mutex};
use tower::ServiceBuilder;
use tracing::{error, info};

mod api_routes;
mod input;
mod response;

use crate::{
    db_redis::{check_rate_limit, RedisKey},
    parse_env::AppEnv,
    scraper::Scrapper,
};
pub use input::{is_hex, ModeS, NNumber};

use self::response::ResponseJson;

#[derive(Clone)]
pub struct ApplicationState {
    postgres: PgPool,
    redis: Arc<Mutex<Connection>>,
    uptime: Instant,
    url_prefix: String,
    scraper: Scrapper,
}

impl ApplicationState {
    pub fn new(postgres: PgPool, redis: Arc<Mutex<Connection>>, app_env: &AppEnv) -> Self {
        Self {
            postgres,
            redis,
            uptime: Instant::now(),
            scraper: Scrapper::new(app_env),
            url_prefix: app_env.url_photo_prefix.clone(),
        }
    }
}

const X_REAL_IP: &str = "x-real-ip";
const X_FORWARDED_FOR: &str = "x-forwarded-for";

/// extract `x-real-ip` header
fn maybe_x_forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get(X_FORWARDED_FOR)
        .and_then(|x| x.to_str().ok())
        .and_then(|s| s.split(',').find_map(|s| s.trim().parse::<IpAddr>().ok()))
}

/// extract the `x-real-ip` header
fn maybe_x_real_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get(X_REAL_IP)
        .and_then(|x| x.to_str().ok())
        .and_then(|s| s.parse::<IpAddr>().ok())
}

/// Get a users ip address, application should always be behind an nginx reverse proxy
/// so header x-forwarded-for should always be valid, then try x-real-ip
/// if neither headers work, use the optional socket address from axum
/// but if for some nothing works, return ipv4 255.255.255.255
fn get_ip(headers: &HeaderMap, addr: Option<&ConnectInfo<SocketAddr>>) -> IpAddr {
    maybe_x_forwarded_for(headers)
        .or_else(|| maybe_x_real_ip(headers))
        .map_or_else(
            || {
                addr.map_or_else(
                    || IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)),
                    |ip| ip.0.ip(),
                )
            },
            |ip_addr| ip_addr,
        )
}

// Limit the users request based on ip address, using redis as mem store
async fn rate_limiting<B: Send + Sync>(
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, AppError> {
    let addr: Option<&ConnectInfo<SocketAddr>> = req.extensions().get();
    match req.extensions().get::<ApplicationState>() {
        Some(state) => {
            let ip = get_ip(req.headers(), addr);
            let rate_limit_key = RedisKey::RateLimit(ip);
            check_rate_limit(&state.redis, rate_limit_key).await?;
            Ok(next.run(req).await)
        }
        None => Err(AppError::Internal(
            "Unable to get application_state".to_owned(),
        )),
    }
}

/// Create a /v[x] prefix for all api routes, where x is the current major version
fn get_api_version() -> String {
    format!(
        "/v{}",
        env!("CARGO_PKG_VERSION")
            .chars()
            .take(1)
            .collect::<String>()
    )
}

enum Routes {
    Aircraft,
    Callsign,
    Online,
    NNumber,
    ModeS,
}

impl fmt::Display for Routes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let disp = match self {
            Self::Aircraft => "aircraft/:mode_s",
            Self::Callsign => "callsign/:callsign",
            Self::Online => "online",
            Self::NNumber => "n-number/:n-number",
            Self::ModeS => "mode-s/:mode_s",
        };
        write!(f, "/{}", disp)
    }
}

pub async fn serve(
    app_env: AppEnv,
    postgres: PgPool,
    redis: Arc<Mutex<Connection>>,
) -> Result<(), AppError> {
    let application_state = ApplicationState::new(postgres, redis, &app_env);

    let api_routes: Router<Limited<Body>> = Router::new()
        .route(&Routes::Aircraft.to_string(), get(api_routes::aircraft_get))
        .route(&Routes::Callsign.to_string(), get(api_routes::callsign_get))
        .route(&Routes::Online.to_string(), get(api_routes::online_get))
        .route(&Routes::NNumber.to_string(), get(api_routes::n_number_get))
        .route(&Routes::ModeS.to_string(), get(api_routes::mode_s_get));

    let prefix = get_api_version();

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any);

    let app = Router::new()
        .nest(&prefix, api_routes)
        .fallback(api_routes::fallback.into_service())
        .layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(1024))
                .layer(cors)
                .layer(Extension(application_state))
                .layer(middleware::from_fn(rate_limiting)),
        );

    let addr = match (app_env.api_host, app_env.api_port).to_socket_addrs() {
        Ok(i) => {
            let vec_i = i.take(1).collect::<Vec<SocketAddr>>();
            vec_i.get(0).map_or_else(
                || Err(AppError::Internal("No addr".to_string())),
                |addr| Ok(*addr),
            )
        }
        Err(e) => Err(AppError::Internal(e.to_string())),
    }?;

    let starting = format!("starting server @ {}", addr);
    info!(%starting);

    match axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err(AppError::Internal("api_server".to_owned())),
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
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown");
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid callsign:")]
    Callsign(String),
    #[error("invalid n_number:")]
    NNumber(String),
    #[error("internal error:")]
    Internal(String),
    #[error("invalid modeS:")]
    ModeS(String),
    #[error("not found")]
    SqlxError(#[from] sqlx::Error),
    #[error("redis error")]
    RedisError(#[from] RedisError),
    #[error("internal error")]
    SerdeJson(#[from] serde_json::Error),
    #[error("rate limited for")]
    RateLimited(usize),
    #[error("unknown")]
    UnknownInDb(&'static str),
    #[error("parse int")]
    ParseInt(#[from] ParseIntError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let prefix = self.to_string();
        let (status, body) = match self {
            Self::Callsign(err) | Self::NNumber(err) | Self::ModeS(err) => (
                axum::http::StatusCode::BAD_REQUEST,
                ResponseJson::new(format!("{} {}", prefix, err)),
            ),
            Self::Internal(err) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson::new(format!("{} {}", prefix, err)),
            ),
            Self::SqlxError(_) | Self::RedisError(_) => {
                (axum::http::StatusCode::NOT_FOUND, ResponseJson::new(prefix))
            }
            Self::SerdeJson(_) | Self::ParseInt(_) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson::new(prefix),
            ),
            Self::RateLimited(limit) => (
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                ResponseJson::new(format!("{} {} seconds", prefix, limit)),
            ),
            Self::UnknownInDb(variety) => (
                axum::http::StatusCode::NOT_FOUND,
                ResponseJson::new(format!("{} {}", prefix, variety)),
            ),
        };

        (status, body).into_response()
    }
}

/// http tests - ran via actual requests to a (local) server
/// cargo watch -q -c -w src/ -x 'test http_mod -- --test-threads=1 --nocapture'
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod tests {
    use super::*;

    use crate::db_postgres;
    use crate::db_redis;
    use crate::parse_env;

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

    async fn sleep(ms: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
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
        sleep(1).await;
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
    // test midpoint
    async fn http_mod_get_callsign() {
        start_server().await;
        let callsign = "TOM35MR";
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
        assert_eq!(result["origin"]["country_iso_name"], "ES".to_uppercase());
        assert_eq!(result["origin"]["country_name"], "Spain");
        assert_eq!(result["origin"]["elevation"], 27);
        assert_eq!(result["origin"]["country_iso_name"], "ES");
        assert_eq!(result["origin"]["country_name"], "Spain");
        assert_eq!(result["origin"]["elevation"], 27);
        assert_eq!(result["origin"]["iata_code"], "PMI");
        assert_eq!(result["origin"]["icao_code"], "LEPA");
        assert_eq!(result["origin"]["latitude"], 39.551_701);
        assert_eq!(result["origin"]["longitude"], 2.738_81);
        assert_eq!(result["origin"]["municipality"], "Palma De Mallorca");
        assert_eq!(result["origin"]["name"], "Palma de Mallorca Airport");

        assert!(result.get("midpoint").is_none());

        assert_eq!(result["destination"]["country_iso_name"], "GB");
        assert_eq!(result["destination"]["country_name"], "United Kingdom");
        assert_eq!(result["destination"]["elevation"], 622);
        assert_eq!(result["destination"]["iata_code"], "BRS");
        assert_eq!(result["destination"]["icao_code"], "EGGD");
        assert_eq!(result["destination"]["latitude"], 51.382_702);
        assert_eq!(result["destination"]["longitude"], -2.719_09);
        assert_eq!(result["destination"]["municipality"], "Bristol");
        assert_eq!(result["destination"]["name"], "Bristol Airport");
    }

    #[tokio::test]
    async fn http_mod_get_callsign_with_midpoint() {
        start_server().await;
        let callsign = "QFA031";
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
        assert_eq!(result["n_number"], "N539GJ");
        assert_eq!(result["registered_owner"], "United Express");
        assert_eq!(result["registered_owner_country_iso_name"], "US");
        assert_eq!(result["registered_owner_country_name"], "United States");
        assert_eq!(result["registered_owner_operator_flag_code"], "GJS");
        assert_eq!(result["type"], "CRJ 700 702");
        assert_eq!(result["url_photo"].to_string(), "null");
        assert_eq!(result["url_photo_thumbnail"].to_string(), "null");
    }

    #[tokio::test]
    async fn http_mod_get_aircraft_and_callsign() {
        start_server().await;
        let mode_s = "A6D27B";
        let callsign = "TOM35MR";
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
        assert_eq!(aircraft_result["n_number"], "N539GJ");
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
        assert_eq!(
            flightroute_result["origin"]["country_iso_name"],
            "ES".to_uppercase()
        );
        assert_eq!(flightroute_result["origin"]["country_name"], "Spain");
        assert_eq!(flightroute_result["origin"]["elevation"], 27);
        assert_eq!(flightroute_result["origin"]["country_iso_name"], "ES");
        assert_eq!(flightroute_result["origin"]["country_name"], "Spain");
        assert_eq!(flightroute_result["origin"]["elevation"], 27);
        assert_eq!(flightroute_result["origin"]["iata_code"], "PMI");
        assert_eq!(flightroute_result["origin"]["icao_code"], "LEPA");
        assert_eq!(flightroute_result["origin"]["latitude"], 39.551_701);
        assert_eq!(flightroute_result["origin"]["longitude"], 2.73881);
        assert_eq!(
            flightroute_result["origin"]["municipality"],
            "Palma De Mallorca"
        );
        assert_eq!(
            flightroute_result["origin"]["name"],
            "Palma de Mallorca Airport"
        );

        assert!(result.get("midpoint").is_none());

        assert_eq!(flightroute_result["destination"]["country_iso_name"], "GB");
        assert_eq!(
            flightroute_result["destination"]["country_name"],
            "United Kingdom"
        );
        assert_eq!(flightroute_result["destination"]["elevation"], 622);
        assert_eq!(flightroute_result["destination"]["iata_code"], "BRS");
        assert_eq!(flightroute_result["destination"]["icao_code"], "EGGD");
        assert_eq!(flightroute_result["destination"]["latitude"], 51.382_702);
        assert_eq!(flightroute_result["destination"]["longitude"], -2.71909);
        assert_eq!(flightroute_result["destination"]["municipality"], "Bristol");
        assert_eq!(flightroute_result["destination"]["name"], "Bristol Airport");
    }

    #[tokio::test]
    async fn http_mod_get_aircraft_and_midpoint_callsign() {
        start_server().await;
        let mode_s = "A6D27B";
        let callsign = "QFA031";
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
        assert_eq!(aircraft_result["n_number"], "N539GJ");
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
        assert_eq!(flightroute_result["destination"]["longitude"], -0.461941);
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
        sleep(1000).await;
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
        let url = format!("http://127.0.0.1:8100{}/{}", version, rand_route);
        let resp = reqwest::get(url).await.unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let result = resp.json::<TestResponse>().await.unwrap().response;

        assert_eq!(
            result,
            format!("unknown endpoint: {}/{}", version, rand_route)
        );
    }

    #[tokio::test]
    async fn http_mod_rate_limit_small() {
        start_server().await;

        let url = format!("http://127.0.0.1:8100{}/online", get_api_version());
        for _ in 0..=118 {
            reqwest::get(&url).await.unwrap();
        }

        // 119 request is fine
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result["api_version"], env!("CARGO_PKG_VERSION"));
        assert!(result.get("uptime").is_some());

        // 120+ request is rate limited
        let resp = reqwest::get(url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "rate limited for 60 seconds");
    }

    #[tokio::test]
    async fn http_mod_rate_limit_big() {
        start_server().await;

        let url = format!("http://127.0.0.1:8100{}/online", get_api_version());
        for _ in 0..=238 {
            reqwest::get(&url).await.unwrap();
        }

        // 239th request is rate limited
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "rate limited for 59 seconds");

        // 240+ request is rate limited for 300 seconds
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let result = resp.json::<TestResponse>().await.unwrap().response;
        assert_eq!(result, "rate limited for 300 seconds");
    }
}
