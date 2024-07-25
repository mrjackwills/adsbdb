use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    api::{AirlineCode, AppError, Callsign},
    redis_hash_to_struct,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelAirline {
    pub airline_id: i64,
    pub airline_name: String,
    pub country_name: String,
    pub country_iso_name: String,
    pub iata_prefix: Option<String>,
    pub icao_prefix: String,
    pub airline_callsign: Option<String>,
}

redis_hash_to_struct!(ModelAirline);

impl ModelAirline {
    pub async fn get_by_icao_callsign(
        db: &PgPool,
        callsign: &Callsign,
    ) -> Result<Option<Self>, AppError> {
        match callsign {
            Callsign::Icao(x) => Ok(sqlx::query_as!(
                Self,
                "
SELECT
    co.country_name,
    co.country_iso_name,
    ai.airline_id,
    ai.airline_callsign,
    ai.airline_name,
    ai.iata_prefix,
    ai.icao_prefix
FROM
    airline ai
    LEFT JOIN country co USING(country_id)
WHERE
    icao_prefix = $1",
                x.0
            )
            .fetch_optional(db)
            .await?),
            _ => Ok(None),
        }
    }

    /// Search for arilines by iata prefix
    async fn get_all_by_iata_code(
        db: &PgPool,
        prefix: &str,
    ) -> Result<Option<Vec<Self>>, AppError> {
        let result = sqlx::query_as!(
            Self,
            "
SELECT
    co.country_name,
    co.country_iso_name,
    ai.airline_id,
    ai.airline_callsign,
    ai.airline_name,
    ai.iata_prefix,
    ai.icao_prefix
FROM
    airline ai
    LEFT JOIN country co USING(country_id)
WHERE
    iata_prefix = $1
ORDER BY
    ai.airline_name",
            prefix
        )
        .fetch_all(db)
        .await?;
        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// Search for arilines by icao prefix
    async fn get_all_by_icao_code(
        db: &PgPool,
        prefix: &str,
    ) -> Result<Option<Vec<Self>>, AppError> {
        let result = sqlx::query_as!(
            Self,
            "
SELECT
    co.country_name,
    co.country_iso_name,
    ai.airline_id,
    ai.airline_callsign,
    ai.airline_name,
    ai.iata_prefix,
    ai.icao_prefix
FROM
    airline ai
    LEFT JOIN country co USING(country_id)
WHERE
    icao_prefix = $1
ORDER BY
    ai.airline_name",
            prefix
        )
        .fetch_all(db)
        .await?;
        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    pub async fn get_all_by_airline_code(
        db: &PgPool,
        airline_code: &AirlineCode,
    ) -> Result<Option<Vec<Self>>, AppError> {
        Ok(match airline_code {
            AirlineCode::Iata(x) => Self::get_all_by_iata_code(db, x).await?,
            AirlineCode::Icao(x) => Self::get_all_by_icao_code(db, x).await?,
        })
    }
}

// Run tests with
//
// cargo watch -q -c -w src/ -x 'test model_airline '
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::api::tests::test_setup;

    #[tokio::test]
    async fn model_airline_get_icao_iata_none() {
        let test_setup = test_setup().await;
        let callsign = &&Callsign::Iata(("EZ".to_owned(), "123".to_owned()));

        let result = ModelAirline::get_by_icao_callsign(&test_setup.postgres, callsign).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn model_airline_get_icao_unknown() {
        let test_setup = test_setup().await;
        let callsign = &Callsign::Icao(("DDD".to_owned(), "123".to_owned()));

        let result = ModelAirline::get_by_icao_callsign(&test_setup.postgres, callsign).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn model_airline_get_icao_ok() {
        let test_setup = test_setup().await;
        let callsign = &Callsign::Icao(("EZY".to_owned(), "123".to_owned()));

        let result = ModelAirline::get_by_icao_callsign(&test_setup.postgres, callsign).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();

        let expected = ModelAirline {
            airline_id: result.airline_id,
            airline_name: "easyJet".to_owned(),
            country_name: "United Kingdom".to_owned(),
            country_iso_name: "GB".to_owned(),
            iata_prefix: Some("U2".to_owned()),
            icao_prefix: "EZY".to_owned(),
            airline_callsign: Some("EASY".to_owned()),
        };

        assert_eq!(result, expected)
    }

    #[tokio::test]
    async fn model_airline_get_airlinecode_icao_none() {
        let test_setup = test_setup().await;
        let airline_code = &AirlineCode::Icao("DDD".to_owned());

        let result =
            ModelAirline::get_all_by_airline_code(&test_setup.postgres, airline_code).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn model_airline_get_airlinecode_icao_ok() {
        let test_setup = test_setup().await;
        let airline_code = &AirlineCode::Icao("EZY".to_owned());

        let result =
            ModelAirline::get_all_by_airline_code(&test_setup.postgres, airline_code).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.len(), 1);

        let expected = [ModelAirline {
            airline_id: result[0].airline_id,
            airline_name: "easyJet".to_owned(),
            country_name: "United Kingdom".to_owned(),
            country_iso_name: "GB".to_owned(),
            iata_prefix: Some("U2".to_owned()),
            icao_prefix: "EZY".to_owned(),
            airline_callsign: Some("EASY".to_owned()),
        }];

        assert_eq!(result, expected)
    }

    #[tokio::test]
    async fn model_airline_get_airlinecode_iata_none() {
        let test_setup = test_setup().await;
        let airline_code = &&AirlineCode::Iata("33".to_owned());

        let result =
            ModelAirline::get_all_by_airline_code(&test_setup.postgres, airline_code).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn model_airline_get_airlinecode_iata_ok() {
        let test_setup = test_setup().await;
        let airline_code = &AirlineCode::Iata("ZY".to_owned());

        let result =
            ModelAirline::get_all_by_airline_code(&test_setup.postgres, airline_code).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.len(), 2);

        let expected = [
            ModelAirline {
                airline_id: result[0].airline_id,
                airline_name: "Ada Air".to_owned(),
                country_name: "Albania".to_owned(),
                country_iso_name: "AL".to_owned(),
                iata_prefix: Some("ZY".to_owned()),
                icao_prefix: "ADE".to_owned(),
                airline_callsign: Some("ADA AIR".to_owned()),
            },
            ModelAirline {
                airline_id: result[1].airline_id,
                airline_name: "Eznis Airways".to_owned(),
                country_name: "Mongolia".to_owned(),
                country_iso_name: "MN".to_owned(),
                iata_prefix: Some("ZY".to_owned()),
                icao_prefix: "EZA".to_owned(),
                airline_callsign: Some("EZNIS".to_owned()),
            },
        ];
        assert_eq!(result, expected)
    }
}
