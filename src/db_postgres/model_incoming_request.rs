use axum::{
    body::Body,
    http::{Request, Uri, request::Parts},
};
use fred::prelude::Pool;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sqlx::{PgExecutor, PgPool};

use crate::{
    api::{AppError, Stats, StatsEntry},
    db_postgres::ID,
    db_redis::{IncomingRequestKey, ONE_MINUTE_AS_SEC, RedisKey, get_cache, insert_cache},
    generic_id, redis_hash_to_struct,
};

pub const RE_SEED_TIME: i64 = ONE_MINUTE_AS_SEC.wrapping_mul(5);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UriMethod(Uri, Method);

impl UriMethod {
    /// Split an url into three optional parts, (version, path, query), split on '/' char
    fn split_into_parts(&self) -> (Option<String>, Option<String>, Option<String>) {
        let url = self
            .0
            .to_string()
            .strip_prefix('/')
            .unwrap_or_default()
            .to_owned();

        let mut parts = url.splitn(3, '/').map(|i| Some(i.to_owned()));
        (
            parts.next().flatten(),
            parts.next().flatten(),
            parts.next().flatten(),
        )
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
    // TODO reseed time?
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
type IRId = ID<IncomingRequestID>;

generic_id!(VersionID);
generic_id!(PathID);
generic_id!(QueryID);

generic_id!(IncomingRequestID);

/// postgres, column, uses "temp_incoming_request" table
macro_rules! fetch_temp_stats {
    ($pg:expr, $path:expr) => {
        sqlx::query_as!(
            EntryCount,
            r#"
WITH url_counts AS (
    SELECT
        tir.incoming_request_url_id,
        SUM(tir.count) AS total_count
    FROM temp_incoming_request tir
    JOIN incoming_request_url iru ON iru.incoming_request_url_id = tir.incoming_request_url_id
    JOIN incoming_request_url_path irup ON irup.incoming_request_url_path_id = iru.incoming_request_url_path_id
    WHERE irup.url_path = $1
    GROUP BY tir.incoming_request_url_id
    ORDER BY total_count DESC
    LIMIT 10
)
SELECT
    '/' || CONCAT_WS(
        '/',
        NULLIF(iruv.url_version, ''),
        NULLIF(irup.url_path, ''),
        NULLIF(iruq.url_query, '')
    ) AS "url!",
    uc.total_count AS "count!"
FROM url_counts uc
JOIN incoming_request_url iru ON iru.incoming_request_url_id = uc.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
JOIN incoming_request_url_path irup ON irup.incoming_request_url_path_id = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
ORDER BY uc.total_count DESC, "url!""#,$path
        )
        .fetch_all($pg)
    };
}

// Used for /stats and /online, else the grouping get's messed up at results in a count of 1
// Need to work out how to correctly combine both queries so that we can use a single macro
macro_rules! fetch_temp_single_stats {
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
    SUM(COALESCE(tir.count, 0)) AS "count!"
FROM temp_incoming_request tir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = tir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = $1
GROUP BY
    '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 1"#,$path
        )
        .fetch_all($pg)
    };
}

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
    SUM(COALESCE(ir.count, 0)) AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = ir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
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
        ir.incoming_request_url_id,
        SUM(COALESCE(ir.count, 0)) AS url_count
    FROM incoming_request ir
    JOIN incoming_request_url iru
        ON iru.incoming_request_url_id = ir.incoming_request_url_id
    JOIN incoming_request_url_path irup
        ON irup.incoming_request_url_path_id = iru.incoming_request_url_path_id
    WHERE irup.url_path = $1
    GROUP BY ir.incoming_request_url_id
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
JOIN incoming_request_url iru
    ON iru.incoming_request_url_id = c.incoming_request_url_id
JOIN incoming_request_url_path irup
    ON irup.incoming_request_url_path_id = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_version iruv
    ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_query iruq
    ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
