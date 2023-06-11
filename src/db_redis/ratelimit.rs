use super::RedisKey;
use crate::api::AppError;
use redis::{aio::Connection, AsyncCommands};
use std::{net::IpAddr, sync::Arc};
use tokio::sync::Mutex;
use tracing::info;

pub struct RateLimit;

const ONE_MINUTE: usize = 60;

impl RateLimit {
    fn get_key(ip: IpAddr) -> String {
        RedisKey::RateLimit(ip).to_string()
    }

    /// Check an incoming request to see if it is ratelimited or not
    pub async fn check(redis: &Arc<Mutex<Connection>>, ip: IpAddr) -> Result<(), AppError> {
        let key = Self::get_key(ip);
        let mut redis = redis.lock().await;
        let count = redis.get::<&str, Option<usize>>(&key).await?;
        redis.incr(&key, 1).await?;
        if let Some(count) = count {
            if count >= 240 {
                info!("{key} - {count}");
                redis.expire(&key, ONE_MINUTE * 5).await?;
            }
            if count > 120 {
                return Err(AppError::RateLimited(
                    usize::try_from(redis.ttl::<&str, isize>(&key).await?).unwrap_or_default(),
                ));
            };
            if count == 120 {
                redis.expire(&key, ONE_MINUTE).await?;
                return Err(AppError::RateLimited(ONE_MINUTE));
            }
        } else {
            redis.expire(&key, ONE_MINUTE).await?;
        }
        // nursery lint suggest using a drop here?
        drop(redis);
        Ok(())
    }
}
