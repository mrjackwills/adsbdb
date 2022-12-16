use crate::{
    api::{AppError, Callsign, ModeS},
    parse_env::AppEnv,
};
use redis::{
    aio::Connection, from_redis_value, AsyncCommands, ConnectionAddr, ConnectionInfo,
    RedisConnectionInfo, Value,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt, net::IpAddr, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tracing::{error, info};

const ONE_WEEK: usize = 60 * 60 * 24 * 7;
// const ONE_WEEK: usize = 30;
const FIELD: &str = "data";

/// Convert a redis string result into a Option<T>
fn redis_to_serde<T: DeserializeOwned>(v: &Value) -> Result<Option<T>, AppError> {
    match from_redis_value::<String>(v) {
        Ok(string_value) => {
            if string_value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(serde_json::from_str::<T>(&string_value)?))
            }
        }
        Err(e) => {
            info!("value::{:#?}", v);
            error!("{:?}", e);
            Err(AppError::RedisError(e))
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
    let value: Option<Value> = redis.hget(&key, FIELD).await?;
    if value.is_some() {
        redis.expire(&key, ONE_WEEK).await?;
    }
    let serialized_data = match value {
        Some(d) => Some(redis_to_serde(&d)?),
        None => None,
    };
    Ok(serialized_data)
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
    redis.expire(&key, ONE_WEEK).await?;
    Ok(())
}

/// Check if rate limited, will return true if so
/// info!() at the moment for bug hunting
pub async fn check_rate_limit(
    redis: &Arc<Mutex<Connection>>,
    key: RedisKey<'_>,
) -> Result<(), AppError> {
    let key = key.to_string();
    let mut redis = redis.lock().await;
    let count = redis.get::<&str, Option<usize>>(&key).await?;
    if let Some(count) = count {
        redis.incr(&key, 1).await?;
        if count >= 240 {
            info!("count: {}, key:{}", count, key);
            info!("blocked for 5 minutes::{}", key);
            redis.expire(&key, 60 * 5).await?;
        }
        if count > 120 {
            info!("count: {}, key:{}", count, key);
            return Err(AppError::RateLimited(
                redis.ttl::<&str, usize>(&key).await?
            ));
        }
        if count == 120 {
            info!("count: {}, key:{}", count, key);
            redis.expire(&key, 60).await?;
            return Err(AppError::RateLimited(60));
        }
    } else {
        redis.incr(&key, 1).await?;
        redis.expire(&key, 60).await?;
    }
    Ok(())
}

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
    Callsign(&'a Callsign),
    ModeS(&'a ModeS),
    RateLimit(IpAddr),
}

impl<'a> fmt::Display for RedisKey<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Callsign(callsign) => write!(f, "callsign::{callsign}"),
            Self::ModeS(mode_s) => write!(f, "mode_s::{mode_s}"),
            Self::RateLimit(ip) => write!(f, "ratelimit::{ip}"),
        }
    }
}
