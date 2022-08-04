use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};

use crate::{
    api::{AppError, ModeS},
    n_number::mode_s_to_n_number,
    scraper::PhotoData,
};

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelAircraft {
    pub aircraft_id: i64,
    #[serde(rename = "type")]
    pub aircraft_type: String,
    pub icao_type: String,
    pub manufacturer: String,
    pub mode_s: String,
    pub n_number: String,
    pub registered_owner_country_iso_name: String,
    pub registered_owner_country_name: String,
    pub registered_owner_operator_flag_code: String,
    pub registered_owner: String,
    pub url_photo: Option<String>,
    pub url_photo_thumbnail: Option<String>,
}

/// Used in transaction of inserting a new photo
#[derive(sqlx::FromRow, Debug, Clone, Copy)]
struct AircraftPhoto {
    aircraft_photo_id: i64,
}

impl ModelAircraft {
    /// Seperated out, so can use in tests with a transaction
    fn get_query() -> &'static str {
        r#"
SELECT
	aa.aircraft_id,
	$1 AS mode_s,
	$2 as n_number,
	aro.registered_owner,
	aof.operator_flag_code AS registered_owner_operator_flag_code,
	co.country_name AS registered_owner_country_name, co.country_iso_name AS registered_owner_country_iso_name,
	am.manufacturer,
	at.type as aircraft_type,
	ait.icao_type,
	CASE WHEN ap.url_photo IS NOT NULL THEN CONCAT($3, ap.url_photo) ELSE NULL END as url_photo,
	CASE WHEN ap.url_photo IS NOT NULL THEN CONCAT($3, 'thumbnails/', ap.url_photo) ELSE NULL END as url_photo_thumbnail
FROM
	aircraft aa
JOIN
	aircraft_mode_s ams
ON
	aa.aircraft_mode_s_id = ams.aircraft_mode_s_id
JOIN
	country co
ON
	aa.country_id = co.country_id
JOIN
	aircraft_type at
ON
	aa.aircraft_type_id = at.aircraft_type_id
JOIN
	aircraft_registered_owner aro
ON
	aa.aircraft_registered_owner_id = aro.aircraft_registered_owner_id
JOIN
	aircraft_icao_type ait
ON
	aa.aircraft_icao_type_id = ait.aircraft_icao_type_id
JOIN
	aircraft_manufacturer am 
ON
	aa.aircraft_manufacturer_id = am.aircraft_manufacturer_id
JOIN
	aircraft_operator_flag_code aof
ON
	aa.aircraft_operator_flag_code_id = aof.aircraft_operator_flag_code_id
LEFT JOIN
	aircraft_photo ap
ON
	aa.aircraft_photo_id = ap.aircraft_photo_id
WHERE
	ams.mode_s = $1"#
    }

    pub async fn get(db: &PgPool, mode_s: &ModeS, prefix: &str) -> Result<Option<Self>, AppError> {
        let n_number = match mode_s_to_n_number(mode_s) {
            Ok(data) => data.to_string(),
            Err(_) => String::from(""),
        };

        let query = Self::get_query();
        Ok(sqlx::query_as::<_, Self>(query)
            .bind(&mode_s.to_string())
            .bind(n_number)
            .bind(prefix)
            .fetch_optional(db)
            .await?)
    }

    /// Insert a new flightroute based on scraped data, seperated transaction so can be tested with a rollback
    pub async fn insert_photo(
        db: &PgPool,
        photo: &PhotoData,
        aircraft: &ModelAircraft,
    ) -> Result<(), AppError> {
        let mut transaction = db.begin().await?;
        Self::photo_transaction(&mut transaction, photo, aircraft).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Transaction to insert a new flightroute
    async fn photo_transaction(
        transaction: &mut Transaction<'_, Postgres>,
        photo: &PhotoData,
        aircraft: &ModelAircraft,
    ) -> Result<(), AppError> {
        let query = "INSERT INTO aircraft_photo(url_photo) VALUES($1) RETURNING aircraft_photo_id";
        let aircraft_photo = sqlx::query_as::<_, AircraftPhoto>(query)
            .bind(photo.image.clone())
            .fetch_one(&mut *transaction)
            .await?;

        let query = r#"
UPDATE
	aircraft
SET
	aircraft_photo_id = $1
WHERE
	aircraft_id = $2"#;

        sqlx::query(query)
            .bind(aircraft_photo.aircraft_photo_id)
            .bind(aircraft.aircraft_id)
            .execute(&mut *transaction)
            .await?;
        Ok(())
    }
}

// Run tests with
//
// cargo watch -q -c -w src/ -x 'test model_aircraft '
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::api::tests::test_setup;

    #[tokio::test]
    async fn model_aircraft_photo_transaction() {
        let test_setup = test_setup().await;

        let mut transaction = test_setup.postgres.begin().await.unwrap();

        let photodata = PhotoData {
            image: "example.jpg".to_owned(),
        };

        let mode_s = "A51D23";

        let url_prefix = "http://www.example.com/";

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "A51D23".to_owned(),
            n_number: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        ModelAircraft::photo_transaction(&mut transaction, &photodata, &test_aircraft)
            .await
            .unwrap();

        let query = ModelAircraft::get_query();

        let result = sqlx::query_as::<_, ModelAircraft>(query)
            .bind(mode_s)
            .bind("N429AW")
            .bind(url_prefix)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        let expected = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            n_number: "N429AW".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "A51D23".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: Some("http://www.example.com/example.jpg".to_owned()),
            url_photo_thumbnail: Some("http://www.example.com/thumbnails/example.jpg".to_owned()),
        };

        assert_eq!(result, expected);

        // Cancel transaction, so can continually re-test with this route
        transaction.rollback().await.unwrap();
    }
}
