use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};

use crate::{api::AppError, scraper::ScrapedFlightroute};

use super::Model;

/// Used in transaction of inserting a new scraped flightroute
#[derive(sqlx::FromRow, Debug, Clone, Copy)]
struct FlightrouteCallsign {
    flightroute_callsign_id: i64,
}

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelFlightroute {
    pub flightroute_id: i64,
    pub callsign: String,

    pub origin_airport_country_iso_name: String,
    pub origin_airport_country_name: String,
    pub origin_airport_elevation: i32,
    pub origin_airport_iata_code: String,
    pub origin_airport_icao_code: String,
    pub origin_airport_latitude: f64,
    pub origin_airport_longitude: f64,
    pub origin_airport_municipality: String,
    pub origin_airport_name: String,

    pub midpoint_airport_country_iso_name: Option<String>,
    pub midpoint_airport_country_name: Option<String>,
    pub midpoint_airport_elevation: Option<i32>,
    pub midpoint_airport_iata_code: Option<String>,
    pub midpoint_airport_icao_code: Option<String>,
    pub midpoint_airport_latitude: Option<f64>,
    pub midpoint_airport_longitude: Option<f64>,
    pub midpoint_airport_municipality: Option<String>,
    pub midpoint_airport_name: Option<String>,

    pub destination_airport_country_iso_name: String,
    pub destination_airport_country_name: String,
    pub destination_airport_elevation: i32,
    pub destination_airport_iata_code: String,
    pub destination_airport_icao_code: String,
    pub destination_airport_latitude: f64,
    pub destination_airport_longitude: f64,
    pub destination_airport_municipality: String,
    pub destination_airport_name: String,
}

#[async_trait]
impl Model<Self> for ModelFlightroute {
    async fn get(db: &PgPool, callsign: &str) -> Result<Option<Self>, AppError> {
        Ok(sqlx::query_as::<_, Self>(Self::get_query())
            .bind(callsign)
            .fetch_optional(db)
            .await?)
    }
}

