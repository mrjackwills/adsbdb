use crate::{api::AppError, db_redis::RedisKey};
use fred::{clients::Pool, interfaces::KeysInterface};
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
    pub async fn check(&self, redis: &Pool) -> Result<(), AppError> {
        if let Some(count) = redis.get::<Option<u64>, &str>(&self.key).await? {
            redis.incr::<(), _>(&self.key).await?;
            if count >= UPPER_LIMIT {
                // Only show the count if is multiple of the upper limit
                if count % UPPER_LIMIT == 0 {
                    tracing::info!("{} - {count}", self.key);
                }
                redis
                    .expire::<(), &str>(&self.key, ONE_MINUTE * 5, None)
                    .await?;
            }
            if count > LOWER_LIMIT {
                return Err(AppError::RateLimited(
                    redis.ttl::<i64, &str>(&self.key).await?,
                ));
            }
            if count == LOWER_LIMIT {
                redis
                    .expire::<i64, &String>(&self.key, ONE_MINUTE, None)
                    .await?;
                return Err(AppError::RateLimited(ONE_MINUTE));
            }
        } else {
            redis.incr::<(), _>(&self.key).await?;
            redis
                .expire::<i64, &String>(&self.key, ONE_MINUTE, None)
                .await?;
        }
        Ok(())
    }
}
