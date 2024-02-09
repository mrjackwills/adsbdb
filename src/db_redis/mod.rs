use crate::{
    api::{AircraftSearch, AirlineCode, AppError, Callsign, ModeS, Registration},
    parse_env::AppEnv,
};
use fred::{
    clients::RedisPool,
    interfaces::{ClientLike, KeysInterface},
    types::{Expiration, ReconnectPolicy},
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt, net::IpAddr};
pub mod ratelimit;

const ONE_WEEK_AS_SEC: i64 = 60 * 60 * 24 * 7;

/// Insert an Option<model> into cache, using redis hashset
pub async fn insert_cache<'a, T: Serialize + Send + Sync + fmt::Debug>(
    redis: &RedisPool,
    to_insert: &Option<T>,
    key: &RedisKey<'a>,
) -> Result<(), AppError> {
    let key = key.to_string();
    let serialized = match to_insert {
        Some(value) => serde_json::to_string(value)?,
        None => String::new(),
    };

    redis
        .set(
            &key,
            serialized,
            Some(Expiration::EX(ONE_WEEK_AS_SEC)),
            None,
            false,
        )
        .await?;
    Ok(())
}

/// See if give value is in cache, if so, extend ttl, and deserialize into T
pub async fn get_cache<'a, T: DeserializeOwned + Send + fmt::Debug>(
    redis: &RedisPool,
    key: &RedisKey<'a>,
) -> Result<Option<Option<T>>, AppError> {
    let key = key.to_string();
    if let Some(value) = redis.get::<Option<String>, &str>(&key).await? {
        redis.expire(&key, ONE_WEEK_AS_SEC).await?;
        if value.is_empty() {
            return Ok(Some(None));
        }
        if let Some(value) = serde_json::from_str(&value)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

pub async fn get_pool(app_env: &AppEnv) -> Result<RedisPool, AppError> {
    let redis_url = format!(
        "redis://:{password}@{host}:{port}/{db}",
        password = app_env.redis_password,
        host = app_env.redis_host,
        port = app_env.redis_port,
        db = app_env.redis_database
    );

    let config = fred::types::RedisConfig::from_url(&redis_url)?;
    let pool = fred::types::Builder::from_config(config)
        .with_performance_config(|config| {
            config.auto_pipeline = true;
        })
        // use exponential backoff, starting at 100 ms and doubling on each failed attempt up to 30 sec
        .set_policy(ReconnectPolicy::new_exponential(0, 100, 30_000, 2))
        .build_pool(32)?;
    pool.init().await?;
    Ok(pool)
}

#[derive(Debug, Clone)]
pub enum RedisKey<'a> {
    Airline(&'a AirlineCode),
    Callsign(&'a Callsign),
    Registration(&'a Registration),
    ModeS(&'a ModeS),
    RateLimit(IpAddr),
}

impl<'a> fmt::Display for RedisKey<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Airline(airline) => write!(f, "airline::{airline}"),
            Self::Callsign(callsign) => write!(f, "callsign::{callsign}"),
            Self::ModeS(mode_s) => write!(f, "mode_s::{mode_s}"),
            Self::RateLimit(ip) => write!(f, "ratelimit::{ip}"),
            Self::Registration(registration) => write!(f, "registration::{registration}"),
        }
    }
}

impl<'a> From<&'a AircraftSearch> for RedisKey<'a> {
    fn from(value: &'a AircraftSearch) -> Self {
        match value {
            AircraftSearch::Registration(registration) => Self::Registration(registration),
            AircraftSearch::ModeS(mode_s) => Self::ModeS(mode_s),
        }
    }
}
