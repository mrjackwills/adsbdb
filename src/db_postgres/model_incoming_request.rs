use axum::{
    body::Body,
    http::{Request, Uri, request::Parts},
};
use fred::interfaces::{KeysInterface, SortedSetsInterface};
use fred::prelude::Pool;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    api::{AppError, Stats, StatsEntry},
    db_postgres::ID,
    db_redis::{
        IncomingRequestKey, ONE_DAY_AS_SEC, ONE_MINUTE_AS_SEC, RedisKey, get_cache, insert_cache,
    },
    generic_id, redis_hash_to_struct,
};
use std::collections::HashMap;

pub const RE_SEED_TIME: i64 = ONE_MINUTE_AS_SEC.wrapping_mul(5);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UriMethod(Uri, Method);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SplitUri {
    version: Option<String>,
    path: Option<String>,
    query: Option<String>,
}

impl SplitUri {
    fn build_url(&self) -> String {
        let segments: Vec<String> = [&self.version, &self.path, &self.query]
            .into_iter()
            .flatten()
            .filter(|i| !i.is_empty())
            .map(|s| s.to_owned())
            .collect();
        format!("/{}", segments.join("/"))
    }
}

impl From<&UriMethod> for SplitUri {
    fn from(value: &UriMethod) -> Self {
        let url = value
            .0
            .to_string()
            .strip_prefix('/')
            .unwrap_or_default()
            .to_owned();

        let mut parts = url.splitn(3, '/').map(|i| Some(i.to_owned()));
        Self {
            version: parts.next().flatten(),
            path: parts.next().flatten(),
            query: parts.next().flatten(),
        }
    }
}

