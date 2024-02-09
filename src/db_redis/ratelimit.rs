use crate::{api::AppError, db_redis::RedisKey};
use fred::{clients::RedisPool, interfaces::KeysInterface};
use std::net::IpAddr;

pub struct RateLimit {
    key: String,
}

const UPPER_LIMIT: u64 = 1024;
const LOWER_LIMIT: u64 = 512;

const ONE_MINUTE: i64 = 60;

impl RateLimit {
    pub fn new(ip: IpAddr) -> Self {
        Self {
            key: RedisKey::RateLimit(ip).to_string(),
        }
    }

    /// Check if request has been rate limited, always increases the current value of the given rate limit
    pub async fn check(&self, redis: &RedisPool) -> Result<(), AppError> {
        if let Some(count) = redis.get::<Option<u64>, &str>(&self.key).await? {
            redis.incr(&self.key).await?;
            if count >= UPPER_LIMIT {
                tracing::info!("{} - {count}", self.key);
                redis.expire(&self.key, ONE_MINUTE * 5).await?;
            }
            if count > LOWER_LIMIT {
                return Err(AppError::RateLimited(
                    redis.ttl::<i64, &str>(&self.key).await?,
                ));
            }
            if count == LOWER_LIMIT {
                redis.expire(&self.key, ONE_MINUTE).await?;
                return Err(AppError::RateLimited(ONE_MINUTE));
            }
        } else {
            redis.incr(&self.key).await?;
            redis.expire(&self.key, ONE_MINUTE).await?;
        }
        Ok(())
    }
}
