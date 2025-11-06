use axum::http::{Uri, request::Parts};
use fred::prelude::Pool;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgExecutor, PgPool, Postgres, Transaction};
use tokio::sync::mpsc::Sender;

use crate::{
    api::{AppError, Stats, StatsEntry},
    db_postgres::ID,
    db_redis::{ONE_MINUTE_AS_SEC, RedisKey, get_cache, insert_cache},
    generic_id, redis_hash_to_struct,
};

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

impl From<&Parts> for UriMethod {
    fn from(value: &Parts) -> Self {
        Self(value.uri.clone(), value.method.clone())
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

#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryCount {
    url: String,
    count: i64,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Count {
    count: i64,
}

redis_hash_to_struct!(Stats);

pub struct ModelIncomingRequest;

generic_id!(IncomingRequestUrlId);

// query_as! doesn't like ID<IncomingRequestUrlId>
type UrlId = ID<IncomingRequestUrlId>;

type GenI64 = ID<i64>;

impl ModelIncomingRequest {
    async fn get_version_id(
        url_version: Option<String>,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<GenI64>, AppError> {
        Ok(if let Some(url_version) = url_version {
            sqlx::query_as!(GenI64,
		"INSERT INTO incoming_request_url_version(url_version) VALUES ($1) ON CONFLICT (url_version)
		DO UPDATE SET url_version = EXCLUDED.url_version
		RETURNING incoming_request_url_version_id AS id", url_version)
            .fetch_optional(&mut **transaction)
            .await?
        } else {
            None
        })
    }

    async fn get_path_id(
        url_path: Option<String>,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<GenI64>, AppError> {
        Ok(if let Some(url_path) = url_path {
            sqlx::query_as!(
                GenI64,
                "INSERT INTO incoming_request_url_path(url_path) VALUES ($1) ON CONFLICT (url_path)
		DO UPDATE SET url_path = EXCLUDED.url_path
		RETURNING incoming_request_url_path_id AS id",
                url_path
            )
            .fetch_optional(&mut **transaction)
            .await?
        } else {
            None
        })
    }

    async fn get_query_id(
        url_query: Option<String>,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<GenI64>, AppError> {
        Ok(if let Some(url_query) = url_query {
            sqlx::query_as!(GenI64,
		"INSERT INTO incoming_request_url_query(url_query) VALUES ($1) ON CONFLICT (url_query)
		DO UPDATE SET url_query = EXCLUDED.url_query
		RETURNING incoming_request_url_query_id AS id", url_query)
            .fetch_optional(&mut **transaction)
            .await?
        } else {
            None
        })
    }
    /// Insert the request url into database, this will recored every single request to the database
    async fn insert_request(postgres: &PgPool, url: UriMethod) -> Result<(), AppError> {
        let mut transaction = postgres.begin().await?;

        Self::delete_temp(&mut *transaction).await?;

        let (url_version, url_path, url_query) = url.split_into_parts();

        // todo tokio join this - if so need to remove transaction

        let version_id = Self::get_version_id(url_version, &mut transaction).await?;
        let path_id = Self::get_path_id(url_path, &mut transaction).await?;
        let query_id = Self::get_query_id(url_query, &mut transaction).await?;

        let request_url = sqlx::query_as!(
            UrlId,
            r#"
     INSERT INTO incoming_request_url (incoming_request_url_version_id, incoming_request_url_path_id, incoming_request_url_query_id)
     VALUES ($1, $2, $3)
    ON CONFLICT (incoming_request_url_version_id, incoming_request_url_path_id, incoming_request_url_query_id) DO UPDATE SET incoming_request_url_version_id = EXCLUDED.incoming_request_url_version_id 
     RETURNING incoming_request_url_id AS id"#,
          version_id.map(|i|i.id), path_id.map(|i|i.id), query_id.map(|i|i.id)
        )
        .fetch_one(&mut *transaction)
        .await?;

        sqlx::query!(
            r#"INSERT INTO incoming_request (incoming_request_url_id, request_method)
        VALUES (
           $1,
            ($2::text)::request_method
        )
       ON CONFLICT (incoming_request_url_id, request_method)
        DO UPDATE SET count = incoming_request.count + 1"#,
            request_url.id.get(),
            url.1.to_string()
        )
        .execute(&mut *transaction)
        .await?;

        sqlx::query!(
            r#"INSERT INTO temp_incoming_request (incoming_request_url_id, request_method)
        VALUES (
            $1,
            ($2::text)::request_method
        )
       ON CONFLICT (incoming_request_url_id, request_method)
      DO UPDATE SET count = temp_incoming_request.count + 1"#,
            request_url.id.get(),
            url.1.to_string()
        )
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;

        Ok(())
    }

    /// Delete all entries from temp table older than 24 hours
    async fn delete_temp(db: impl PgExecutor<'_>) -> Result<(), AppError> {
        sqlx::query!("DELETE FROM temp_incoming_request WHERE timestamp <= (CURRENT_TIMESTAMP - INTERVAL '24 hours')").execute(db).await?;
        Ok(())
    }

    // Get usgae stats from postgres
    async fn _get(postgres: &PgPool) -> Result<Stats, AppError> {
        let mut transaction = postgres.begin().await?;
        sqlx::query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *transaction)
            .await?;
        let daily = Self::get_daily(&mut transaction).await?;
        let total = Self::get_total(&mut transaction).await?;
        transaction.commit().await?;
        Ok(Stats { daily, total })
    }

    /// Return stats for aircraft & flightroutes for previous 24 hours
    #[allow(clippy::too_many_lines)]
    async fn get_daily(postgres: &mut Transaction<'_, Postgres>) -> Result<StatsEntry, AppError> {
        // TODO tokio join
		// Would need to remove transaction if want to use tokio_join
        let aircraft = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(tir.count, 0)) AS "count!"
FROM temp_incoming_request tir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = tir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'aircraft'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let airline = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(tir.count, 0)) AS "count!"
FROM temp_incoming_request tir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = tir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'airline'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let flightroute = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(tir.count, 0)) AS "count!"
FROM temp_incoming_request tir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = tir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'callsign'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let mode_s = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(tir.count, 0)) AS "count!"
FROM temp_incoming_request tir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = tir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'mode-s'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let n_number = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(tir.count, 0)) AS "count!"
FROM temp_incoming_request tir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = tir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'n-number'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let stats = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(tir.count, 0)) AS "count!"
FROM temp_incoming_request tir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = tir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'stats'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let online = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(tir.count, 0)) AS "count!"
FROM temp_incoming_request tir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = tir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'online'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let aggregate = sqlx::query_as!(
            Count,
            r#"SELECT COALESCE(SUM(count), 0) AS "count!" FROM temp_incoming_request"#
        )
        .fetch_one(&mut **postgres)
        .await?
        .count;