impl From<&Request<Body>> for UriMethod {
    fn from(value: &Request<Body>) -> Self {
        Self(value.uri().clone(), value.method().clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MsgIncomingRequest {
    Url(UriMethod),
}

impl From<&Parts> for MsgIncomingRequest {
    fn from(value: &Parts) -> Self {
        Self::Url(UriMethod(value.uri.clone(), value.method.clone()))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct EntryCount {
    url: String,
    count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Count {
    count: i64,
}

redis_hash_to_struct!(Stats);

pub struct ModelIncomingRequest;

// Only using types here, as the sqlx macro doesn't like generic types
type VId = ID<VersionID>;
type QId = ID<QueryID>;
type PId = ID<PathID>;

generic_id!(VersionID);
generic_id!(PathID);
generic_id!(QueryID);

/// postgres, column, uses "incoming_request" table
macro_rules! fetch_single_stats {
    ($pg:expr, $path:expr) => {
        sqlx::query_as!(
            EntryCount,
r#"SELECT
    '/' || CONCAT_WS(
        '/',
        NULLIF(iruv.url_version, ''),
        NULLIF(irup.url_path, ''),
        NULLIF(iruq.url_query, '')
    ) AS "url!",
    SUM(COALESCE(ir.count, 0))::BIGINT AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = ir.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = ir.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = ir.incoming_request_url_query_id
WHERE irup.url_path = $1
GROUP BY
    '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 1"#,$path
        )
        .fetch_all($pg)
    };
}

/// postgres, column, uses "incoming_request" table
macro_rules! fetch_stats {
    ($pg:expr, $path:expr) => {
        sqlx::query_as!(
            EntryCount,
            r#"
WITH counts AS (
    SELECT
        ir.incoming_request_url_version_id,
        ir.incoming_request_url_path_id,
        ir.incoming_request_url_query_id,
        SUM(COALESCE(ir.count, 0))::BIGINT AS url_count
    FROM incoming_request ir
    JOIN incoming_request_url_path irup
        ON irup.incoming_request_url_path_id = ir.incoming_request_url_path_id
    WHERE irup.url_path = $1
    GROUP BY
        ir.incoming_request_url_version_id,
        ir.incoming_request_url_path_id,
        ir.incoming_request_url_query_id
    ORDER BY url_count DESC
    LIMIT 10
)
SELECT
    '/' || CONCAT_WS(
        '/',
        NULLIF(iruv.url_version, ''),
        NULLIF(irup.url_path, ''),
        NULLIF(iruq.url_query, '')
    ) AS "url!",
    c.url_count AS "count!"
FROM counts c
LEFT JOIN incoming_request_url_version iruv
    ON iruv.incoming_request_url_version_id = c.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup
    ON irup.incoming_request_url_path_id = c.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq
    ON iruq.incoming_request_url_query_id = c.incoming_request_url_query_id
ORDER BY c.url_count DESC, "url!""#,
            $path
        )
        .fetch_all($pg)
    };
}

/// Current time as an epoch-minute, so that time buckets are distinct across days and TZ-free
fn epoch_minute() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
    .unwrap_or_default()
    .saturating_div(ONE_MINUTE_AS_SEC)
}

/// Increment the daily stats for a given request
async fn increment_temp_stat(redis: &Pool, split_url: SplitUri) -> Result<(), AppError> {
    let minute = epoch_minute();
    let full_url = split_url.build_url();

    if let Some(path) = split_url.path.filter(|p| !p.is_empty()) {
        let path_key = RedisKey::TempIR((&path, minute)).to_string();
        let exists = redis.exists::<bool, _>(&path_key).await?;
        redis.zincrby::<(), _, _>(&path_key, 1.0, &full_url).await?;
        if !exists {
            redis
                .expire::<(), _>(&path_key, ONE_DAY_AS_SEC, None)
                .await?;
        }
    }

    let count_key = RedisKey::TempIRCount(minute).to_string();
    let exists = redis.exists::<bool, _>(&count_key).await?;
    redis.zincrby::<(), _, _>(&count_key, 1.0, full_url).await?;
    if !exists {
        redis
            .expire::<(), _>(&count_key, ONE_DAY_AS_SEC, None)
            .await?;
    }

    Ok(())
}

/// Aggregate the top `limit` url counts for a path
async fn fetch_temp_stats_limit(
    redis: &Pool,
    path: &str,
    limit: usize,
) -> Result<Vec<EntryCount>, AppError> {
    let minute = epoch_minute();
    let mut counts = HashMap::new();
    for minute in (minute - 1439)..=minute {
        let key = RedisKey::TempIR((path, minute)).to_string();
        let entries = redis
            .zrevrange::<Vec<(String, i64)>, &str>(&key, 0, -1, true)
            .await?;
        for (url, count) in entries {
            *counts.entry(url).or_default() += count
        }
    }
    let mut out = counts
        .into_iter()
        .map(|(url, count)| EntryCount { url, count })
        .collect::<Vec<EntryCount>>();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.url.cmp(&b.url)));
    out.truncate(limit);
    Ok(out)
}

async fn fetch_temp_stats(redis: &Pool, path: &str) -> Result<Vec<EntryCount>, AppError> {
    fetch_temp_stats_limit(redis, path, 10).await
}

async fn fetch_temp_single_stats(redis: &Pool, path: &str) -> Result<Vec<EntryCount>, AppError> {
    fetch_temp_stats_limit(redis, path, 1).await
}

/// Sum the aggregate request counts over the last 24 hours of minute-buckets
async fn get_aggregate_count(redis: &Pool) -> Result<i64, AppError> {
    let now_minute = epoch_minute();
    let mut total = 0.0;
    for minute in (now_minute - 1439)..=now_minute {
        let key = RedisKey::TempIRCount(minute).to_string();
        let entries = redis
            .zrevrange::<Vec<(String, f64)>, &str>(&key, 0, -1, true)
            .await?;
        for (_, count) in entries {
            total += count
        }
    }
    // TODO fix this
    Ok(total as i64)
}

impl ModelIncomingRequest {
    /// As I can't be bothered/know how to change the postgres query macro to allow a definable limit
    /// just use this function to cut certain url stats to a single item, used for  /stats & /online
    fn single_entry_count(input: Vec<EntryCount>) -> Vec<EntryCount> {
        input.into_iter().take(1).collect()
    }

    async fn get_version_id(
        url_version: Option<&str>,
        postgres: &PgPool,
        redis: &Pool,
    ) -> Result<Option<VersionID>, AppError> {
        Ok(if let Some(url_version) = url_version {
            let key = RedisKey::IncomingRequest(IncomingRequestKey::Version(url_version));
            if let Some(Some(id)) = get_cache::<VersionID>(redis, &key).await? {
                return Ok(Some(id));
            }

            let id = sqlx::query_as!(
                VId,
                r#"
INSERT INTO
    incoming_request_url_version (url_version)
VALUES
    ($1)
ON CONFLICT
    (url_version)
DO UPDATE SET
    url_version = EXCLUDED.url_version
RETURNING
    incoming_request_url_version_id AS id;"#,
                url_version
            )
            .fetch_one(postgres)
            .await?
            .id;

            insert_cache::<VersionID>(redis, Some(&id), key).await?;
            return Ok(Some(id));
        } else {
            None
        })
    }

    async fn get_path_id(
        url_path: Option<&str>,
        postgres: &PgPool,
        redis: &Pool,
    ) -> Result<Option<PathID>, AppError> {
        Ok(if let Some(url_path) = url_path {
            let key = RedisKey::IncomingRequest(IncomingRequestKey::Path(url_path));

            if let Some(Some(id)) = get_cache::<PathID>(redis, &key).await? {
                return Ok(Some(id));
            }

            let id = sqlx::query_as!(
                PId,
                r#"
INSERT INTO
    incoming_request_url_path (url_path)
VALUES
    ($1)
ON CONFLICT
    (url_path)
DO UPDATE SET
    url_path = EXCLUDED.url_path
RETURNING
    incoming_request_url_path_id AS id;"#,
                url_path
            )
            .fetch_one(postgres)
            .await?
            .id;

            insert_cache::<PathID>(redis, Some(&id), key).await?;
            return Ok(Some(id));
        } else {
            None
        })
    }

    async fn get_query_id(
        url_query: Option<&str>,
        postgres: &PgPool,
        redis: &Pool,
    ) -> Result<Option<QueryID>, AppError> {
        Ok(if let Some(url_query) = url_query {
            let key = RedisKey::IncomingRequest(IncomingRequestKey::Query(url_query));
            if let Some(Some(id)) = get_cache::<QueryID>(redis, &key).await? {
                return Ok(Some(id));
            }

            let id = sqlx::query_as!(
                QId,
                r#"
INSERT INTO
    incoming_request_url_query (url_query)
VALUES
    ($1)
ON CONFLICT
    (url_query)
DO UPDATE SET
    url_query = EXCLUDED.url_query
RETURNING
    incoming_request_url_query_id AS id;"#,
                url_query
            )
            .fetch_one(postgres)
            .await?
            .id;

            insert_cache::<QueryID>(redis, Some(&id), key).await?;
            return Ok(Some(id));
        } else {
            None
        })
    }

    async fn insert_request(
        postgres: &PgPool,
        redis: &Pool,
        url: UriMethod,
    ) -> Result<(), AppError> {
        let split_url = SplitUri::from(&url);

        let (version_id, path_id, query_id) = tokio::try_join!(
            Self::get_version_id(split_url.version.as_deref(), postgres, redis),
            Self::get_path_id(split_url.path.as_deref(), postgres, redis),
            Self::get_query_id(split_url.query.as_deref(), postgres, redis)
        )?;

        let mut tx = postgres.begin().await?;
        sqlx::query!(
            r#"
INSERT INTO incoming_request (
    incoming_request_url_version_id,
    incoming_request_url_path_id,
    incoming_request_url_query_id,
    request_method
    )
VALUES
    ( $1, $2, $3, ($4::text)::request_method)
ON CONFLICT ON CONSTRAINT incoming_request_pkey
DO UPDATE SET
    count = incoming_request.count + 1;"#,
            version_id.map(|i| i.get()),
            path_id.map(|i| i.get()),
            query_id.map(|i| i.get()),
            url.1.to_string()
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
INSERT INTO request_total (id, total)
VALUES
    (1, 1)
ON CONFLICT
    (id)
DO UPDATE SET
    total = request_total.total + 1;"#
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        increment_temp_stat(redis, split_url).await?;
        Ok(())
    }

    /// Return stats for aircraft & flightroutes for previous 24 hours
    async fn get_daily(redis: &Pool) -> Result<StatsEntry, AppError> {
        let (aircraft, airline, callsign, mode_s, n_number, online, stats, aggregate) = tokio::try_join!(
            fetch_temp_stats(redis, "aircraft"),
            fetch_temp_stats(redis, "airline"),
            fetch_temp_stats(redis, "callsign"),
            fetch_temp_stats(redis, "mode-s"),
            fetch_temp_stats(redis, "n-number"),
            fetch_temp_single_stats(redis, "online"),
            fetch_temp_single_stats(redis, "stats"),
            get_aggregate_count(redis),
        )?;
        Ok(StatsEntry {
            aircraft,
            airline,
            callsign,
            mode_s,
            n_number,
            online,
            stats,
            aggregate,
        })
    }

    /// Return stats for aircraft & flightroutes for previous 24 hours
    #[allow(unused)]
    async fn get_total(postgres: &PgPool) -> Result<StatsEntry, AppError> {
        let (aircraft, airline, callsign, mode_s, n_number, online, stats, aggregate) = tokio::try_join!(
            fetch_stats!(postgres, "aircraft"),
            fetch_stats!(postgres, "airline"),
            fetch_stats!(postgres, "callsign"),
            fetch_stats!(postgres, "mode-s"),
            fetch_stats!(postgres, "n-number"),
            fetch_single_stats!(postgres, "online"),
            fetch_single_stats!(postgres, "stats"),
            sqlx::query_as!(
                Count,
                r#"SELECT COALESCE(MAX(total), 0) AS "count!" FROM request_total WHERE id = 1;"#
            )
            .fetch_one(postgres)
        )?;

        Ok(StatsEntry {
            aircraft,
            airline,
            callsign,
            mode_s,
            n_number,
            online: Self::single_entry_count(online),
            stats: Self::single_entry_count(stats),
            aggregate: aggregate.count,
        })
    }

    /// This is slow
    async fn seed_redis(postgres: &PgPool, redis: &Pool) -> Result<(), AppError> {
        let statistics = Self::get_daily_total_postgres(postgres, redis).await?;
        insert_cache(redis, Some(&statistics), RedisKey::Stats).await?;
        Ok(())
    }

    #[cfg(test)]
    /// Get usage stats - For testing just return same values for daily and total, else the tests are inordinately slow
    async fn get_daily_total_postgres(_postgres: &PgPool, redis: &Pool) -> Result<Stats, AppError> {
        let daily = Self::get_daily(redis).await?;
        Ok(Stats {
            daily: daily.clone(),
            total: daily,
        })
    }
    #[cfg(not(test))]
    /// Get usage stats - the total is a slow query
    async fn get_daily_total_postgres(postgres: &PgPool, redis: &Pool) -> Result<Stats, AppError> {
        let daily = Self::get_daily(redis).await?;
        let total = Self::get_total(postgres).await?;
        Ok(Stats { daily, total })
    }

    pub async fn get_stats(postgres: &PgPool, redis: &Pool) -> Result<Stats, AppError> {
        if let Some(Some(stats)) = get_cache::<Stats>(redis, &RedisKey::Stats).await? {
            Ok(stats)
        } else {
            let stats = Self::get_daily_total_postgres(postgres, redis).await?;
            insert_cache(redis, Some(&stats), RedisKey::Stats).await?;
            Ok(stats)
        }
    }

    /// Check if the stats need to be re-seeded into Redis
    /// If so, will be spawned into new tokio thread
    /// RE_SEED_TIME is vastly reduced when testing
    fn check_to_re_seed(now: &mut std::time::Instant, postgres: &PgPool, redis: &Pool) {
        // TODO should calc the time it takes to reseed, and then minus that from re_sseed time?
        if now.elapsed().as_secs() >= u64::try_from(RE_SEED_TIME).unwrap_or_default() {
            *now = std::time::Instant::now();
            let (postgres, redis) = (postgres.clone(), redis.clone());
            tokio::spawn(async move {
                if let Err(e) = Self::seed_redis(&postgres, &redis).await {
                    tracing::error!("{e:?}");
                }
            });
        }
    }

    /// Create a message handler on it's own tokio thread, and return it's message sender
    /// Will insert request_statistics on each message received
    /// Will insert cache stats at interval RE_SEED_TIME - assuming it has recieved any messages at all in that time period
    /// As the /online route gets checked via Docker, we can assume atleast single message every 60 seconds
    pub async fn start(
        postgres: &PgPool,
        redis: &Pool,
    ) -> Result<async_channel::Sender<MsgIncomingRequest>, AppError> {
        Self::seed_redis(postgres, redis).await?;
        let (tx, rx) = async_channel::bounded(8192);
		let postgres = postgres.clone();
		let redis = redis.clone();
        tokio::spawn(async move {
            let mut now = std::time::Instant::now();
            while let Ok(msg) = rx.recv().await {
                if let Err(e) = match msg {
                    MsgIncomingRequest::Url(i) => Self::insert_request(&postgres, &redis, i).await,
                } {
                    tracing::error!("{e:?}");
                }
                Self::check_to_re_seed(&mut now, &postgres, &redis);
            }
        });

        Ok(tx)
    }
}
