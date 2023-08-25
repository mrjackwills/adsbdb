use crate::{
    api::{AircraftSearch, AirlineCode, AppError, Callsign, ModeS, Registration},
    parse_env::AppEnv,
};
use redis::{
    aio::Connection, from_redis_value, AsyncCommands, ConnectionAddr, ConnectionInfo,
    RedisConnectionInfo, Value,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt, net::IpAddr, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tracing::error;
pub mod ratelimit;

const ONE_WEEK: usize = 60 * 60 * 24 * 7;
const FIELD: &str = "data";

/// Convert a redis string result into a Option<T>
fn redis_to_serde<T: DeserializeOwned>(v: &Value) -> Option<T> {
    if v == &Value::Nil {
        return None;
    }
    match from_redis_value::<String>(v) {
        Ok(string_value) => {
            if string_value.is_empty() {
                None
            } else {
                serde_json::from_str::<T>(&string_value).ok()
            }
        }
        Err(e) => {
            error!("value::{:#?}", v);
            error!("{e:?}");
            None
        }
    }
}

/// See if give value is in cache, if so, extend ttl, and deserialize into T
pub async fn get_cache<'a, T: DeserializeOwned + Send>(
    redis: &Arc<Mutex<Connection>>,
    key: &RedisKey<'a>,
) -> Result<Option<Option<T>>, AppError> {
    let key = key.to_string();
    let mut redis = redis.lock().await;
    if let Some(value) = redis
        .hget::<'_, &str, &str, Option<Value>>(&key, FIELD)
        .await?
    {
        redis.expire(&key, ONE_WEEK).await?;
        drop(redis);
        Ok(Some(redis_to_serde(&value)))
    } else {
        Ok(None)
    }
}

/// Insert an Option<model> into cache, using redis hashset
pub async fn insert_cache<'a, T: Serialize + Send + Sync + fmt::Debug>(
    redis: &Arc<Mutex<Connection>>,
    to_insert: &Option<T>,
    key: &RedisKey<'a>,
) -> Result<(), AppError> {
    let key = key.to_string();
    let mut redis = redis.lock().await;
    let cache = match to_insert {
        Some(v) => serde_json::to_string(&v)?,
        None => String::new(),
    };
    redis.hset(&key, FIELD, cache).await?;
    Ok(redis.expire::<&str, ()>(&key, ONE_WEEK).await?)
}

/// Get an async redis connection
pub async fn get_connection(app_env: &AppEnv) -> Result<Connection, AppError> {
    let connection_info = ConnectionInfo {
        redis: RedisConnectionInfo {
            db: i64::from(app_env.redis_database),
            password: Some(app_env.redis_password.clone()),
            username: None,
        },
        addr: ConnectionAddr::Tcp(app_env.redis_host.clone(), app_env.redis_port),
    };
    let client = redis::Client::open(connection_info)?;
    match tokio::time::timeout(Duration::from_secs(10), client.get_async_connection()).await {
        Ok(con) => Ok(con?),
        Err(_) => Err(AppError::Internal("Unable to connect to redis".to_owned())),
    }
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
