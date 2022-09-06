use crate::{api::AppError, parse_env::AppEnv};
use redis::{
    aio::Connection, from_redis_value, AsyncCommands, ConnectionAddr, ConnectionInfo, ErrorKind,
    RedisConnectionInfo, RedisError, Value,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt, net::IpAddr, sync::Arc, time::Duration};
use tokio::sync::Mutex;

const ONE_WEEK: usize = 60 * 60 * 24 * 7;
const FIELD: &str = "data";

// Convert a redis string result into a struct/None
// If the value is null, returns as Some("none")
fn optional_null<T: DeserializeOwned>(v: &Value) -> Result<Option<T>, AppError> {
    let valid: Result<String, RedisError> = from_redis_value(v);
    match valid {
        Ok(valid_string) => {
            let data: T = serde_json::from_str(&valid_string)?;
            // This can be either "null" or a Model struct
            Ok(Some(data))
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
	let key =key.to_string();
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
    let value = serde_json::to_string(&to_insert)?;
    redis.lock().await.hset(key.to_string(), FIELD, value).await?;
    redis.lock().await.expire(key.to_string(), ONE_WEEK).await?;
    Ok(())
}

/// Check if rate limited, will return true if so
pub async fn check_rate_limit(redis: &Arc<Mutex<Connection>>, key: RedisKey) -> Result<(), AppError> {
	let key = key.to_string();
    let count: Option<usize> = redis.lock().await.get(&key).await?;
    redis.lock().await.incr(&key, 1).await?;

    // Only increasing ttl if NOT already blocked
    // Has to be -1 of whatever limit you want, as first request doesn't count
    if let Some(i) = count {
        // If bigger than 240, rate limit for 5 minutes
        if i >= 240 {
            redis.lock().await.expire(&key, 60 * 5).await?;
            let ttl: usize = redis.lock().await.ttl(key.to_string()).await?;
            return Err(AppError::RateLimited(ttl));
        }
        if i > 120 {
            let ttl: usize = redis.lock().await.ttl(&key).await?;
            return Err(AppError::RateLimited(ttl));
        };
        if i == 120 {
            redis.lock().await.expire(&key, 60).await?;
            return Err(AppError::RateLimited(60));
        }
    } else {
        redis.lock().await.expire(&key, 60).await?;
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