impl ModelFlightroute {
    /// Seperated out, so can use in tests with a transaction
    /// Could also just be a const str
    const fn get_query() -> &'static str {
        r#"
		SELECT
			$1 AS callsign,
			fl.flightroute_id,
			( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_country_name,
			( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_country_iso_name,
			( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_municipality,
			( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_icao_code,
			( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_iata_code,
			( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_name,
			( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_elevation,
			( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_latitude,
			( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_longitude,
		
			( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_country_name,
			( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_country_iso_name,
			( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_municipality,
			( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_icao_code,
			( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_iata_code,
			( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_name,
			( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_elevation,
			( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_latitude,
			( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_longitude,
			
			( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_country_name,
			( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_country_iso_name,
			( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_municipality,
			( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_icao_code,
			( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_iata_code,
			( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_name,
			( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_elevation,
			( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_latitude,
			( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_longitude
		FROM
			flightroute fl
		JOIN
			flightroute_callsign flc
		ON
			fl.flightroute_callsign_id = flc.flightroute_callsign_id
		JOIN
			airport apo
		ON
			fl.airport_origin_id = apo.airport_id
		LEFT JOIN
			airport apm
		ON
			fl.airport_midpoint_id = apm.airport_id
		JOIN
			airport apd
		ON
			fl.airport_destination_id = apd.airport_id
		WHERE 
			flc.callsign = $1"#
    }

    /// Insert a new flightroute based on scraped data, seperated transaction so can be tested with a rollback
    pub async fn insert_scraped_flightroute(
        db: &PgPool,
        scraped_flightroute: ScrapedFlightroute,
    ) -> Result<(), AppError> {
        let mut transaction = db.begin().await?;
        Self::scraped_flightroute_transaction(&mut transaction, scraped_flightroute).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Transaction to insert a new flightroute
    async fn scraped_flightroute_transaction(
        transaction: &mut Transaction<'_, Postgres>,
        scraped_flightroute: ScrapedFlightroute,
    ) -> Result<(), AppError> {
        let query = "INSERT INTO flightroute_callsign(callsign) VALUES ($1) RETURNING flightroute_callsign_id";
        //
        let callsign = sqlx::query_as::<_, FlightrouteCallsign>(query)
            .bind(scraped_flightroute.callsign.clone())
            .fetch_one(&mut *transaction)
            .await?;
        let query = r#"INSERT INTO
	flightroute(flightroute_callsign_id, airport_origin_id, airport_destination_id)
VALUES (
	$1,
	(SELECT aa.airport_id FROM airport aa JOIN airport_icao_code aic ON aa.airport_icao_code_id = aic.airport_icao_code_id WHERE aic.icao_code = $2),
	(SELECT aa.airport_id FROM airport aa JOIN airport_icao_code aic ON aa.airport_icao_code_id = aic.airport_icao_code_id WHERE aic.icao_code = $3)
	)"#;

        sqlx::query(query)
            .bind(callsign.flightroute_callsign_id)
            .bind(scraped_flightroute.origin)
            .bind(scraped_flightroute.destination)
            .execute(&mut *transaction)
            .await?;
        Ok(())
    }
}

/// Run tests with
///
/// cargo watch -q -c -w src/ -x 'test model_flightroute '
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
mod tests {
    use crate::{db_postgres, parse_env::AppEnv};

    async fn setup() -> (AppEnv, PgPool) {
        let app_env = AppEnv::get_env();
        let db = db_postgres::db_pool(&app_env).await.unwrap();
        (app_env, db)
    }

    use super::*;
    #[tokio::test]
    async fn model_flightroute_scraped_flightroute_transaction() {
        let setup = setup().await;

        let mut transaction = setup.1.begin().await.unwrap();

        let scraped_flightroute = ScrapedFlightroute {
            callsign: "ANA460".to_owned(),
            origin: "ROAH".to_owned(),
            destination: "RJTT".to_owned(),
        };

        ModelFlightroute::scraped_flightroute_transaction(
            &mut transaction,
            scraped_flightroute.clone(),
        )
        .await
        .unwrap();

        let query = ModelFlightroute::get_query();
        let result = sqlx::query_as::<_, ModelFlightroute>(query)
            .bind(scraped_flightroute.callsign)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        let expected = ModelFlightroute {
            flightroute_id: 0,
            callsign: "ANA460".to_owned(),
            origin_airport_country_iso_name: "JP".to_owned(),
            origin_airport_country_name: "Japan".to_owned(),
            origin_airport_elevation: 12,
            origin_airport_iata_code: "OKA".to_owned(),
            origin_airport_icao_code: "ROAH".to_owned(),
            origin_airport_latitude: 26.195_801,
            origin_airport_longitude: 127.646_004,
            origin_airport_municipality: "Naha".to_owned(),
            origin_airport_name: "Naha Airport / JASDF Naha Air Base".to_owned(),
            midpoint_airport_country_iso_name: None,
            midpoint_airport_country_name: None,
            midpoint_airport_elevation: None,
            midpoint_airport_iata_code: None,
            midpoint_airport_icao_code: None,
            midpoint_airport_latitude: None,
            midpoint_airport_longitude: None,
            midpoint_airport_municipality: None,
            midpoint_airport_name: None,
            destination_airport_country_iso_name: "JP".to_owned(),
            destination_airport_country_name: "Japan".to_owned(),
            destination_airport_elevation: 35,
            destination_airport_iata_code: "HND".to_owned(),
            destination_airport_icao_code: "RJTT".to_owned(),
            destination_airport_latitude: 35.552_299,
            destination_airport_longitude: 139.779_999,
            destination_airport_municipality: "Tokyo".to_owned(),
            destination_airport_name: "Tokyo Haneda International Airport".to_owned(),
        };

        // Have to do it like this, as unable to guarantee the flightroute id correctly in tests
        assert_eq!(result.callsign, expected.callsign);
        assert_eq!(
            result.origin_airport_country_iso_name,
            expected.origin_airport_country_iso_name
        );
        assert_eq!(
            result.origin_airport_country_name,
            expected.origin_airport_country_name
        );
        assert_eq!(
            result.origin_airport_elevation,
            expected.origin_airport_elevation
        );
        assert_eq!(
            result.origin_airport_iata_code,
            expected.origin_airport_iata_code
        );
        assert_eq!(
            result.origin_airport_icao_code,
            expected.origin_airport_icao_code
        );
        assert_eq!(
            result.origin_airport_latitude,
            expected.origin_airport_latitude
        );
        assert_eq!(
            result.origin_airport_longitude,
            expected.origin_airport_longitude
        );
        assert_eq!(
            result.origin_airport_municipality,
            expected.origin_airport_municipality
        );
        assert_eq!(result.origin_airport_name, expected.origin_airport_name);
        assert_eq!(
            result.midpoint_airport_country_iso_name,
            expected.midpoint_airport_country_iso_name
        );
        assert_eq!(
            result.midpoint_airport_country_name,
            expected.midpoint_airport_country_name
        );
        assert_eq!(
            result.midpoint_airport_elevation,
            expected.midpoint_airport_elevation
        );
        assert_eq!(
            result.midpoint_airport_iata_code,
            expected.midpoint_airport_iata_code
        );
        assert_eq!(
            result.midpoint_airport_icao_code,
            expected.midpoint_airport_icao_code
        );
        assert_eq!(
            result.midpoint_airport_latitude,
            expected.midpoint_airport_latitude
        );
        assert_eq!(
            result.midpoint_airport_longitude,
            expected.midpoint_airport_longitude
        );
        assert_eq!(
            result.midpoint_airport_municipality,
            expected.midpoint_airport_municipality
        );
        assert_eq!(result.midpoint_airport_name, expected.midpoint_airport_name);
        assert_eq!(
            result.destination_airport_country_iso_name,
            expected.destination_airport_country_iso_name
        );
        assert_eq!(
            result.destination_airport_country_name,
            expected.destination_airport_country_name
        );
        assert_eq!(
            result.destination_airport_elevation,
            expected.destination_airport_elevation
        );
        assert_eq!(
            result.destination_airport_iata_code,
            expected.destination_airport_iata_code
        );
        assert_eq!(
            result.destination_airport_icao_code,
            expected.destination_airport_icao_code
        );
        assert_eq!(
            result.destination_airport_latitude,
            expected.destination_airport_latitude
        );
        assert_eq!(
            result.destination_airport_longitude,
            expected.destination_airport_longitude
        );
        assert_eq!(
            result.destination_airport_municipality,
            expected.destination_airport_municipality
        );
        assert_eq!(
            result.destination_airport_name,
            expected.destination_airport_name
        );

        // Cancel transaction, so can continually re-test with this route
        transaction.rollback().await.unwrap();
    }
}
