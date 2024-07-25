use serde::{Deserialize, Serialize};
use sqlx::{PgExecutor, PgPool, Postgres, Transaction};

use crate::{
    api::{AircraftSearch, AppError},
    redis_hash_to_struct,
    scraper::PhotoData,
};

// sqlx::FromRow,
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    pub registered_owner_operator_flag_code: Option<String>,
    pub registered_owner: String,
    pub url_photo: Option<String>,
    pub url_photo_thumbnail: Option<String>,
}

redis_hash_to_struct!(ModelAircraft);

/// Used in transaction of inserting a new photo
#[derive(sqlx::FromRow, Debug, Clone, Copy)]
struct AircraftPhoto {
    aircraft_photo_id: i64,
}

impl ModelAircraft {
    /// Search for an aircraft by mode_s
    async fn query_by_mode_s(
        db: impl PgExecutor<'_>,
        aircraft_search: &AircraftSearch,
        photo_prefix: &str,
    ) -> Result<Option<Self>, AppError> {
        Ok(sqlx::query_as!(
            Self,
            r#"
SELECT
    aa.aircraft_id,
    $1 AS "mode_s!: _",
    ar.registration,
    aro.registered_owner,
    aof.operator_flag_code AS "registered_owner_operator_flag_code?",
    co.country_name AS registered_owner_country_name,
    co.country_iso_name AS registered_owner_country_iso_name,
    am.manufacturer,
    at.type AS aircraft_type,
    ait.icao_type,
    CASE
        WHEN ap.url_photo IS NOT NULL THEN CONCAT($2::TEXT, ap.url_photo)
        ELSE NULL
    END AS url_photo,
    CASE
        WHEN ap.url_photo IS NOT NULL THEN CONCAT($2::TEXT, 'thumbnails/', ap.url_photo)
        ELSE NULL
    END AS url_photo_thumbnail
FROM
    aircraft aa
    LEFT JOIN aircraft_mode_s ams USING(aircraft_mode_s_id)
    LEFT JOIN aircraft_registration ar USING(aircraft_registration_id)
    LEFT JOIN country co USING(country_id)
    LEFT JOIN aircraft_type at USING(aircraft_type_id)
    LEFT JOIN aircraft_registered_owner aro USING(aircraft_registered_owner_id)
    LEFT JOIN aircraft_icao_type ait USING(aircraft_icao_type_id)
    LEFT JOIN aircraft_manufacturer am USING(aircraft_manufacturer_id)
    LEFT JOIN aircraft_operator_flag_code aof USING(aircraft_operator_flag_code_id)
    LEFT JOIN aircraft_photo ap USING(aircraft_photo_id)
WHERE
    ams.mode_s = $1"#,
            aircraft_search.to_string(),
            photo_prefix
        )
        .fetch_optional(db)
        .await?)
    }

    /// Search for an aircraft by registration
    async fn query_by_registration(
        db: impl PgExecutor<'_>,
        aircraft_search: &AircraftSearch,
        photo_prefix: &str,
    ) -> Result<Option<Self>, AppError> {
        Ok(sqlx::query_as!(
            Self,
            r#"
SELECT
    aa.aircraft_id,
    ams.mode_s,
    $1 AS "registration!: _",
    aro.registered_owner,
    aof.operator_flag_code AS "registered_owner_operator_flag_code?",
    co.country_name AS registered_owner_country_name,
    co.country_iso_name AS registered_owner_country_iso_name,
    am.manufacturer,
    at.type AS aircraft_type,
    ait.icao_type,
    CASE
        WHEN ap.url_photo IS NOT NULL THEN CONCAT($2::TEXT, ap.url_photo)
        ELSE NULL
    END AS url_photo,
    CASE
        WHEN ap.url_photo IS NOT NULL THEN CONCAT($2::TEXT, 'thumbnails/', ap.url_photo)
        ELSE NULL
    END AS url_photo_thumbnail
FROM
    aircraft aa
    LEFT JOIN aircraft_mode_s ams USING(aircraft_mode_s_id)
    LEFT JOIN aircraft_registration ar USING(aircraft_registration_id)
    LEFT JOIN country co USING(country_id)
    LEFT JOIN aircraft_type at USING(aircraft_type_id)
    LEFT JOIN aircraft_registered_owner aro USING(aircraft_registered_owner_id)
    LEFT JOIN aircraft_icao_type ait USING(aircraft_icao_type_id)
    LEFT JOIN aircraft_manufacturer am USING(aircraft_manufacturer_id)
    LEFT JOIN aircraft_operator_flag_code aof USING(aircraft_operator_flag_code_id)
    LEFT JOIN aircraft_photo ap USING(aircraft_photo_id)
WHERE
    ar.registration = $1"#,
            aircraft_search.to_string(),
            photo_prefix
        )
        .fetch_optional(db)
        .await?)
    }

    pub async fn get(
        db: &PgPool,
        aircraft_search: &AircraftSearch,
        photo_prefix: &str,
    ) -> Result<Option<Self>, AppError> {
        Ok(match aircraft_search {
            AircraftSearch::ModeS(_) => {
                Self::query_by_mode_s(db, aircraft_search, photo_prefix).await?
            }
            AircraftSearch::Registration(_) => {
                Self::query_by_registration(db, aircraft_search, photo_prefix).await?
            }
        })
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
        let aircraft_photo = sqlx::query_as!(
            AircraftPhoto,
            "
INSERT INTO
    aircraft_photo(url_photo)
VALUES
    ($1)
RETURNING
    aircraft_photo_id",
            photo.image
        )
        .fetch_one(&mut **transaction)
        .await?;
        sqlx::query!("
UPDATE
    aircraft
SET
    aircraft_photo_id = $1
WHERE
    aircraft_id = $2",
            aircraft_photo.aircraft_photo_id,
            self.aircraft_id
        )
        .execute(&mut **transaction)
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
    use crate::api::{tests::test_setup, ModeS, Registration, Validate};

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
            registered_owner_operator_flag_code: Some("AWI".to_owned()),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        test_aircraft
            .photo_transaction(&mut transaction, &photodata)
            .await
            .unwrap();

        let result = ModelAircraft::query_by_mode_s(
            &mut *transaction,
            &AircraftSearch::ModeS(ModeS::validate(mode_s).unwrap()),
            url_prefix,
        )
        .await
        .unwrap()
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
            registered_owner_operator_flag_code: Some("AWI".to_owned()),
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
            registered_owner_operator_flag_code: Some("AWI".to_owned()),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        test_aircraft
            .photo_transaction(&mut transaction, &photodata)
            .await
            .unwrap();

        let result = ModelAircraft::query_by_registration(
            &mut *transaction,
            &AircraftSearch::Registration(Registration::validate(registration).unwrap()),
            url_prefix,
        )
        .await
        .unwrap()
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
            registered_owner_operator_flag_code: Some("AWI".to_owned()),
            registered_owner: "United Express".to_owned(),
            url_photo: Some("http://www.example.com/example.jpg".to_owned()),
            url_photo_thumbnail: Some("http://www.example.com/thumbnails/example.jpg".to_owned()),
        };

        assert_eq!(result, expected);

        // Cancel transaction, so can continually re-test with this route
        transaction.rollback().await.unwrap();
    }
}
