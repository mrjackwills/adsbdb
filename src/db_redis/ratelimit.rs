use crate::{api::AppError, db_redis::RedisKey};
use fred::{clients::Pool, interfaces::KeysInterface};
use std::net::IpAddr;

pub struct RateLimit {
    key: String,
}

const UPPER_LIMIT: usize = 1024;
const LOWER_LIMIT: usize = 512;

const ONE_MINUTE_AS_SEC: i64 = 60;
const FIVE_MINUTES_AS_SEC: i64 = ONE_MINUTE_AS_SEC * 5;

impl RateLimit {
    pub fn new(ip: IpAddr) -> Self {
        Self {
            key: RedisKey::RateLimit(ip).to_string(),
        }
    }

    /// Check if request has been rate limited, always increases the current value of the given rate limit
    pub async fn check(&self, redis: &Pool) -> Result<(), AppError> {
        let count = redis.incr::<usize, _>(&self.key).await?.saturating_sub(1);
        if count == 0 {
            redis
                .expire::<i64, &String>(&self.key, ONE_MINUTE_AS_SEC, None)
                .await?;
            return Ok(());
        }

        if count >= UPPER_LIMIT {
            // Only show the count if is multiple of the upper limit
            if count % UPPER_LIMIT == 0 {
                tracing::info!("{} - {count}", self.key);
            }
            redis
                .expire::<(), &str>(&self.key, FIVE_MINUTES_AS_SEC, None)
                .await?;
        }
        if count > LOWER_LIMIT {
            return Err(AppError::RateLimited(
                redis.ttl::<i64, &str>(&self.key).await?,
            ));
        }
        if count == LOWER_LIMIT {
            redis
                .expire::<i64, &String>(&self.key, ONE_MINUTE_AS_SEC, None)
                .await?;
            return Err(AppError::RateLimited(ONE_MINUTE_AS_SEC));
        }
        Ok(())
    }
}