ORDER BY c.url_count DESC, "url!""#,
            $path
        )
        .fetch_all($pg)
    };
}

impl ModelIncomingRequest {
    /// As I can't be bothered/know how to change the postgres query macro to allow a definable limit
    /// just use this function to cut certain url stats to a single item, used for  /stats & /online
    fn single_entry_count(input: Vec<EntryCount>) -> Vec<EntryCount> {
        input.into_iter().take(1).collect()
    }

    // Argh, TODO cache all of these!
    async fn get_version_id(
        url_version: Option<String>,
        postgres: &PgPool,
        redis: &Pool,
    ) -> Result<Option<VersionID>, AppError> {
        Ok(if let Some(url_version) = url_version {
            let key = RedisKey::IncomingRequest(IncomingRequestKey::Version(&url_version));
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

    // All these should be cached in redis, just to get ID's, can cache with a ttl of 1 week

    async fn get_path_id(
        url_path: Option<String>,
        postgres: &PgPool,
        redis: &Pool,
    ) -> Result<Option<PathID>, AppError> {
        Ok(if let Some(url_path) = url_path {
            let key = RedisKey::IncomingRequest(IncomingRequestKey::Path(&url_path));

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
        url_query: Option<String>,
        postgres: &PgPool,
        redis: &Pool,
    ) -> Result<Option<QueryID>, AppError> {
        Ok(if let Some(url_query) = url_query {
            let key = RedisKey::IncomingRequest(IncomingRequestKey::Query(&url_query));
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

    async fn insert_request_url(
        postgres: &PgPool,
        redis: &Pool,
        version_id: Option<VersionID>,
        path_id: Option<PathID>,
        query_id: Option<QueryID>,
    ) -> Result<IncomingRequestID, AppError> {
        let key = RedisKey::IncomingRequest(IncomingRequestKey::IncomingRequestUrl(
            version_id.as_ref(),
            path_id.as_ref(),
            query_id.as_ref(),
        ));

        if let Some(Some(id)) = get_cache::<IncomingRequestID>(redis, &key).await? {
            return Ok(id);
        }
        let id = sqlx::query_as!(
            IRId,
            r#"
INSERT INTO incoming_request_url (
    incoming_request_url_version_id,
    incoming_request_url_path_id, 
    incoming_request_url_query_id
)
VALUES
    ($1, $2, $3)
ON CONFLICT (
    incoming_request_url_version_id, 
    incoming_request_url_path_id, 
    incoming_request_url_query_id
) 
DO UPDATE SET 
    incoming_request_url_version_id = EXCLUDED.incoming_request_url_version_id
RETURNING
    incoming_request_url_id AS id;"#,
            version_id.map(|i| i.get()),
            path_id.map(|i| i.get()),
            query_id.map(|i| i.get())
        )
        .fetch_one(postgres)
        .await?
        .id;

        insert_cache::<IncomingRequestID>(redis, Some(&id), key).await?;
        Ok(id)
    }

    /// Insert the request url into database, this will recored every single request to the database
    async fn insert_request(
        postgres: &PgPool,
        redis: &Pool,
        url: UriMethod,
    ) -> Result<(), AppError> {
        let (url_version, url_path, url_query) = url.split_into_parts();

        let (version_id, path_id, query_id) = tokio::try_join!(
            Self::get_version_id(url_version, postgres, redis),
            Self::get_path_id(url_path, postgres, redis),
            Self::get_query_id(url_query, postgres, redis)
        )?;

        let request_id =
            Self::insert_request_url(postgres, redis, version_id, path_id, query_id).await?;

        tokio::try_join!(
            sqlx::query!(
                r#"
INSERT INTO incoming_request (
    incoming_request_url_id,
    request_method
    )
VALUES
    ( $1, ($2::text)::request_method)
ON CONFLICT
    (incoming_request_url_id, request_method)
DO UPDATE SET
    count = incoming_request.count + 1;"#,
                request_id.get(),
                url.1.to_string()
            )
            .execute(postgres),
            sqlx::query!(
                r#"
INSERT INTO temp_incoming_request (
    incoming_request_url_id,
    request_method
    )
VALUES
    ($1,($2::text)::request_method)
ON CONFLICT
    (incoming_request_url_id, request_method)
DO UPDATE SET
    count = temp_incoming_request.count + 1;"#,
                request_id.get(),
                url.1.to_string()
            )
            .execute(postgres)
        )?;
        Ok(())
    }

    /// Delete all entries from temp table older than 24 hours
    async fn delete_temp(db: impl PgExecutor<'_>) -> Result<(), AppError> {
        sqlx::query!("DELETE FROM temp_incoming_request WHERE timestamp <= (CURRENT_TIMESTAMP - INTERVAL '24 hours');").execute(db).await?;
        Ok(())
    }

    /// Return stats for aircraft & flightroutes for previous 24 hours
    /// TODO This is a slow, think 30 second, query, need to work on it
    /// Ideally should be using redis instead!
    #[allow(clippy::too_many_lines)]
    async fn get_daily(postgres: &PgPool) -> Result<StatsEntry, AppError> {
        let (aircraft, airline, callsign, mode_s, n_number, online, stats, aggregate) = tokio::try_join!(
            fetch_temp_stats!(postgres, "aircraft"),
            fetch_temp_stats!(postgres, "airline"),
            fetch_temp_stats!(postgres, "callsign"),
            fetch_temp_stats!(postgres, "mode-s"),
            fetch_temp_stats!(postgres, "n-number"),
            fetch_temp_single_stats!(postgres, "online"),
            fetch_temp_single_stats!(postgres, "stats"),
            sqlx::query_as!(
                Count,
                r#"SELECT COALESCE(SUM(count), 0) AS "count!" FROM temp_incoming_request;"#
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

    /// Return stats for aircraft & flightroutes for previous 24 hours
    #[allow(clippy::too_many_lines, unused)]
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
                r#"SELECT COALESCE(SUM(count), 0) AS "count!" FROM incoming_request;"#
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

    async fn seed_redis(postgres: &PgPool, redis: &Pool) -> Result<(), AppError> {
        let statistics = Self::get_daily_total_postgres(postgres).await?;
        insert_cache(redis, Some(&statistics), RedisKey::Stats).await?;
        Ok(())
    }

    #[cfg(test)]
    /// Get usage stats from postgres - For testing just return same values for daily and total, else the tests are inordinately slow
    async fn get_daily_total_postgres(postgres: &PgPool) -> Result<Stats, AppError> {
        let daily = Self::get_daily(postgres).await?;
        Ok(Stats {
            daily: daily.clone(),
            total: daily,
        })
    }
    #[cfg(not(test))]
    /// Get usage stats from postgres - this is a slow query
    async fn get_daily_total_postgres(postgres: &PgPool) -> Result<Stats, AppError> {
        let daily = Self::get_daily(postgres).await?;
        let total = Self::get_total(postgres).await?;
        Ok(Stats { daily, total })
    }

    pub async fn get_stats(postgres: &PgPool, redis: &Pool) -> Result<Stats, AppError> {
        if let Some(Some(stats)) = get_cache::<Stats>(redis, &RedisKey::Stats).await? {
            Ok(stats)
        } else {
            Self::get_daily_total_postgres(postgres).await
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
                if let Err(e) = tokio::try_join!(
                    Self::delete_temp(&postgres),
                    Self::seed_redis(&postgres, &redis),
                ) {
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
        postgres: PgPool,
        redis: Pool,
    ) -> Result<async_channel::Sender<MsgIncomingRequest>, AppError> {
        Self::seed_redis(&postgres, &redis).await?;
        let (tx, rx) = async_channel::bounded(8192);
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
