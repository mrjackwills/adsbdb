use crate::{api::AppError, db_redis::RedisKey};
use redis::{aio::Connection, AsyncCommands};
use std::{net::IpAddr, sync::Arc};
use tokio::sync::Mutex;
use tracing::info;

pub struct RateLimit;

const ONE_MINUTE: usize = 60;

// TODO put rate limits in the app_env, would need tests to react to this

impl RateLimit {
    fn get_key(ip: IpAddr) -> String {
        RedisKey::RateLimit(ip).to_string()
    }

    /// Check an incoming request to see if it is ratelimited or not
    pub async fn check(redis: &Arc<Mutex<Connection>>, ip: IpAddr) -> Result<(), AppError> {
        let key = Self::get_key(ip);
        let count = redis.lock().await.get::<&str, Option<usize>>(&key).await?;
        redis.lock().await.incr(&key, 1).await?;
        if let Some(count) = count {
            if count >= 1024 {
                info!("{key} - {count}");
                redis.lock().await.expire(&key, ONE_MINUTE * 5).await?;
            }
            if count > 512 {
                return Err(AppError::RateLimited(
                    usize::try_from(redis.lock().await.ttl::<&str, isize>(&key).await?)
                        .unwrap_or_default(),
                ));
            };
            if count == 512 {
                redis.lock().await.expire(&key, ONE_MINUTE).await?;
                return Err(AppError::RateLimited(ONE_MINUTE));
            }
        } else {
            redis.lock().await.expire(&key, ONE_MINUTE).await?;
        }
        Ok(())
    }
}
