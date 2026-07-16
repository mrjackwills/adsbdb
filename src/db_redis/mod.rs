use crate::{
    S,
    api::{AircraftSearch, AirlineCode, AppError, Callsign, ModeS, Registration},
    db_postgres::{PathID, QueryID, RE_SEED_TIME, VersionID},
    parse_env::AppEnv,
};
use fred::{
    clients::Pool,
    interfaces::{ClientLike, HashesInterface, KeysInterface},
    prelude::ReconnectPolicy,
    types::FromValue,
};
use serde::{Serialize, de::DeserializeOwned};
// use tower_http::ServiceExt;
use std::{collections::HashMap, fmt, net::IpAddr};
pub mod ratelimit;

pub const ONE_MINUTE_AS_SEC: i64 = 60;
pub const ONE_WEEK_AS_SEC: i64 = ONE_MINUTE_AS_SEC * 60 * 24 * 7;
pub const HASH_FIELD: &str = "data";

/// Macro to convert a stringified struct back into the struct
#[macro_export]
macro_rules! redis_hash_to_struct {
    ($struct_name:ident) => {
        impl fred::types::FromValue for $struct_name {
            fn from_value(value: fred::prelude::Value) -> Result<Self, fred::prelude::Error> {
                value.as_str().map_or(
                    Err(fred::error::Error::new(
                        fred::error::ErrorKind::Parse,
                        format!("FromRedis: {}", stringify!(struct_name)),
                    )),
                    |i| {
                        serde_json::from_str::<Self>(&i).map_err(|_| {
                            fred::error::Error::new(fred::error::ErrorKind::Parse, "serde")
                        })
                    },
                )
            }
        }
    };
}

/// Insert an Option<model> into cache, using redis hashset
pub async fn insert_cache<T: Serialize + Send + Sync>(
    redis: &Pool,
    to_insert: Option<&T>,
    key: RedisKey<'_>,
) -> Result<(), AppError> {
    let ttl = key.get_ttl();
    let key = key.to_string();
    let serialized = to_insert
        .as_ref()
        .map_or_else(|| S!(), |i| serde_json::to_string(&i).unwrap_or_default());
    redis
        .hset::<(), _, _>(&key, HashMap::from([(HASH_FIELD, serialized)]))
        .await?;

    Ok(redis.expire(&key, ttl, None).await?)
}

/// See if give value is in cache, if so, extend ttl, and deserialize into T
pub async fn get_cache<T: DeserializeOwned + Send + FromValue>(
    redis: &Pool,
    key: &RedisKey<'_>,
) -> Result<Option<Option<T>>, AppError> {
    let set_expire = key.get_expire();
    let key = key.to_string();
    if let Some(value) = redis
        .hget::<Option<String>, &str, &str>(&key, HASH_FIELD)
        .await?
    {
        if set_expire {
            redis
                .expire::<(), &str>(&key, ONE_WEEK_AS_SEC, None)
                .await?;
        }
        if value.is_empty() {
            return Ok(Some(None));
        }
        if let Some(value) = serde_json::from_str(&value)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

pub async fn get_pool(app_env: &AppEnv) -> Result<Pool, AppError> {
    let redis_url = format!(
        "redis://:{password}@{host}:{port}/{db}",
        password = app_env.redis_password,
        host = app_env.redis_host,
        port = app_env.redis_port,
        db = app_env.redis_database
    );

    let config = fred::types::config::Config::from_url(&redis_url)?;
    let pool = fred::types::Builder::from_config(config)
        .set_policy(ReconnectPolicy::new_exponential(0, 100, 30_000, 2))
        .build_pool(32)?;
    pool.init().await?;
    Ok(pool)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingRequestKey<'a> {
    IncomingRequestUrl(
        Option<&'a VersionID>,
        Option<&'a PathID>,
        Option<&'a QueryID>,
    ),
    Path(&'a str),
    Query(&'a str),
    Version(&'a str),
}

impl fmt::Display for IncomingRequestKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::IncomingRequestUrl(version, path, query) => write!(
                f,
                "v::{}::p::{}::q::{}",
                version.map_or(S!(), |i| i.get().to_string()),
                path.map_or(S!(), |i| i.get().to_string()),
                query.map_or(S!(), |i| i.get().to_string()),
            ),
            Self::Query(query) => write!(f, "q::{query}"),
            Self::Path(path) => write!(f, "p::{path}"),
            Self::Version(version) => write!(f, "v::{version}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedisKey<'a> {
    Airline(&'a AirlineCode),
    Callsign(&'a Callsign),
    IncomingRequest(IncomingRequestKey<'a>),
    ModeS(&'a ModeS),
    RateLimit(IpAddr),
    Registration(&'a Registration),
    Stats,
}

impl<'a> RedisKey<'a> {
    const fn get_ttl(&self) -> i64 {
        match self {
            // Want this to be double the RE_SEED_TIME, so that there is always a cache available
            Self::Stats => RE_SEED_TIME.wrapping_mul(2),
            _ => ONE_WEEK_AS_SEC,
        }
    }

    const fn get_expire(&self) -> bool {
        !matches!(self, Self::Stats)
    }
}

impl fmt::Display for RedisKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Airline(airline) => write!(f, "airline::{airline}"),
            Self::Callsign(callsign) => write!(f, "callsign::{callsign}"),
            Self::ModeS(mode_s) => write!(f, "mode_s::{mode_s}"),
            Self::RateLimit(ip) => write!(f, "ratelimit::{ip}"),
            Self::Registration(registration) => write!(f, "registration::{registration}"),
            Self::Stats => write!(f, "stats"),
            Self::IncomingRequest(incoming_request_key) => {
                write!(f, "ir::{incoming_request_key}")
            }
        }
    }
}

impl<'a> From<&'a AircraftSearch> for RedisKey<'a> {
    fn from(value: &'a AircraftSearch) -> Self {
        match value {
            AircraftSearch::ModeS(mode_s) => Self::ModeS(mode_s),
            AircraftSearch::Registration(registration) => Self::Registration(registration),
        }
    }
}
