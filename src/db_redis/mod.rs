use crate::{api::AppError, parse_env::AppEnv};
use redis::{
    aio::Connection, from_redis_value, AsyncCommands, ConnectionAddr, ConnectionInfo, ErrorKind,
    RedisConnectionInfo, Value,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt, net::IpAddr, sync::Arc, time::Duration};
use tokio::sync::Mutex;

const ONE_WEEK: usize = 60 * 60 * 24 * 7;
const FIELD: &str = "data";

// Convert a redis string result into a struct/None
// If the value is null, returns as Some("none")
fn optional_null<T: DeserializeOwned>(v: &Value) -> Result<Option<T>, AppError> {
    match from_redis_value::<String>(v) {
        Ok(valid_string) => {
            // This can be either "null" or a Model struct
            Ok(Some(serde_json::from_str::<T>(&valid_string)?))
        }
        Err(e) => match e.kind() {
            ErrorKind::TypeError => Ok(None),
            _ => Err(AppError::RedisError(e)),
        },
    }
}

/// See if give value is in cache, if so, extend ttl, and deserialize into T
pub async fn get_cache<T: DeserializeOwned + Send>(
    redis: &Arc<Mutex<Connection>>,
    key: &RedisKey,
) -> Result<Option<T>, AppError> {
    let key = key.to_string();
    let value: Value = redis.lock().await.hget(&key, FIELD).await?;
    let serialized_data: Option<T> = optional_null(&value)?;
    // Can either by "null" or a Model struct,
    if serialized_data.is_some() {
        redis.lock().await.expire(&key, ONE_WEEK).await?;
    }
    Ok(serialized_data)
}

pub async fn insert_cache<T: Serialize + Send + Sync>(
    redis: &Arc<Mutex<Connection>>,
    to_insert: &T,
    key: &RedisKey,
) -> Result<(), AppError> {
    let key = key.to_string();
    let value = serde_json::to_string(&to_insert)?;
    redis.lock().await.hset(&key, FIELD, value).await?;
    redis.lock().await.expire(&key, ONE_WEEK).await?;
    Ok(())
}

/// Check if rate limited, will return true if so
pub async fn check_rate_limit(
    redis: &Arc<Mutex<Connection>>,
    key: RedisKey,
) -> Result<(), AppError> {
    let key = key.to_string();
    let mut redis = redis.lock().await;
    let count = redis.get::<&str, Option<usize>>(&key).await?;
    redis.incr(&key, 1).await?;

    if let Some(count) = count {
        if count >= 240 {
            redis.expire(&key, 60 * 5).await?;
        }
        if count > 120 {
            return Err(AppError::RateLimited(redis.ttl(&key).await?));
        }
        if count == 120 {
            redis.expire(&key, 60).await?;
            return Err(AppError::RateLimited(60));
        }
    } else {
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
pub enum RedisKey {
    Callsign(String),
    ModeS(String),
    RateLimit(IpAddr),
}

impl fmt::Display for RedisKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Callsign(callsign) => write!(f, "callsign::{}", callsign),
            Self::ModeS(mode_s) => write!(f, "mode_s::{}", mode_s),
            Self::RateLimit(ip) => write!(f, "ratelimit::{}", ip),
        }
    }
}