        Ok(StatsEntry {
            aircraft,
            airline,
            callsign: flightroute,
            mode_s,
            n_number,
            online,
            stats,
            aggregate,
        })
    }

    /// Return stats for aircraft & flightroutes for previous 24 hours
    #[allow(clippy::too_many_lines)]
    async fn get_total(postgres: &mut Transaction<'_, Postgres>) -> Result<StatsEntry, AppError> {
        let aircraft = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(ir.count, 0)) AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = ir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'aircraft'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let airline = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(ir.count, 0)) AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = ir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'airline'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let flightroute = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(ir.count, 0)) AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = ir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'callsign'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let mode_s = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(ir.count, 0)) AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = ir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'mode-s'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let n_number = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(ir.count, 0)) AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = ir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'n-number'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let stats = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(ir.count, 0)) AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = ir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'stats'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let online = sqlx::query_as!(
            EntryCount,
            r#"
SELECT  '/' || CONCAT_WS('/',
    NULLIF(iruv.url_version,''),
    NULLIF(irup.url_path,''),
    NULLIF(iruq.url_query,'')) AS "url!",
    SUM(COALESCE(ir.count, 0)) AS "count!"
FROM incoming_request ir
LEFT JOIN incoming_request_url iru  ON iru.incoming_request_url_id = ir.incoming_request_url_id
LEFT JOIN incoming_request_url_version iruv ON iruv.incoming_request_url_version_id = iru.incoming_request_url_version_id
LEFT JOIN incoming_request_url_path irup  ON irup.incoming_request_url_path_id  = iru.incoming_request_url_path_id
LEFT JOIN incoming_request_url_query iruq ON iruq.incoming_request_url_query_id = iru.incoming_request_url_query_id
WHERE irup.url_path = 'online'
GROUP BY '/' || CONCAT_WS('/', NULLIF(iruv.url_version,''), NULLIF(irup.url_path,''), NULLIF(iruq.url_query,''))
ORDER BY "count!" DESC, "url!"
LIMIT 10"#
        )
        .fetch_all(&mut **postgres)
        .await?;

        let aggregate = sqlx::query_as!(
            Count,
            r#"SELECT COALESCE(SUM(count), 0) AS "count!" FROM incoming_request"#
        )
        .fetch_one(&mut **postgres)
        .await?
        .count;

        Ok(StatsEntry {
            aircraft,
            airline,
            callsign: flightroute,
            mode_s,
            n_number,
            online,
            stats,
            aggregate,
        })
    }

    /// Get stats, first check cache, then try postgres, will insert to cache if not found
    pub async fn get_stats(postgres: &PgPool, redis: &Pool) -> Result<Stats, AppError> {
        Self::delete_temp(postgres).await?;
        let redis_key = RedisKey::Stats;
        if let Some(Some(cache_stats)) = get_cache::<Stats>(redis, &redis_key).await? {
            Ok(cache_stats)
        } else {
            let statistics = Self::_get(postgres).await?;
            insert_cache(redis, Some(&statistics), redis_key).await?;
            Ok(statistics)
        }
    }

    /// Create a message handler on it's own tokio thread, and return it's message sender
    /// Will insert request_statistics on each message received
    /// Will insert cache stats every ten minutes - assuming it has recieved any messages at all in that time period
    pub async fn start(
        postgres: PgPool,
        redis: Pool,
    ) -> Result<Sender<MsgIncomingRequest>, AppError> {
        Self::get_stats(&postgres, &redis).await?;

        let (tx, mut rx) = tokio::sync::mpsc::channel(8192);
        let mut now = std::time::Instant::now();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = match msg {
                    MsgIncomingRequest::Url(i) => Self::insert_request(&postgres, i).await,
                } {
                    tracing::error!("{e:?}");
                }

                if now.elapsed().as_secs() > u64::try_from(ONE_MINUTE_AS_SEC).unwrap_or_default() {
                    if let Err(e) = Self::get_stats(&postgres, &redis).await {
                        tracing::error!("{e:?}");
                    }
                    now = std::time::Instant::now();
                }
            }
        });
        Ok(tx)
    }
}
