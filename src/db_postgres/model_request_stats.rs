use jiff_sqlx::Timestamp;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tokio::sync::mpsc::Sender;

use crate::{
    S,
    api::{AppError, StatsEntry},
    db_postgres::{
        ModelAircraft, ModelAirline, ModelFlightroute, model_aircraft::AircraftId,
        model_airline::AirlineId, model_flightroute::FlightrouteId,
    },
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
    // pub request_statistics_id: StatsId,
    pub airline_id: Option<AirlineId>,
    pub aircraft_id: Option<AircraftId>,
    pub flightroute_id: Option<FlightrouteId>,
    pub timestamp: Timestamp,
}
// generic_id!(StatsId);

#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryCount {
    entry: String,
    count: i64,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Count {
    count: i64,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatsTime {
    Daily,
    All,
}

impl ModelRequestStatistics {
    /// Get the request stats
    pub async fn get(db: &PgPool, time: StatsTime) -> Result<StatsEntry, AppError> {
        match time {
            StatsTime::All => Self::get_all(db).await,
            StatsTime::Daily => Self::get_daily(db).await,
        }
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
    al.airline_name AS "entry!",
    COUNT(*) AS "count!"
FROM
    request_statistics rs
JOIN
    airline al ON rs.airline_id = al.airline_id
WHERE
    rs.timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY
    al.airline_name
ORDER BY
    "count!" DESC
LIMIT 10;"#
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
LIMIT 10;"#
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
    async fn get_all(db: &PgPool) -> Result<StatsEntry, AppError> {
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
    al.airline_name AS "entry!",
    COUNT(*) AS "count!"
FROM
    request_statistics rs
JOIN
    airline al ON rs.airline_id = al.airline_id
GROUP BY
    al.airline_name
ORDER BY
    "count!" DESC
LIMIT 10;"#
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
LIMIT 10;"#
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

    /// Create a message handler on it's own tokio thread, and return it's message sender
    /// Will insert request_statistics on each message received
    pub fn start(db: &PgPool) -> Sender<RequestStatMsg> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
        let db = db.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = Self::insert(&db, msg).await {
                    tracing::error!("{e:?}");
                }
            }
        });
        tx
    }
}

// // Run tests with
// //
// // cargo watch -q -c -w src/ -x 'test model_airline '
// #[cfg(test)]
// #[allow(clippy::pedantic, clippy::unwrap_used)]
// mod tests {
//     use super::*;
//     use crate::{S, api::tests::test_setup};

//     #[tokio::test]
//     async fn model_airline_get_icao_iata_none() {
//         let test_setup = test_setup().await;
//         let callsign = &&Callsign::Iata((S!("EZ"), S!("123")));

//         let result = ModelStats::get_by_icao_callsign(&test_setup.postgres, callsign).await;

//         assert!(result.is_ok());
//         let result = result.unwrap();
//         assert!(result.is_none());
//     }

//     #[tokio::test]
//     async fn model_airline_get_icao_unknown() {
//         let test_setup = test_setup().await;
//         let callsign = &Callsign::Icao((S!("DDD"), S!("123")));

//         let result = ModelStats::get_by_icao_callsign(&test_setup.postgres, callsign).await;

//         assert!(result.is_ok());
//         let result = result.unwrap();
//         assert!(result.is_none());
//     }

//     #[tokio::test]
//     async fn model_airline_get_icao_ok() {
//         let test_setup = test_setup().await;
//         let callsign = &Callsign::Icao((S!("EZY"), S!("123")));

//         let result = ModelStats::get_by_icao_callsign(&test_setup.postgres, callsign).await;

//         assert!(result.is_ok());
//         let result = result.unwrap();
//         assert!(result.is_some());
//         let result = result.unwrap();

//         let expected = ModelStats {
//             airline_id: result.airline_id,
//             airline_name: S!("easyJet"),
//             country_name: S!("United Kingdom"),
//             country_iso_name: S!("GB"),
//             iata_prefix: Some(S!("U2")),
//             icao_prefix: S!("EZY"),
//             airline_callsign: Some(S!("EASY")),
//         };

//         assert_eq!(result, expected)
//     }

//     #[tokio::test]
//     async fn model_airline_get_airlinecode_icao_none() {
//         let test_setup = test_setup().await;
//         let airline_code = &AirlineCode::Icao(S!("DDD"));

//         let result =
//             ModelStats::get_all_by_airline_code(&test_setup.postgres, airline_code).await;

//         assert!(result.is_ok());
//         let result = result.unwrap();
//         assert!(result.is_none());
//     }

//     #[tokio::test]
//     async fn model_airline_get_airlinecode_icao_ok() {
//         let test_setup = test_setup().await;
//         let airline_code = &AirlineCode::Icao(S!("EZY"));

//         let result =
//             ModelStats::get_all_by_airline_code(&test_setup.postgres, airline_code).await;

//         assert!(result.is_ok());
//         let result = result.unwrap();
//         assert!(result.is_some());
//         let result = result.unwrap();
//         assert_eq!(result.len(), 1);

//         let expected = [ModelStats {
//             airline_id: result[0].airline_id,
//             airline_name: S!("easyJet"),
//             country_name: S!("United Kingdom"),
//             country_iso_name: S!("GB"),
//             iata_prefix: Some(S!("U2")),
//             icao_prefix: S!("EZY"),
//             airline_callsign: Some(S!("EASY")),
//         }];

//         assert_eq!(result, expected)
//     }

//     #[tokio::test]
//     async fn model_airline_get_airlinecode_iata_none() {
//         let test_setup = test_setup().await;
//         let airline_code = &&AirlineCode::Iata(S!("33"));

//         let result =
//             ModelStats::get_all_by_airline_code(&test_setup.postgres, airline_code).await;

//         assert!(result.is_ok());
//         let result = result.unwrap();
//         assert!(result.is_none());
//     }

//     #[tokio::test]
//     async fn model_airline_get_airlinecode_iata_ok() {
//         let test_setup = test_setup().await;
//         let airline_code = &AirlineCode::Iata(S!("ZY"));

//         let result =
//             ModelStats::get_all_by_airline_code(&test_setup.postgres, airline_code).await;

//         assert!(result.is_ok());
//         let result = result.unwrap();
//         assert!(result.is_some());
//         let result = result.unwrap();
//         assert_eq!(result.len(), 2);

//         let expected = [
//             ModelStats {
//                 airline_id: result[0].airline_id,
//                 airline_name: S!("Ada Air"),
//                 country_name: S!("Albania"),
//                 country_iso_name: S!("AL"),
//                 iata_prefix: Some(S!("ZY")),
//                 icao_prefix: S!("ADE"),
//                 airline_callsign: Some(S!("ADA AIR")),
//             },
//             ModelStats {
//                 airline_id: result[1].airline_id,
//                 airline_name: S!("Eznis Airways"),
//                 country_name: S!("Mongolia"),
//                 country_iso_name: S!("MN"),
//                 iata_prefix: Some(S!("ZY")),
//                 icao_prefix: S!("EZA"),
//                 airline_callsign: Some(S!("EZNIS")),
//             },
//         ];
//         assert_eq!(result, expected)
//     }
// }
