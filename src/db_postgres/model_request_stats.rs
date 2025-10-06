use fred::prelude::Pool;
use jiff_sqlx::Timestamp;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tokio::sync::mpsc::Sender;

use crate::{
    api::{AppError, Stats, StatsEntry},
    db_postgres::{
        ModelAircraft, ModelAirline, ModelFlightroute, model_aircraft::AircraftId,
        model_airline::AirlineId, model_flightroute::FlightrouteId,
    },
    db_redis::{RedisKey, TEN_MINUTES_AS_SEC, get_cache, insert_cache},
    redis_hash_to_struct,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RequestStatMsg {
    pub aircraft: Option<AircraftId>,
    pub airline: Option<AirlineId>,
    pub flightroute: Option<FlightrouteId>,
}

impl From<&ModelAircraft> for RequestStatMsg {
    fn from(value: &ModelAircraft) -> Self {
        Self {
            aircraft: Some(value.aircraft_id),
            airline: None,
            flightroute: None,
        }
    }
}

impl From<&ModelAirline> for RequestStatMsg {
    fn from(value: &ModelAirline) -> Self {
        Self {
            aircraft: None,
            airline: Some(value.airline_id),
            flightroute: None,
        }
    }
}

impl From<&ModelFlightroute> for RequestStatMsg {
    fn from(value: &ModelFlightroute) -> Self {
        Self {
            aircraft: None,
            airline: None,
            flightroute: Some(value.flightroute_id),
        }
    }
}

impl From<(&ModelAircraft, &Option<ModelFlightroute>)> for RequestStatMsg {
    fn from(value: (&ModelAircraft, &Option<ModelFlightroute>)) -> Self {
        Self {
            aircraft: Some(value.0.aircraft_id),
            airline: None,
            flightroute: value.1.as_ref().map(|i| i.flightroute_id),
        }
    }
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ModelRequestStatistics {
    pub airline_id: Option<AirlineId>,
    pub aircraft_id: Option<AircraftId>,
    pub flightroute_id: Option<FlightrouteId>,
    pub timestamp: Timestamp,
}

#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryCount {
    entry: String,
    count: i64,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Count {
    count: i64,
}

redis_hash_to_struct!(Stats);

impl ModelRequestStatistics {
    async fn _get(db: &PgPool) -> Result<Stats, AppError> {
        Ok(Stats {
            total: Self::get_total(db).await?,
            daily: Self::get_daily(db).await?,
        })
    }

    /// Return stats for aircraft & flightroutes for previous 24 hours
    async fn get_daily(db: &PgPool) -> Result<StatsEntry, AppError> {
        let flightroute = sqlx::query_as!(
            EntryCount,
            r#"SELECT
    ai.icao_prefix || fci.callsign AS "entry!",
    COUNT(*) as "count!"
FROM
    request_statistics rs
JOIN
    flightroute fr ON rs.flightroute_id = fr.flightroute_id
JOIN
    flightroute_callsign fc ON fr.flightroute_callsign_id = fc.flightroute_callsign_id
JOIN
    flightroute_callsign_inner fci ON fc.icao_prefix_id = fci.flightroute_callsign_inner_id
JOIN
    airline ai ON fc.airline_id = ai.airline_id
WHERE
    rs.timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY
    ai.icao_prefix, fci.callsign
ORDER BY
    "count!" DESC
LIMIT 10"#
        )
        .fetch_all(db)
        .await?;

        let airline = sqlx::query_as!(
            EntryCount,
            r#"SELECT
    al.icao_prefix AS "entry!",
    COUNT(*) AS "count!"
FROM
    request_statistics rs
JOIN
    airline al ON rs.airline_id = al.airline_id
WHERE
    rs.timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY
    al.icao_prefix
ORDER BY
    "count!" DESC
LIMIT 10"#
        )
        .fetch_all(db)
        .await?;

        let aircraft = sqlx::query_as!(
            EntryCount,
            r#"SELECT
    ams.mode_s AS "entry!",
    COUNT(*) AS "count!"
FROM
    request_statistics rs
JOIN
    aircraft ai ON rs.aircraft_id = ai.aircraft_id
JOIN
    aircraft_mode_s ams ON ai.aircraft_mode_s_id = ams.aircraft_mode_s_id
WHERE
    rs.timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY
    ams.mode_s
ORDER BY
    "count!" DESC
LIMIT 10"#
        )
        .fetch_all(db)
        .await?;

        let requests = sqlx::query_as!(
            Count,
            r#"SELECT COUNT(*) AS "count!" FROM request_statistics WHERE timestamp >= NOW() - INTERVAL '24 hours'"#
        )
        .fetch_one(db)
        .await?.count;
        Ok(StatsEntry {
            aircraft,
            airline,
            flightroute,
            requests,
        })
    }

    /// Return stats for aircraft & flightroutes since the begining of time
    async fn get_total(db: &PgPool) -> Result<StatsEntry, AppError> {
        let flightroute = sqlx::query_as!(
            EntryCount,
            r#"SELECT
    ai.icao_prefix || fci.callsign AS "entry!",
    COUNT(*) as "count!"
FROM
    request_statistics rs
JOIN
    flightroute fr ON rs.flightroute_id = fr.flightroute_id
JOIN
    flightroute_callsign fc ON fr.flightroute_callsign_id = fc.flightroute_callsign_id
JOIN
    flightroute_callsign_inner fci ON fc.icao_prefix_id = fci.flightroute_callsign_inner_id
JOIN
    airline ai ON fc.airline_id = ai.airline_id
GROUP BY
    ai.icao_prefix, fci.callsign
ORDER BY
    "count!" DESC
LIMIT 10"#
        )
        .fetch_all(db)
        .await?;

        let airline = sqlx::query_as!(
            EntryCount,
            r#"SELECT
    al.icao_prefix AS "entry!",
    COUNT(*) AS "count!"
FROM
    request_statistics rs
JOIN
    airline al ON rs.airline_id = al.airline_id
GROUP BY
    al.icao_prefix
ORDER BY
    "count!" DESC
LIMIT 10"#
        )
        .fetch_all(db)
        .await?;

        let aircraft = sqlx::query_as!(
            EntryCount,
            r#"SELECT
    ams.mode_s AS "entry!",
    COUNT(*) AS "count!"
FROM
    request_statistics rs
JOIN
    aircraft ai ON rs.aircraft_id = ai.aircraft_id
JOIN
    aircraft_mode_s ams ON ai.aircraft_mode_s_id = ams.aircraft_mode_s_id
GROUP BY
    ams.mode_s
ORDER BY
    "count!" DESC
LIMIT 10"#
        )
        .fetch_all(db)
        .await?;

        let requests = sqlx::query_as!(
            Count,
            r#"SELECT COUNT(*) AS "count!" FROM request_statistics"#
        )
        .fetch_one(db)
        .await?
        .count;

        Ok(StatsEntry {
            aircraft,
            airline,
            flightroute,
            requests,
        })
    }

    /// Insert a new request_stats entry
    /// Spawns into own thread, although probably should use a message handler on it's own thread to handle it
    /// Rather than spawn on X number of threads a second?
    async fn insert(db: &PgPool, msg: RequestStatMsg) -> Result<(), AppError> {
        let db = db.clone();
        sqlx::query!(
            "INSERT INTO request_statistics(aircraft_id, airline_id, flightroute_id) VALUES($1, $2, $3)",
            msg.aircraft.map(|i|i.get()),
            msg.airline.map(|i|i.get()),
            msg.flightroute.map(|i|i.get()),
        )
        .execute(&db)
        .await?;
        Ok(())
    }

    /// Get stats, first check cache, then try postgres
    pub async fn get(postgres: &PgPool, redis: &Pool) -> Result<Stats, AppError> {
        let redis_key = RedisKey::Stats;
        if let Some(Some(cache_stats)) = get_cache::<Stats>(redis, &redis_key).await? {
            Ok(cache_stats)
        } else {
            let statistics = Self::_get(postgres).await?;
            insert_cache(redis, Some(&statistics), &redis_key).await?;
            Ok(statistics)
        }
    }

    /// Create a message handler on it's own tokio thread, and return it's message sender
    /// Will insert request_statistics on each message received
    /// Will insert cache stats every ten minutes - assuming it has recieved any messages at all in that time period
    pub fn start(postgres: &PgPool, redis: &Pool) -> Sender<RequestStatMsg> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8192);
        let mut now = std::time::Instant::now();
        let postgres = postgres.clone();
        let redis = redis.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = Self::insert(&postgres, msg).await {
                    tracing::error!("{e:?}");
                }
                if now.elapsed().as_secs() > u64::try_from(TEN_MINUTES_AS_SEC).unwrap_or_default() {
                    if let Err(e) = Self::get(&postgres, &redis).await {
                        tracing::error!("{e:?}");
                    }
                    now = std::time::Instant::now();
                }
            }
        });
        tx
    }
}
