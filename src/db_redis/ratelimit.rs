use crate::{api::AppError, db_redis::RedisKey};
use redis::{aio::Connection, AsyncCommands};
use std::{net::IpAddr, sync::Arc};
use tokio::sync::{Mutex, MutexGuard};
use tracing::info;

pub struct RateLimit {
    key: String,
}

const UPPER_LIMIT: u64 = 1024;
const LOWER_LIMIT: u64 = 512;

const ONE_MINUTE: usize = 60;

impl RateLimit {
    pub fn new(ip: IpAddr) -> Self {
        Self {
            key: RedisKey::RateLimit(ip).to_string(),
        }
    }

    /// Get current rate limit count
    async fn get_count(
        &self,
        redis: &mut MutexGuard<'_, Connection>,
    ) -> Result<Option<u64>, AppError> {
        Ok(redis.get::<&str, Option<u64>>(&self.key).await?)
    }

    /// Get the ttl for a given limiter, converts from the redis isize to usize
    async fn ttl(&self, redis: &mut MutexGuard<'_, Connection>) -> Result<usize, AppError> {
        Ok(usize::try_from(redis.ttl::<&str, isize>(&self.key).await?).unwrap_or_default())
    }

    /// Check if request has been rate limited, always increases the current value of the given rate limit
    pub async fn check(&self, redis: &Arc<Mutex<Connection>>) -> Result<(), AppError> {
        let mut redis = redis.lock().await;
        if let Some(count) = self.get_count(&mut redis).await? {
            redis.incr(&self.key, 1).await?;
            if count >= UPPER_LIMIT {
                info!("{} - {count}", self.key);
                redis.expire(&self.key, ONE_MINUTE * 5).await?;
            }
            if count > LOWER_LIMIT {
                return Err(AppError::RateLimited(self.ttl(&mut redis).await?));
            }
            if count == LOWER_LIMIT {
                redis.expire(&self.key, ONE_MINUTE).await?;
                return Err(AppError::RateLimited(ONE_MINUTE));
            }
        } else {
            redis.incr(&self.key, 1).await?;
            redis.expire(&self.key, ONE_MINUTE).await?;
        }
        drop(redis);
        Ok(())
    }
}
