use crate::{api::AppError, parse_env::AppEnv};
use redis::{
    aio::Connection, from_redis_value, AsyncCommands, ConnectionAddr, ConnectionInfo, ErrorKind,
    RedisConnectionInfo, RedisError, Value,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt, net::IpAddr, sync::Arc};
use tokio::sync::Mutex;

const ONE_WEEK: usize = 60 * 60 * 24 * 7;

// See if give value is in cache, if so, extend ttl, and deserialize into T
// pub async fn get_cache<T: DeserializeOwned + FromRedisValue>(
//     con: &Arc<Mutex<Connection>>,
//     key: &RedisKey,
// ) -> Result<Option<T>, AppError> {
//     let p: Option<T> = con.lock().await.get(key.to_string()).await?;
//     if let Some(result) = p {
//         // extend ttl if cache exists
//         con.lock().await.expire(key.to_string(), ONE_WEEK).await?;
//         Ok(Some(result))
//     } else {
//         Ok(None)
//     }
// }

// Convert a redis string result into a struct/None
// If the value is nil, returns as Some("none")
fn optional_null<T: DeserializeOwned>(v: &Value) -> Result<Option<T>, AppError> {
    let valid: Result<String, RedisError> = from_redis_value(v);
    match valid {
        Ok(valid_string) => {
            let data: T = serde_json::from_str(&valid_string)?;
            // This can be either "null" or a Model
            Ok(Some(data))
        }
        Err(e) => match e.kind() {
            ErrorKind::TypeError => Ok(None),
            _ => Err(AppError::RedisError(e)),
        },
    }
}

/// See if give value is in cache, if so, extend ttl, and deserialize into T
pub async fn get_cache<T: DeserializeOwned>(
    con: &Arc<Mutex<Connection>>,
    key: &RedisKey,
) -> Result<Option<T>, AppError> {
    let value: Value = con.lock().await.get(key.to_string()).await?;
    let serialized_data: Option<T> = optional_null(&value)?;
    // Can either by "null" or a model struct,
    if serialized_data.is_some() {
        con.lock().await.expire(key.to_string(), ONE_WEEK).await?;
    }
    Ok(serialized_data)
}

pub async fn insert_cache<T: Serialize>(
    con: &Arc<Mutex<Connection>>,
    to_insert: &T,
    key: &RedisKey,
) -> Result<(), AppError> {
    let value = serde_json::to_string(&to_insert)?;
    con.lock().await.set(key.to_string(), value).await?;
    con.lock().await.expire(key.to_string(), ONE_WEEK).await?;
    Ok(())
}

/// Check if rate limited, will return true if so
pub async fn check_rate_limit(con: &Arc<Mutex<Connection>>, key: RedisKey) -> Result<(), AppError> {
    let count: Option<usize> = con.lock().await.get(key.to_string()).await?;
    con.lock().await.incr(key.to_string(), 1).await?;

    // Only increasing ttl if NOT already blocked
    // Has to be -1 of whatever limit you want, as first request doesn't count
    if let Some(i) = count {
        // If bigger than 240, rate limit for 5 minutes
        if i >= 240 {
            con.lock().await.expire(key.to_string(), 60 * 5).await?;
            let ttl: usize = con.lock().await.ttl(key.to_string()).await?;
            return Err(AppError::RateLimited(ttl));
        }
        if i >= 120 {
            let ttl: usize = con.lock().await.ttl(key.to_string()).await?;
            return Err(AppError::RateLimited(ttl));
        };
    }
    con.lock().await.expire(key.to_string(), 60).await?;
    Ok(())
}

pub async fn get_connection(app_env: &AppEnv) -> Result<Connection, AppError> {
    let connection_info = ConnectionInfo {
        redis: RedisConnectionInfo {
            db: app_env.redis_database as i64,
            password: Some(app_env.redis_password.to_owned()),
            username: None,
        },
        addr: ConnectionAddr::Tcp(app_env.redis_host.to_owned(), app_env.redis_port),
    };
    let client = redis::Client::open(connection_info)?;
    let con = client.get_async_connection().await?;
    Ok(con)
}

#[derive(Debug, Clone)]
pub enum RedisKey {
    Callsign(String),
    ModeS(String),
    RateLimit(IpAddr),
}

impl fmt::Display for RedisKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let disp = match self {
            Self::Callsign(callsign) => format!("callsign::{}", callsign),
            Self::ModeS(mode_s) => format!("mode_s::{}", mode_s),
            Self::RateLimit(ip) => format!("ratelimit::{}", ip),
        };
        write!(f, "{}", disp)
    }
}

// TODO test me
