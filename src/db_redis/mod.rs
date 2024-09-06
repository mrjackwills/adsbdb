use crate::{
    api::{AircraftSearch, AirlineCode, AppError, Callsign, ModeS, Registration},
    parse_env::AppEnv,
};
use fred::{
    clients::RedisPool,
    interfaces::{ClientLike, HashesInterface, KeysInterface},
    types::{FromRedis, ReconnectPolicy},
};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, fmt, net::IpAddr};
pub mod ratelimit;

const ONE_WEEK_AS_SEC: i64 = 60 * 60 * 24 * 7;
const HASH_FIELD: &str = "data";

/// Macro to convert a stringified struct back into the struct
#[macro_export]
macro_rules! redis_hash_to_struct {
    ($struct_name:ident) => {
        impl fred::types::FromRedis for $struct_name {
            fn from_value(
                value: fred::prelude::RedisValue,
            ) -> Result<Self, fred::prelude::RedisError> {
                value.as_str().map_or(
                    Err(fred::error::RedisError::new(
                        fred::error::RedisErrorKind::Parse,
                        format!("FromRedis: {}", stringify!(struct_name)),
                    )),
                    |i| {
                        serde_json::from_str::<Self>(&i).map_err(|_| {
                            fred::error::RedisError::new(
                                fred::error::RedisErrorKind::Parse,
                                "serde",
                            )
                        })
                    },
                )
            }
        }
    };
}

/// Insert an Option<model> into cache, using redis hashset
pub async fn insert_cache<'a, T: Serialize + Send + Sync>(
    redis: &RedisPool,
    to_insert: &Option<T>,
    key: &RedisKey<'a>,
) -> Result<(), AppError> {
    let key = key.to_string();
    let serialized = to_insert.as_ref().map_or_else(String::new, |i| {
        serde_json::to_string(&i).unwrap_or_default()
    });
    redis
        .hset::<(),_,_>(&key, HashMap::from([(HASH_FIELD, serialized)]))
        .await?;
    Ok(redis.expire(&key, ONE_WEEK_AS_SEC).await?)
}

/// See if give value is in cache, if so, extend ttl, and deserialize into T
pub async fn get_cache<'a, T: DeserializeOwned + Send + FromRedis>(
    redis: &RedisPool,
    key: &RedisKey<'a>,
) -> Result<Option<Option<T>>, AppError> {
    let key = key.to_string();
    if let Some(value) = redis
        .hget::<Option<String>, &str, &str>(&key, HASH_FIELD)
        .await?
    {
        let _: () = redis.expire(&key, ONE_WEEK_AS_SEC).await?;
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
