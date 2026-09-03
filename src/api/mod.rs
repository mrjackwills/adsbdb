use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::Request,
    middleware::{self, Next},
    response::Response,
    routing::{get, patch},
};
use crossfire::AsyncTxTrait;
use fred::clients::Pool;
use router_get::ApiRoutes;
use sqlx::PgPool;
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::LazyLock,
    time::Instant,
};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

mod app_error;
mod input;
mod response;
mod router_get;
mod router_patch;

use crate::{
    S, Tx,
    db::{
        postgres::{MsgIncomingRequest, UriMethod},
        redis::ratelimit::RateLimit,
    },
    parse_env::AppEnv,
    scraper::MsgScraper,
};
pub use app_error::*;
pub use input::{AircraftSearch, AirlineCode, Callsign, ModeS, NNumber, Registration, Validate};
pub use response::{ResponseAircraft, Stats, StatsEntry};

const X_REAL_IP: &str = "x-real-ip";
const X_FORWARDED_FOR: &str = "x-forwarded-for";
const STATS_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(5);

#[derive(Debug, Clone)]
pub struct PhotoPrefixes {
    pub photo: String,
    pub thumbnail: String,
}

impl From<&AppEnv> for PhotoPrefixes {
    fn from(value: &AppEnv) -> Self {
        Self {
            photo: value.url_photo_prefix.clone(),
            thumbnail: value.url_photo_thumbnail_prefix.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ApplicationState {
    postgres: PgPool,
    redis: Pool,
    uptime: Instant,
    scraper_tx: Tx<MsgScraper>,
    stats_tx: Tx<MsgIncomingRequest>,
    photo_prefixes: PhotoPrefixes,
}

impl ApplicationState {
    pub fn new(
        app_env: &AppEnv,
        postgres: PgPool,
        redis: Pool,
        scraper_tx: Tx<MsgScraper>,
        stats_tx: Tx<MsgIncomingRequest>,
    ) -> Self {
        Self {
            postgres,
            redis,
            uptime: Instant::now(),
            scraper_tx,
            stats_tx,
            photo_prefixes: PhotoPrefixes::from(app_env),
        }
    }
}

// todo imple me
// pub struct Api;

/// Get a users ip address, application should always be behind an nginx reverse proxy
/// so header x-forwarded-for should always be valid, but if not, then try x-real-ip
/// if neither headers work, use the optional socket address from axum
fn get_ip(req: &Request<Body>) -> Result<IpAddr, AppError> {
    let headers = req.headers();

    let from_header = |name| headers.get(name).and_then(|v| v.to_str().ok());

    let parse_ip = |s: &str| s.trim().parse::<IpAddr>().ok();

    from_header(X_REAL_IP)
        .and_then(parse_ip)
        .or_else(|| from_header(X_FORWARDED_FOR).and_then(|s| s.split(',').find_map(parse_ip)))
        .or_else(|| {
            req.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.ip())
        })
        .ok_or_else(|| AppError::Internal(S!("IP error")))
}

async fn insert_stats(
    State(state): State<ApplicationState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if let Err(e) = state
        .stats_tx
        .send_timeout(
            MsgIncomingRequest::Url(UriMethod::from(&req)),
            STATS_TIMEOUT,
        )
        .await
    {
        tracing::error!("{e}");
    }
    next.run(req).await
}

/// Limit the users request based on ip address, using redis as mem store
async fn rate_limiting(
    State(state): State<ApplicationState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    RateLimit::new(get_ip(&req)?).check(&state.redis).await?;
    Ok(next.run(req).await)
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
    tx_scraper: Tx<MsgScraper>,
    tx_stats: Tx<MsgIncomingRequest>,
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
                patch(router_patch::callsign_patch).layer(middleware::from_fn_with_state(
                    update_hash.clone(),
                    router_patch::auth_header,
                )),
            )
            .route(
                &Routes::Aircraft.addr(),
                patch(router_patch::aircraft_patch).layer(middleware::from_fn_with_state(
                    update_hash.clone(),
                    router_patch::auth_header,
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
                    application_state.clone(),
                    insert_stats,
                ))
                .layer(middleware::from_fn_with_state(
                    application_state,
                    rate_limiting,
                )),
        );

    let addr = get_addr(&app_env)?;
    tracing::info!("starting server @ {addr}{}", API_VERSION.as_str());
    tracing::info!(
        "scrape_flightroute: {}, scrape_photo: {}",
        app_env.allow_scrape_flightroute.is_some(),
        app_env.allow_scrape_photo.is_some()
    );
    tracing::info!("updater: {}", app_env.allow_update.is_some());

    axum::serve(
        tokio::net::TcpListener::bind(&addr).await?,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|_| AppError::Internal(S!("api_server")))
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

    use std::collections::HashMap;

    use super::*;

    use crate::api::response::{
        AircraftAndRoute, Airport, Online, ResponseAirline, ResponseFlightRoute, ResponseJson,
    };
    use crate::db::postgres::{ModelAircraft, ModelFlightroute};
    use crate::db::redis::HASH_FIELD;
    use crate::parse_env;
    use crate::scraper::Scraper;
    use crate::start_incoming_requests;
    use crate::{
        db::postgres,
        db::redis::{self, RedisKey},
    };

    use axum::{extract::Query, http::status};
    use fred::{
        interfaces::{ClientLike, KeysInterface},
        prelude::HashesInterface,
    };
    use rand::random_range;
    use reqwest::StatusCode;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use tokio::task::JoinHandle;

    /// Sleep for a given number of milliseconds, is an async fn.
    /// If no parameter supplied, defaults to 1000ms
    #[macro_export]
    macro_rules! sleep {
        () => {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        };
        ($ms:expr) => {
            tokio::time::sleep(std::time::Duration::from_millis($ms)).await;
        };
    }

    const RATE_LIMIT_KEY: &str = "ratelimit::127.0.0.1";

    /// This is a valid mode_s, but has no mathcin n_number
    pub const VALID_MODE_S_NO_N_NUMBER: &str = "CCD2D3";

    /// Empty query hashmap for aircraft searches
    pub static EMPTY_QUERY: LazyLock<Query<HashMap<String, String>>> =
        LazyLock::new(|| Query(HashMap::new()));

    /// These are valid callsigns but not connected to any flight
    pub const UNKNOWN_CALLSIGN_ICAO: &str = "ZZZ999";
    pub const UNKNOWN_CALLSIGN_IATA: &str = "ZZ999";
    pub const UNKNOWN_CALLSIGN_OTHER: &str = "NZZ999";

    /// These are valid airlines but not connected to any airline
    pub const UNKNOWN_AIRLINE_ICAO: &str = "Z99";
    pub const UNKNOWN_AIRLINE_IATA: &str = "Z9";

    /// These are valid aircraft but not connected to any aircraft
    pub const UNKNOWN_AIRCRAFT_MODE_S: &str = "000000";
    pub const UNKNOWN_AIRCRAFT_REGISTRATION: &str = "00-000";

    pub const AIRCRAFT_MODE_S: &str = "ADC1F0";

    pub static DEFAULT_AIRCRAFT_PATH: LazyLock<String> =
        LazyLock::new(|| format!("aircraft/{AIRCRAFT_MODE_S}"));

    pub static DEFAULT_CALLSIGN_PATH: LazyLock<String> =
        LazyLock::new(|| format!("callsign/{}", FLIGHTROUTE.callsign));

    /// This is an aircraft in the DB with no photo attached
    pub static AIRCRAFT_NO_PHOTO: LazyLock<ResponseAircraft> = LazyLock::new(|| ResponseAircraft {
        aircraft_type: S!("208 B"),
        icao_type: S!("C208"),
        manufacturer: S!("Cessna"),
        mode_s: S!("ADC1F0"),
        registration: S!("N986FE"),
        registered_owner: S!("FedEx Feeder"),
        registered_owner_operator_flag_code: Some(S!("IRO")),
        registered_owner_country_name: S!("United States"),
        registered_owner_country_iso_name: S!("US"),
        url_photo: None,
        url_photo_thumbnail: None,
    });

    /// This is an aircraft in the DB with a photo attached
    pub fn gen_aircraft_with_photo(pp: &PhotoPrefixes) -> ResponseAircraft {
        ResponseAircraft {
            aircraft_type: S!("Citation Sovereign"),
            icao_type: S!("C680"),
            manufacturer: S!("Cessna"),
            mode_s: S!("A44F3B"),
            registration: S!("N377QS"),
            registered_owner: S!("NetJets"),
            registered_owner_operator_flag_code: Some(S!("EJA")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
            url_photo: Some(format!("{}{}", pp.photo, "001572354.jpg")),
            url_photo_thumbnail: Some(format!("{}{}", pp.thumbnail, "001/572/001572354.jpg")),
        }
    }

    pub const VALID_AUTH_HEADER: (&str, &str) = ("Authorization", "password123");

    pub static KNOWN_AIRLINE: LazyLock<ResponseAirline> = LazyLock::new(|| ResponseAirline {
        name: S!("Air Canada"),
        icao: S!("ACA"),
        iata: Some(S!("AC")),
        country: S!("Canada"),
        country_iso: S!("CA"),
        callsign: Some(S!("AIR CANADA")),
    });

    pub static FLIGHTROUTE: LazyLock<ResponseFlightRoute> = LazyLock::new(|| ResponseFlightRoute {
        callsign: S!("ACA959"),
        callsign_icao: Some(S!("ACA959")),
        callsign_iata: Some(S!("AC959")),
        airline: Some(KNOWN_AIRLINE.clone()),
        origin: Airport {
            country_iso_name: S!("CA"),
            country_name: S!("Canada"),
            elevation: 118,
            iata_code: Some(S!("YUL")),
            icao_code: S!("CYUL"),
            latitude: 45.4706,
            longitude: -73.7408,
            municipality: S!("Montréal"),
            name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
        },
        midpoint: None,
        destination: Airport {
            country_iso_name: S!("CR"),
            country_name: S!("Costa Rica"),
            elevation: 3021,
            iata_code: Some(S!("SJO")),
            icao_code: S!("MROC"),
            latitude: 9.99386,
            longitude: -84.2088,
            municipality: S!("San José (Alajuela)"),
            name: S!("Juan Santamaría International Airport"),
        },
    });

    pub static FLIGHTROUTE_OTHER: LazyLock<ResponseFlightRoute> =
        LazyLock::new(|| ResponseFlightRoute {
            callsign: S!("N154CA"),
            callsign_icao: None,
            callsign_iata: None,
            airline: None,
            origin: Airport {
                country_iso_name: S!("US"),
                country_name: S!("United States"),
                elevation: 173,
                iata_code: Some(S!("BDL")),
                icao_code: S!("KBDL"),
                latitude: 41.93851,
                longitude: -72.68807,
                municipality: S!("Hartford"),
                name: S!("Bradley International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("US"),
                country_name: S!("United States"),
                elevation: 822,
                iata_code: None,
                icao_code: S!("KHFY"),
                latitude: 39.6284,
                longitude: -86.0879,
                municipality: S!("Indianapolis"),
                name: S!("Indy South Greenwood Airport"),
            },
        });

    /// Generate a flightroute with a midpoint
    pub static FLIGHTROUTE_MIDPOINT: LazyLock<ResponseFlightRoute> =
        LazyLock::new(|| ResponseFlightRoute {
            callsign: S!("QFA31"),
            callsign_iata: Some(S!("QF31")),
            callsign_icao: Some(S!("QFA31")),
            airline: Some(ResponseAirline {
                name: S!("Qantas"),
                icao: S!("QFA"),
                iata: Some(S!("QF")),
                callsign: Some(S!("QANTAS")),
                country: S!("Australia"),
                country_iso: S!("AU"),
            }),
            origin: Airport {
                country_iso_name: S!("AU"),
                country_name: S!("Australia"),
                elevation: 21,
                iata_code: Some(S!("SYD")),
                icao_code: S!("YSSY"),
                latitude: -33.9461,
                longitude: 151.177,
                municipality: S!("Sydney"),
                name: S!("Sydney Kingsford Smith International Airport"),
            },
            midpoint: Some(Airport {
                country_iso_name: S!("SG"),
                country_name: S!("Singapore"),
                elevation: 22,
                iata_code: Some(S!("SIN")),
                icao_code: S!("WSSS"),
                latitude: 1.35019,
                longitude: 103.994,
                municipality: S!("Singapore"),
                name: S!("Singapore Changi Airport"),
            }),
            destination: Airport {
                country_iso_name: S!("GB"),
                country_name: S!("United Kingdom"),
                elevation: 83,
                iata_code: Some(S!("LHR")),
                icao_code: S!("EGLL"),
                latitude: 51.4706,
                longitude: -0.46194,
                municipality: S!("London"),
                name: S!("London Heathrow Airport"),
            },
        });

    fn gen_ttl_msg(x: u16) -> String {
        format!("rate limited for {} seconds", x)
    }
    /// Get the ratelimit ttl for RATE_LIMIT_KEY
    async fn get_rate_limit_ttl(redis: &Pool) -> usize {
        redis.ttl(RATE_LIMIT_KEY).await.unwrap()
    }

    /// Get the ratelimit points count for RATE_LIMIT_KEY
    async fn get_rate_limit_count(redis: &Pool) -> usize {
        redis.get(RATE_LIMIT_KEY).await.unwrap()
    }

    pub async fn send_patch_request<T: Serialize>(
        path: &str,
        headers: Option<(&str, &str)>,
        body: Option<T>,
    ) -> reqwest::Response {
        let mut request = CLIENT.patch(format!(
            "http://127.0.0.1:8282{}/{path}",
            API_VERSION.as_str()
        ));

        if let Some(headers) = headers {
            request = request.header(headers.0, headers.1);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await;
        assert!(response.is_ok());
        response.unwrap()
    }

    pub async fn cache_aircraft_exists(redis: &Pool) -> bool {
        redis
            .hexists(
                RedisKey::ModeS(&ModeS::validate(AIRCRAFT_MODE_S).unwrap()).to_string(),
                HASH_FIELD,
            )
            .await
            .unwrap()
            && redis
                .hexists(
                    RedisKey::Registration(
                        &Registration::validate(&AIRCRAFT_NO_PHOTO.registration).unwrap(),
                    )
                    .to_string(),
                    HASH_FIELD,
                )
                .await
                .unwrap()
    }

    pub async fn cache_callsign_exists(redis: &Pool) -> bool {
        let flightroute = FLIGHTROUTE.clone();
        redis
            .hexists(
                RedisKey::Callsign(&Callsign::validate(&flightroute.callsign).unwrap()).to_string(),
                HASH_FIELD,
            )
            .await
            .unwrap()
            && redis
                .hexists(
                    RedisKey::Callsign(
                        &Callsign::validate(&flightroute.callsign_iata.unwrap()).unwrap(),
                    )
                    .to_string(),
                    HASH_FIELD,
                )
                .await
                .unwrap()
    }

    /// Return the aircraft details from redis
    pub async fn get_aircraft_cache(redis: &Pool) -> ModelAircraft {
        redis
            .hget(
                RedisKey::ModeS(&ModeS::validate(AIRCRAFT_MODE_S).unwrap()).to_string(),
                HASH_FIELD,
            )
            .await
            .unwrap()
    }

    /// Return the aircraft details from redis
    pub async fn get_callsign_cache(redis: &Pool) -> ModelFlightroute {
        redis
            .hget(
                RedisKey::Callsign(&Callsign::validate(&FLIGHTROUTE.callsign).unwrap()).to_string(),
                HASH_FIELD,
            )
            .await
            .unwrap()
    }

    /// Reset aircraft details back to original
    pub async fn reset_aircraft() {
        send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(VALID_AUTH_HEADER),
            // AIRCRAFT_NO_PHOTO
            Some(AIRCRAFT_NO_PHOTO.clone()),
        )
        .await;
    }

    /// Reset callsign back to original
    pub async fn reset_callsign() {
        send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([
                ("origin", FLIGHTROUTE.origin.icao_code.as_str()),
                ("destination", FLIGHTROUTE.destination.icao_code.as_str()),
            ])),
        )
        .await;
    }

    /// Generate a ranomd hex string of length x
    pub fn random_hex(x: usize) -> String {
        const HEX_CHARS: &[u8] = b"0123456789ABCDEF";
        (0..x)
            .map(|_| HEX_CHARS[random_range(0..HEX_CHARS.len())] as char)
            .collect()
    }
    /// Reqwest client
    pub static CLIENT: LazyLock<reqwest::Client> =
        LazyLock::new(|| reqwest::ClientBuilder::new().build().unwrap());

    pub struct TestSetup {
        pub _handle: Option<JoinHandle<()>>,
        pub app_env: AppEnv,
        pub postgres: PgPool,
        pub redis: Pool,
        pub photo_prefixes: PhotoPrefixes,
    }

    impl TestSetup {
        pub async fn flush_redis(&self) {
            self.redis.flushall::<()>(true).await.unwrap();
        }
    }

    // Get basic api params, also flushes all redis keys
    pub async fn test_setup() -> TestSetup {
        let app_env = parse_env::AppEnv::get_env();
        let (postgres, redis) =
            tokio::try_join!(postgres::get_pool(&app_env), redis::get_pool(&app_env)).unwrap();
        let photo_prefixes = PhotoPrefixes::from(&app_env);
        let setup = TestSetup {
            _handle: None,
            app_env,
            postgres,
            redis,
            photo_prefixes,
        };
        setup.flush_redis().await;
        setup
    }

    /// Start the server, so can test over http, with all layers applied etc
    pub async fn start_server(allow_update: Option<()>) -> TestSetup {
        let setup = test_setup().await;
        let mut app_env = setup.app_env.clone();

        if allow_update.is_none() {
            app_env.allow_update = None;
        }

        let postgres = setup.postgres.clone();
        // let app_env = setup.app_env.clone();
        let redis = setup.redis.clone();
        setup.flush_redis().await;

        // need to set up scrapers here
        let tx_scraper = Scraper::start(&app_env, &postgres);
        let tx_stats = start_incoming_requests(&postgres, &redis).await.unwrap();
        let handle = tokio::spawn(async {
            serve(app_env, postgres, redis, tx_scraper, tx_stats)
                .await
                .unwrap();
        });
        // just sleep to make sure the server is running - 1ms is enough
        // Will seed that stats in redis, so now need 100ms
        sleep!(100);
        let photo_prefixes = PhotoPrefixes::from(&setup.app_env);
        TestSetup {
            _handle: Some(handle),
            app_env: setup.app_env,
            postgres: setup.postgres,
            redis: setup.redis,
            photo_prefixes,
        }
    }

    #[test]
    /// This *will* fail if/when the api version gets bumped
    fn http_mod_get_api_version() {
        assert_eq!(API_VERSION.as_str(), S!("/v0"));
    }

    /// Make request to all endpoints to seed incoming_request entries, will make 21 reqeusts
    async fn test_seed_stats() {
        let urls = [
            "/aircraft/test",
            "/airline/test",
            "/callsign/test",
            "/mode-s/test",
            "/n-number/test",
            "/online",
        ];
        let mut count = 0;
        for i in urls.iter().enumerate() {
            let url = format!("http://127.0.0.1:8282{}{}", API_VERSION.as_str(), i.1);
            for _ in 0..=i.0 {
                count += 1;
                CLIENT.get(&url).send().await.unwrap();
                // Sleep to re-seed the redis stats, or allow stats insertion
                if count == 20 {
                    sleep!(1600);
                } else {
                    sleep!(50);
                }
            }
        }
    }

    /// Check that the stats are all empty
    async fn assert_empty_stats_cache(redis: &Pool) {
        let stats_cache: Option<String> = redis.hget("stats", "data").await.unwrap();
        let stats_ttl = redis.ttl::<i64, _>("stats").await.unwrap();
        assert_eq!(stats_ttl, 600);
        assert!(stats_cache.is_some());
        let cache = serde_json::from_str::<Stats>(&stats_cache.unwrap()).unwrap();

        assert_eq!(cache.daily.aircraft, vec![]);
        assert_eq!(cache.daily.airline, vec![]);
        assert_eq!(cache.daily.callsign, vec![]);
        assert_eq!(cache.daily.n_number, vec![]);
        assert_eq!(cache.daily.mode_s, vec![]);
        assert_eq!(cache.daily.online, vec![]);
        assert_eq!(cache.daily.stats, vec![]);
        assert_eq!(cache.daily.aggregate, 0);
    }

    /// Send a request to a path, return response
    async fn send_request(path: &str) -> reqwest::Response {
        let response = CLIENT
            .get(format!(
                "http://127.0.0.1:8282{}/{path}",
                API_VERSION.as_str()
            ))
            .send()
            .await;
        assert!(response.is_ok());
        response.unwrap()
    }

    /// Check a response returns a 400, and return TestReponse<T> value
    async fn get_response<T: DeserializeOwned>(path: &str, status: StatusCode) -> T {
        let response = send_request(path).await;
        assert_eq!(response.status(), status);
        let response = response.json::<ResponseJson<T>>().await;
        assert!(response.is_ok());
        response.unwrap().response
    }

    /// Check a response returns a 200, and return TestReponse<T> value
    pub async fn get_response_200<T: DeserializeOwned>(path: &str) -> T {
        get_response(path, status::StatusCode::OK).await
    }

    /// Check a response returns a 400, and return TestReponse<T> value
    async fn get_response_400<T: DeserializeOwned>(path: &str) -> T {
        get_response(path, status::StatusCode::BAD_REQUEST).await
    }

    /// Check a response returns a 404, and return TestReponse<T> value
    async fn get_response_404<T: DeserializeOwned>(path: &str) -> T {
        get_response(path, status::StatusCode::NOT_FOUND).await
    }

    /// Check a response returns a 429, and return err message as string
    async fn get_response_429() -> String {
        get_response("online", status::StatusCode::TOO_MANY_REQUESTS).await
    }

    ///************
    /// Stats route
    ///************
    ///
    /// Stats route
    /// /v[API_VERSION]/stats
    #[tokio::test]
    /// Test the stats endpoint, can only really test the "daily" key, as the "total" key will have real data in it
    /// Check that the stats cache is correctly populated and ttl/expire working as expected
    async fn http_mod_get_stats() {
        let setup = start_server(None).await;
        assert_empty_stats_cache(&setup.redis).await;

        let response = get_response_200::<Stats>("stats").await;

        assert_eq!(response.daily.aircraft, vec![]);
        assert_eq!(response.daily.aircraft, vec![]);
        assert_eq!(response.daily.callsign, vec![]);
        assert_eq!(response.daily.mode_s, vec![]);
        assert_eq!(response.daily.n_number, vec![]);
        assert_eq!(response.daily.online, vec![]);
        assert_eq!(response.daily.stats, vec![]);
        assert_eq!(response.daily.aggregate, 0);

        test_seed_stats().await;
        setup
            .redis
            .del::<(), _>(RedisKey::Stats.to_string())
            .await
            .unwrap();

        let result = get_response_200::<Stats>("stats").await;

        assert_eq!(
            serde_json::to_string(&result.daily).unwrap(),
            r#"{"aircraft":[{"url":"/v0/aircraft/test","count":1}],"airline":[{"url":"/v0/airline/test","count":2}],"callsign":[{"url":"/v0/callsign/test","count":3}],"mode_s":[{"url":"/v0/mode-s/test","count":4}],"n_number":[{"url":"/v0/n-number/test","count":5}],"online":[{"url":"/v0/online","count":6}],"stats":[{"url":"/v0/stats","count":1}],"aggregate":22}"#
        );
    }

    #[tokio::test]
    /// Check that request ids are correctly stores in the redis cache, with valid TTLs, and also inserted in postgres
    async fn http_mod_stats_cache() {
        let setup = start_server(None).await;

        test_seed_stats().await;

        let assert_ttl = |ttl: i64| assert!(ttl > 604790);

        let version_result = setup
            .redis
            .hget::<i64, &str, &str>("ir::v::v0", "data")
            .await;
        assert!(version_result.is_ok());

        assert_ttl(setup.redis.ttl::<i64, &str>("ir::v::v0").await.unwrap());

        let query_result = setup
            .redis
            .hget::<i64, &str, &str>("ir::q::test", "data")
            .await;
        assert!(query_result.is_ok());
        let query_id = query_result.unwrap();

        assert_ttl(setup.redis.ttl::<i64, &str>("ir::q::test").await.unwrap());

        for path in ["aircraft", "callsign", "mode-s", "n-number", "online"] {
            let path_key = format!("ir::p::{path}");
            let path_result = setup.redis.hget::<i64, &str, &str>(&path_key, "data").await;
            assert!(path_result.is_ok());

            assert_ttl(setup.redis.ttl::<i64, String>(path_key).await.unwrap());

            let path_id = path_result.unwrap();
            let pg_result = if path == "online" {
                sqlx::query(
                    "SELECT * FROM incoming_request WHERE incoming_request_url_path_id = $1 AND incoming_request_url_query_id IS NULL",
                )
                .bind(path_id)
                .fetch_optional(&setup.postgres)
                .await
            } else {
                sqlx::query(
                    "SELECT * FROM incoming_request WHERE incoming_request_url_path_id = $1 AND incoming_request_url_query_id = $2",
                )
                .bind(path_id)
                .bind(query_id)
                .fetch_optional(&setup.postgres)
                .await
            };
            assert!(pg_result.is_ok());
            assert!(pg_result.unwrap().is_some());

            let minute = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                / 60;
            let key = RedisKey::TempIR((path, minute)).to_string();
            let temp_bucket = setup.redis.exists::<bool, _>(&key).await.unwrap();
            assert!(temp_bucket);
        }
    }

    ///****************
    /// Callsign routes
    ///****************/
    ///
    /// Callsign route
    /// /v[API_VERSION]/callsign/[ICAO]
    #[tokio::test]
    async fn http_mod_get_callsign_icao() {
        start_server(None).await;
        let known_flightroute = FLIGHTROUTE.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "callsign/{}",
            known_flightroute.callsign
        ))
        .await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[IATA]
    #[tokio::test]
    async fn http_mod_get_callsign_iata() {
        start_server(None).await;
        let mut known_flightroute = FLIGHTROUTE.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "callsign/{}",
            known_flightroute.callsign_iata.as_ref().unwrap()
        ))
        .await;
        known_flightroute.callsign = known_flightroute.callsign_iata.as_ref().unwrap().to_owned();
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[OTHER]
    #[tokio::test]
    async fn http_mod_get_callsign_other() {
        start_server(None).await;
        let known_flightroute = FLIGHTROUTE_OTHER.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "callsign/{}",
            known_flightroute.callsign
        ))
        .await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[ICAO]
    #[tokio::test]
    async fn http_mod_get_callsign_midpoint_icao() {
        start_server(None).await;
        let known_flightroute = FLIGHTROUTE_MIDPOINT.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "callsign/{}",
            known_flightroute.callsign
        ))
        .await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[IATA]
    #[tokio::test]
    async fn http_mod_get_callsign_midpoint_iata() {
        start_server(None).await;
        let mut known_flightroute = FLIGHTROUTE_MIDPOINT.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "callsign/{}",
            known_flightroute.callsign_iata.as_ref().unwrap()
        ))
        .await;
        known_flightroute.callsign = known_flightroute.callsign_iata.as_ref().unwrap().to_owned();
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[ICAO]
    #[tokio::test]
    async fn http_mod_get_callsign_icao_unknown() {
        start_server(None).await;
        let response =
            get_response_404::<String>(&format!("callsign/{}", UNKNOWN_CALLSIGN_ICAO)).await;
        assert_eq!(response, "unknown callsign");
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[IATA]
    #[tokio::test]
    async fn http_mod_get_callsign_iata_unknown() {
        start_server(None).await;
        let response =
            get_response_404::<String>(&format!("callsign/{}", UNKNOWN_CALLSIGN_IATA)).await;
        assert_eq!(response, "unknown callsign");
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[OTHER]
    #[tokio::test]
    async fn http_mod_get_callsign_other_unknown() {
        start_server(None).await;
        let response =
            get_response_404::<String>(&format!("callsign/{}", UNKNOWN_CALLSIGN_OTHER)).await;
        assert_eq!(response, "unknown callsign");
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[random_string]
    #[tokio::test]
    async fn http_mod_get_callsign_malformed() {
        start_server(None).await;
        let random_hex = random_hex(10);
        let response = get_response_400::<String>(&format!("callsign/{}", random_hex)).await;
        assert_eq!(response, format!("invalid callsign: {random_hex}"));
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/random
    #[tokio::test]
    async fn http_mod_get_callsign_random() {
        start_server(None).await;
        for _ in 0..20 {
            let response = get_response_200::<AircraftAndRoute>("callsign/random").await;
            assert!(response.aircraft.is_none());
            assert!(response.flightroute.is_some());
        }
    }

    ///****************
    /// Aircraft routes
    ///****************/
    ///
    /// Aircraft route
    /// /v[API_VERSION]/aircraft/random
    #[tokio::test]
    async fn http_mod_get_aircraft_random() {
        start_server(None).await;
        for _ in 0..20 {
            let response = get_response_200::<AircraftAndRoute>("aircraft/random").await;
            assert!(response.aircraft.is_some());
            assert!(response.flightroute.is_none());
        }
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[UNKNOWN_ICAO]
    #[tokio::test]
    async fn http_mod_get_aircraft_unknown_icao() {
        start_server(None).await;
        let response =
            get_response_404::<String>(&format!("aircraft/{UNKNOWN_AIRCRAFT_MODE_S}",)).await;
        assert_eq!(response, "unknown aircraft");
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[UNKNOWN_ICAO]
    #[tokio::test]
    async fn http_mod_get_aircraft_unknown_registration() {
        start_server(None).await;
        let response =
            get_response_404::<String>(&format!("aircraft/{UNKNOWN_AIRCRAFT_REGISTRATION}",)).await;
        assert_eq!(response, "unknown aircraft");
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[malformed]
    #[tokio::test]
    async fn http_mod_get_aircraft_malformed() {
        start_server(None).await;
        let response = get_response_404::<String>(&format!("aircraft/{}", random_hex(8))).await;
        assert_eq!(response, "unknown aircraft");
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[MODE_S]
    #[tokio::test]
    async fn http_mod_get_aircraft_mode_s() {
        let state = start_server(None).await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let response =
            get_response_200::<AircraftAndRoute>(&format!("aircraft/{}", known_aircraft.mode_s))
                .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[MODE_S]
    #[tokio::test]
    async fn http_mod_get_aircraft_mode_s_no_photo() {
        start_server(None).await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let response =
            get_response_200::<AircraftAndRoute>(&format!("aircraft/{}", known_aircraft.mode_s))
                .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]
    #[tokio::test]
    async fn http_mod_get_aircraft_registration() {
        let state = start_server(None).await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}",
            known_aircraft.registration
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]
    #[tokio::test]
    async fn http_mod_get_aircraft_registration_s_no_photo() {
        start_server(None).await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}",
            known_aircraft.registration
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[MODE_S]?callsign=[UNKNOWN_ICAO]
    #[tokio::test]
    async fn http_mod_get_aircraft_mode_s_unknown_flightroute_icao() {
        let state = start_server(None).await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.mode_s, UNKNOWN_CALLSIGN_ICAO
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[MODE_S]?callsign=[UNKNOWN_IATA]
    #[tokio::test]
    async fn http_mod_get_aircraft_mode_s_unknown_flightroute_iata() {
        let state = start_server(None).await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.mode_s, UNKNOWN_CALLSIGN_IATA
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[MODE_S]?callsign=[UNKNOWN_OTHER]
    #[tokio::test]
    async fn http_mod_get_aircraft_mode_s_unknown_flightroute_other() {
        let state = start_server(None).await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.mode_s, UNKNOWN_CALLSIGN_OTHER
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// /v[API_VERSION]/aircraft/[REGISTRATION]?callsign=[UNKNOWN_ICAO]
    #[tokio::test]
    async fn http_mod_get_aircraft_registration_unknown_flightroute_icao() {
        let state = start_server(None).await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.registration, UNKNOWN_CALLSIGN_ICAO
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[REGISTRATION]?callsign=[UNKNOWN_IATA]
    #[tokio::test]
    async fn http_mod_get_aircraft_registration_unknown_flightroute_iata() {
        let state = start_server(None).await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.registration, UNKNOWN_CALLSIGN_IATA
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[REGISTRATION]?callsign=[UNKNOWN_OTHER]
    #[tokio::test]
    async fn http_mod_get_aircraft_registration_unknown_flightroute_other() {
        let state = start_server(None).await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.registration, UNKNOWN_CALLSIGN_OTHER
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[MODE_S]?callsign=[ICAO]
    #[tokio::test]
    async fn http_mod_get_aircraft_mode_s_callsign_icao_no_photo() {
        start_server(None).await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let known_flightroute = FLIGHTROUTE.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.mode_s,
            known_flightroute.callsign_icao.as_ref().unwrap()
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_some());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[MODE_S]?callsign=[IATA]
    #[tokio::test]
    async fn http_mod_get_aircraft_mode_s_callsign_iata_no_photo() {
        start_server(None).await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let mut known_flightroute = FLIGHTROUTE.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.mode_s,
            known_flightroute.callsign_iata.as_ref().unwrap()
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_some());
        known_flightroute.callsign = known_flightroute.callsign_iata.as_ref().unwrap().to_owned();
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[MODE_S]?callsign=[OTHER]
    #[tokio::test]
    async fn http_mod_get_aircraft_mode_s_callsign_other_no_photo() {
        start_server(None).await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let known_flightroute = FLIGHTROUTE_OTHER.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.mode_s, known_flightroute.callsign
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_some());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[REGISTRATION]?callsign=[ICAO]
    #[tokio::test]
    async fn http_mod_get_aircraft_registration_callsign_icao_no_photo() {
        start_server(None).await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let known_flightroute = FLIGHTROUTE.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.registration,
            known_flightroute.callsign_icao.as_ref().unwrap()
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_some());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[REGISTRATION]?callsign=[IATA]
    #[tokio::test]
    async fn http_mod_get_aircraft_registration_callsign_iata_no_photo() {
        start_server(None).await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let mut known_flightroute = FLIGHTROUTE.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.registration,
            known_flightroute.callsign_iata.as_ref().unwrap()
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_some());
        known_flightroute.callsign = known_flightroute.callsign_iata.as_ref().unwrap().to_owned();
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[REGISTRATION]?callsign=[OTHER]
    #[tokio::test]
    async fn http_mod_get_aircraft_registration_callsign_other_no_photo() {
        start_server(None).await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let known_flightroute = FLIGHTROUTE_OTHER.clone();
        let response = get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}?callsign={}",
            known_aircraft.registration, known_flightroute.callsign
        ))
        .await;
        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_some());
        assert_eq!(response.aircraft.unwrap(), known_aircraft);
        assert_eq!(known_flightroute, response.flightroute.unwrap());
    }

    ///***************
    /// Airline routes
    ///***************/
    ///
    /// Airline route
    /// /v[API_VERSION]/airline/random
    #[tokio::test]
    async fn http_mod_get_airline_random() {
        start_server(None).await;
        for _ in 0..20 {
            let response = get_response_200::<Vec<ResponseAirline>>("airline/random").await;
            assert!(response.len() == 1);
            assert!(!response[0].name.is_empty())
        }
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[UNKNOWN_ICAO]
    #[tokio::test]
    async fn http_mod_get_airline_unknown_icao() {
        start_server(None).await;
        let response =
            get_response_404::<String>(&format!("airline/{UNKNOWN_AIRLINE_ICAO}",)).await;
        assert_eq!(response, "unknown airline");
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[UNKNOWN_IATA]
    #[tokio::test]
    async fn http_mod_get_airline_unknown_iata() {
        start_server(None).await;
        let response = get_response_404::<String>(&format!("airline/{UNKNOWN_AIRLINE_IATA}")).await;
        assert_eq!(response, "unknown airline");
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[random]
    #[tokio::test]
    async fn http_mod_get_airline_unknown_random() {
        start_server(None).await;
        let response = get_response_404::<String>(&format!("airline/{}", random_hex(3))).await;
        assert_eq!(response, "unknown airline");
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[malformed]
    #[tokio::test]
    async fn http_mod_get_airline_unknown_malformed() {
        start_server(None).await;
        let random_hex = random_hex(10);
        let response = get_response_400::<String>(&format!("airline/{}", random_hex)).await;
        assert_eq!(response, format!("invalid airline: {random_hex}"));
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[ICAO]
    #[tokio::test]
    async fn http_mod_get_airline_icao() {
        start_server(None).await;
        let airline = KNOWN_AIRLINE.clone();
        let response =
            get_response_200::<Vec<ResponseAirline>>(&format!("airline/{}", airline.icao)).await;
        assert_eq!(response, vec![airline]);
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[IATA]
    #[tokio::test]
    async fn http_mod_get_airline_iata() {
        start_server(None).await;
        let airline = KNOWN_AIRLINE.clone();
        let response = get_response_200::<Vec<ResponseAirline>>(&format!(
            "airline/{}",
            airline.iata.as_ref().unwrap()
        ))
        .await;
        assert_eq!(response, vec![airline]);
    }

    ///***************
    /// N-Number routes
    ///***************/
    ///
    /// N-Number route
    /// /v[API_VERSION]/n-number/[VALID-N-NUMBER]
    #[tokio::test]
    async fn http_mod_get_n_number_valid() {
        start_server(None).await;
        let response =
            get_response_200::<String>(&format!("n-number/{}", AIRCRAFT_NO_PHOTO.registration))
                .await;
        assert_eq!(response, AIRCRAFT_NO_PHOTO.mode_s);
    }

    /// N-Number route
    /// /v[API_VERSION]/n-number/[VALID-N-NUMBER]
    #[tokio::test]
    async fn http_mod_get_n_number_invalid() {
        start_server(None).await;
        let response =
            get_response_400::<String>(&format!("n-number/{}", AIRCRAFT_NO_PHOTO.mode_s)).await;
        assert_eq!(
            response,
            format!("invalid n-number: {}", AIRCRAFT_NO_PHOTO.mode_s)
        );
    }

    /// N-Number route
    /// /v[API_VERSION]/n-number/[VALID-N-NUMBER]/[random_hex]
    #[tokio::test]
    async fn http_mod_get_n_number_404() {
        start_server(None).await;
        let aircraft = AIRCRAFT_NO_PHOTO.clone();
        let random_hex = random_hex(10);
        let response = get_response_404::<String>(&format!(
            "n-number/{}/{}",
            aircraft.registration, random_hex
        ))
        .await;
        assert_eq!(
            response,
            format!(
                "unknown endpoint: {}/n-number/{}/{}",
                *API_VERSION, aircraft.registration, random_hex
            )
        );
    }

    ///***************
    /// Mode-s routes
    ///***************/
    ///
    /// Mode-S route
    /// /v[API_VERSION]/mode-s/[VALID-MODE-S]
    #[tokio::test]
    async fn http_mod_get_mode_s_valid() {
        start_server(None).await;
        let response =
            get_response_200::<String>(&format!("mode-s/{}", AIRCRAFT_NO_PHOTO.mode_s)).await;
        assert_eq!(response, AIRCRAFT_NO_PHOTO.registration);
    }

    /// Mode-S route
    /// /v[API_VERSION]/mode-s/[INVALID-MODE-S]
    #[tokio::test]
    async fn http_mod_get_mode_s_invalid() {
        start_server(None).await;
        let response =
            get_response_400::<String>(&format!("mode-s/{}", AIRCRAFT_NO_PHOTO.registration)).await;
        assert_eq!(
            response,
            format!("invalid mode-s: {}", AIRCRAFT_NO_PHOTO.registration)
        );
    }

    /// Mode-S route - a valid Mode-S, but not for an aircraft that has an N-Number
    /// /v[API_VERSION]/mode-s/[VALID-MODE-S-INVALID-CONVERSION]
    #[tokio::test]
    async fn http_mod_get_mode_s_valid_invalid_n_number() {
        start_server(None).await;
        let response =
            get_response_404::<String>(&format!("mode-s/{VALID_MODE_S_NO_N_NUMBER}")).await;
        assert_eq!(response, "unknown mode-s conversion");
    }

    /// Mode-S route
    /// /v[API_VERSION]/mode-s/[VALID-MODE-S]/[RANDOM_HEX]
    #[tokio::test]
    async fn http_mod_get_mode_s_404() {
        start_server(None).await;
        let random_hex = random_hex(10);
        let response = get_response_404::<String>(&format!(
            "mode-s/{}/{}",
            AIRCRAFT_NO_PHOTO.mode_s, random_hex
        ))
        .await;
        assert_eq!(
            response,
            format!(
                "unknown endpoint: {}/mode-s/{}/{}",
                *API_VERSION, AIRCRAFT_NO_PHOTO.mode_s, random_hex
            )
        );
    }

    ///**************
    /// Online routes
    ///**************/
    ///
    /// Online route
    /// /v[API_VERSION]/mode-s/[VALID-MODE-S]
    #[tokio::test]
    async fn http_mod_get_online() {
        start_server(None).await;
        sleep!();
        let response = get_response_200::<Online>("online").await;

        assert_eq!(response.api_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(response.uptime, 1);
    }

    /// Mode-S route
    /// /v[API_VERSION]/online/[RANDOM_HEX]
    #[tokio::test]
    async fn http_mod_get_online_404() {
        start_server(None).await;
        let random_hex = random_hex(10);
        let response = get_response_404::<String>(&format!("online/{}", random_hex)).await;
        assert_eq!(
            response,
            format!("unknown endpoint: {}/online/{}", *API_VERSION, random_hex)
        );
    }

    ///*****************
    /// Rate limit tests
    ///*****************/

    #[tokio::test]
    /// Not rate limited, but rate limit points = number of requests, and ttl 60
    async fn http_mod_rate_limit() {
        let setup = start_server(None).await;
        for _ in 1..=45 {
            get_response_200::<Online>("online").await;
        }

        assert_eq!(get_rate_limit_count(&setup.redis).await, 45);
        assert_eq!(get_rate_limit_ttl(&setup.redis).await, 60);
    }

    #[tokio::test]
    async fn http_mod_rate_limit_small() {
        let setup = start_server(None).await;

        for _ in 1..=511 {
            send_request("online").await;
        }

        // 512th request is fine
        get_response_200::<Online>("online").await;

        // 512th+ request is rate limited
        let response = get_response_429().await;
        assert_eq!(response, gen_ttl_msg(60));

        assert_eq!(get_rate_limit_ttl(&setup.redis).await, 60);

        sleep!(1000);

        // TTL reduces by 1 after 1 second
        assert_eq!(get_rate_limit_ttl(&setup.redis).await, 59);
        sleep!(1000);

        // TTL doesn't get reset on further requwest
        let response = get_response_429().await;
        assert_eq!(response, gen_ttl_msg(58));
        assert_eq!(get_rate_limit_ttl(&setup.redis).await, 58);

        // points increased
        assert_eq!(get_rate_limit_count(&setup.redis).await, 514);
    }

    #[tokio::test]
    async fn http_mod_rate_limit_big() {
        let setup = start_server(None).await;

        for _ in 1..=1023 {
            send_request("online").await;
        }

        // 1023rd request is rate limited
        let response = get_response_429().await;
        assert!([gen_ttl_msg(59), gen_ttl_msg(60)].contains(&response));

        // 1024th + request is rate limited for 300 seconds
        let response = get_response_429().await;
        assert_eq!(response, gen_ttl_msg(300));

        assert_eq!(get_rate_limit_ttl(&setup.redis).await, 300);

        sleep!(1000);

        // TTL reduces by 1 after 1 second
        assert_eq!(get_rate_limit_ttl(&setup.redis).await, 299);

        // TTL is reset to 300 on one more request
        let response = get_response_429().await;
        assert_eq!(response, gen_ttl_msg(300));
        assert_eq!(get_rate_limit_ttl(&setup.redis).await, 300);

        // points increased
        assert_eq!(get_rate_limit_count(&setup.redis).await, 1026);
    }
}
