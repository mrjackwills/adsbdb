use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};

use crate::{
    api::{AircraftSearch, AppError},
    scraper::PhotoData,
};

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelAircraft {
    pub aircraft_id: i64,
    #[serde(rename = "type")]
    pub aircraft_type: String,
    pub icao_type: String,
    pub manufacturer: String,
    pub mode_s: String,
    pub registration: String,
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
    /// Separated out, so can use in tests with a transaction
    /// Get aircraft by the mode_s value
    const fn get_query_mode_s() -> &'static str {
        r#"
SELECT
    aa.aircraft_id,
    $1 AS mode_s,
    ar.registration,
    aro.registered_owner,
    aof.operator_flag_code AS registered_owner_operator_flag_code,
    co.country_name AS registered_owner_country_name, co.country_iso_name AS registered_owner_country_iso_name,
    am.manufacturer,
    at.type AS aircraft_type,
    ait.icao_type,
    CASE WHEN ap.url_photo IS NOT NULL THEN CONCAT($2, ap.url_photo) ELSE NULL END AS url_photo,
    CASE WHEN ap.url_photo IS NOT NULL THEN CONCAT($2, 'thumbnails/', ap.url_photo) ELSE NULL END AS url_photo_thumbnail
FROM
    aircraft aa
JOIN
    aircraft_mode_s ams
ON
    aa.aircraft_mode_s_id = ams.aircraft_mode_s_id
JOIN
    aircraft_registration ar
ON
    aa.aircraft_registration_id = ar.aircraft_registration_id
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
LEFT JOIN aircraft_photo ap USING (aircraft_photo_id)
WHERE
    ams.mode_s = $1"#
    }

    /// Separated out, so can use in tests with a transaction
    /// Get aircraft by the registration value
    const fn get_query_registration() -> &'static str {
        r#"
SELECT
    aa.aircraft_id,
    ams.mode_s,
    $1 AS registration,
    aro.registered_owner,
    aof.operator_flag_code AS registered_owner_operator_flag_code,
    co.country_name AS registered_owner_country_name, co.country_iso_name AS registered_owner_country_iso_name,
    am.manufacturer,
    at.type AS aircraft_type,
    ait.icao_type,
    CASE WHEN ap.url_photo IS NOT NULL THEN CONCAT($2, ap.url_photo) ELSE NULL END AS url_photo,
    CASE WHEN ap.url_photo IS NOT NULL THEN CONCAT($2, 'thumbnails/', ap.url_photo) ELSE NULL END AS url_photo_thumbnail
FROM
    aircraft aa
JOIN
    aircraft_mode_s ams
ON
    aa.aircraft_mode_s_id = ams.aircraft_mode_s_id
JOIN
    aircraft_registration ar
ON
    aa.aircraft_registration_id = ar.aircraft_registration_id
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
LEFT JOIN aircraft_photo ap USING (aircraft_photo_id)
WHERE
    ar.registration = $1"#
    }

    pub async fn get(
        db: &PgPool,
        aircraft_search: &AircraftSearch,
        photo_prefix: &str,
    ) -> Result<Option<Self>, AppError> {
        let query = match aircraft_search {
            AircraftSearch::ModeS(_) => Self::get_query_mode_s(),
            AircraftSearch::Registration(_) => Self::get_query_registration(),
        };

        Ok(sqlx::query_as::<_, Self>(query)
            .bind(aircraft_search.to_string())
            .bind(photo_prefix)
            .fetch_optional(db)
            .await?)
    }

    /// Insert a new flightroute based on scraped data, separated transaction so can be tested with a rollback
    pub async fn insert_photo(&self, db: &PgPool, photo: &PhotoData) -> Result<(), AppError> {
        let mut transaction = db.begin().await?;
        self.photo_transaction(&mut transaction, photo).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Transaction to insert a new flightroute
    async fn photo_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        photo: &PhotoData,
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
            .bind(self.aircraft_id)
            .execute(&mut *transaction)
            .await?;
        Ok(())
    }
}

// Run tests with
//
// cargo watch -q -c -w src/ -x 'test model_aircraft '
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::api::tests::test_setup;

    #[tokio::test]
    async fn model_aircraft_photo_transaction_mode_s() {
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
            registration: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        test_aircraft
            .photo_transaction(&mut transaction, &photodata)
            .await
            .unwrap();

        let query = ModelAircraft::get_query_mode_s();

        let result = sqlx::query_as::<_, ModelAircraft>(query)
            .bind(mode_s)
            .bind(url_prefix)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        let expected = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            registration: "N429AW".to_owned(),
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

    #[tokio::test]
    async fn model_aircraft_photo_transaction_registration() {
        let test_setup = test_setup().await;

        let mut transaction = test_setup.postgres.begin().await.unwrap();

        let photodata = PhotoData {
            image: "example.jpg".to_owned(),
        };

        let registration = "N429AW";

        let url_prefix = "http://www.example.com/";

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "A51D23".to_owned(),
            registration: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        test_aircraft
            .photo_transaction(&mut transaction, &photodata)
            .await
            .unwrap();

        let query = ModelAircraft::get_query_registration();

        let result = sqlx::query_as::<_, ModelAircraft>(query)
            .bind(registration)
            .bind(url_prefix)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        let expected = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            registration: "N429AW".to_owned(),
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
