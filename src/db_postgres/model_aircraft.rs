use serde::{Deserialize, Serialize};
use sqlx::{PgExecutor, PgPool, Postgres, Transaction};

use crate::{
    S,
    api::{AircraftSearch, AppError, ResponseAircraft},
    redis_hash_to_struct,
    scraper::PhotoData,
};

/// Generic PostgreSQL ID
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, serde::Deserialize)]
struct ID<T> {
    id: T,
}

/// This macro generates a newtype wrapper around `i64`, providing useful trait implementations for custom specific ID types
#[macro_export]
macro_rules! generic_id {
    ($struct_name:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            Hash,
            Eq,
            PartialEq,
            PartialOrd,
            Ord,
            sqlx::Decode,
            serde::Deserialize,
        )]
        struct $struct_name(i64);

        impl sqlx::Type<sqlx::Postgres> for $struct_name {
            fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
                <i64 as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        impl From<i64> for $struct_name {
            fn from(x: i64) -> Self {
                Self(x)
            }
        }

        impl fred::types::FromValue for $struct_name {
            fn from_value(value: fred::prelude::Value) -> Result<Self, fred::prelude::Error> {
                value.as_i64().map_or(
                    Err(fred::error::Error::new(
                        fred::error::ErrorKind::Parse,
                        format!("FromRedis: {}", stringify!($struct_name)),
                    )),
                    |i| Ok(Self(i)),
                )
            }
        }

        impl serde::Serialize for $struct_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_newtype_struct(stringify!($struct_name), &self.0)
            }
        }
    };
}

generic_id!(Country);
generic_id!(AircraftType);
generic_id!(AircraftIcaoType);
generic_id!(AircraftManufacturer);
generic_id!(AircraftRegistration);
generic_id!(AircraftRegisteredOwner);
generic_id!(AircraftOperatorFlagCode);
generic_id!(AircraftPhoto);

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, serde::Deserialize)]
struct CountryRegistrationPrefix {
    aircraft_registration_country_prefix_id: i64,
    registration_country_prefix: String,
}

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
            ID::<AircraftPhoto>,
            "
INSERT INTO
    aircraft_photo(url_photo)
VALUES
    ($1)
RETURNING
    aircraft_photo_id AS id",
            photo.image
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;
        sqlx::query!(
            "
UPDATE
    aircraft
SET
    aircraft_photo_id = $1
WHERE
    aircraft_id = $2",
            aircraft_photo.0,
            self.aircraft_id
        )
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn update_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        input: &ResponseAircraft,
    ) -> Result<(), AppError> {
        // check that country exists
        let Some(country_id) = sqlx::query_as!(ID::<Country>,
            "SELECT country_id as id FROM country WHERE country_name = $1 AND country_iso_name = $2",
            input.registered_owner_country_name,
            input.registered_owner_country_iso_name
        )
        .fetch_optional(&mut **transaction)
        .await? else
        {
            return Err(AppError::Body(S!("unknown country")));
        };

        // Ireland and Argentina have multiple prefixes, so need to account for that by query for a vec
        // and then checking against the given registration
        // Something is wrong with this query
        let registration_prefix = sqlx::query_as!(
            CountryRegistrationPrefix,
            "
SELECT DISTINCT
    arc.registration_country_prefix, arc.aircraft_registration_country_prefix_id
FROM
    aircraft a
JOIN
    country c USING(country_id)
JOIN
    aircraft_registration_country_prefix arc USING(aircraft_registration_country_prefix_id)
WHERE
    c.country_iso_name = $1
ORDER BY
    registration_country_prefix;",
            input.registered_owner_country_iso_name
        )
        .fetch_all(&mut **transaction)
        .await?;

        if registration_prefix.is_empty() {
            return Err(AppError::Body(S!("unknown registration prefix")));
        }

        let Some(country_prefix) = registration_prefix.iter().find(|i| {
            input
                .registration
                .starts_with(&i.registration_country_prefix)
        }) else {
            return Err(AppError::Body(S!("unknown registration prefix")));
        };

        // Aircraft Registration
        sqlx::query!("INSERT INTO aircraft_registration(registration) VALUES($1) ON CONFLICT(registration) DO NOTHING", input.registration).execute(&mut **transaction).await?;
        let aircraft_registration_id = sqlx::query_as!(
            ID::<AircraftRegistration>,
            "SELECT aircraft_registration_id AS id FROM aircraft_registration WHERE registration = $1",
            input.registration
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;

        // Aircraft Type
        sqlx::query!(
            "INSERT INTO aircraft_type(type) VALUES($1) ON CONFLICT(type) DO NOTHING",
            input.aircraft_type
        )
        .execute(&mut **transaction)
        .await?;
        let aircraft_type_id = sqlx::query_as!(
            ID::<AircraftType>,
            "SELECT aircraft_type_id AS id FROM aircraft_type WHERE type = $1",
            input.aircraft_type
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;

        // Aircraft ICAO Type
        sqlx::query!("INSERT INTO aircraft_icao_type(icao_type) VALUES($1) ON CONFLICT(icao_type) DO NOTHING", input.icao_type).execute(&mut **transaction).await?;
        let aircraft_icao_type_id: AircraftIcaoType = sqlx::query_as!(
            ID::<AircraftIcaoType>,
            "SELECT aircraft_icao_type_id AS id FROM aircraft_icao_type WHERE icao_type = $1",
            input.icao_type
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;

        // Manufacturer
        sqlx::query!("INSERT INTO aircraft_manufacturer(manufacturer) VALUES($1) ON CONFLICT(manufacturer) DO NOTHING", input.manufacturer).execute(&mut **transaction).await?;
        let aircraft_manufacturer_id = sqlx::query_as!(
            ID::<AircraftManufacturer>,
            "SELECT aircraft_manufacturer_id AS id FROM aircraft_manufacturer WHERE manufacturer = $1",
            input.manufacturer
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;

        // Registered Owner
        sqlx::query!("INSERT INTO aircraft_registered_owner(registered_owner) VALUES($1) ON CONFLICT(registered_owner) DO NOTHING", input.registered_owner).execute(&mut **transaction).await?;
        let aircraft_registered_owner_id = sqlx::query_as!(
        ID::<AircraftRegisteredOwner>,
        "SELECT aircraft_registered_owner_id AS id FROM aircraft_registered_owner WHERE registered_owner = $1",
        input.registered_owner
    )
    .fetch_one(&mut **transaction)
    .await?
    .id;

        // Registered Owner Flag Code
        let aircraft_operator_flag_code_id = if input.registered_owner_operator_flag_code.is_some()
        {
            sqlx::query!("INSERT INTO aircraft_operator_flag_code(operator_flag_code) VALUES($1) ON CONFLICT(operator_flag_code) DO NOTHING", input.registered_owner_operator_flag_code).execute(&mut **transaction).await?;
            Some(sqlx::query_as!(
            ID::<AircraftOperatorFlagCode>,
            "SELECT aircraft_operator_flag_code_id AS id FROM aircraft_operator_flag_code WHERE operator_flag_code = $1",
            input.registered_owner_operator_flag_code
        )
        .fetch_one(&mut **transaction)
        .await?
        .id)
        } else {
            None
        };

        sqlx::query!(
            "
UPDATE 
    aircraft
SET
    aircraft_type_id = $1,
    aircraft_icao_type_id = $2,
    aircraft_manufacturer_id = $3,
    aircraft_registration_country_prefix_id = $4,
    aircraft_registration_id = $5,
    country_id = $6,
    aircraft_operator_flag_code_id = $7,
    aircraft_registered_owner_id = $8
WHERE
    aircraft_id = $9",
            aircraft_type_id.0,
            aircraft_icao_type_id.0,
            aircraft_manufacturer_id.0,
            country_prefix.aircraft_registration_country_prefix_id,
            aircraft_registration_id.0,
            country_id.id.0,
            aircraft_operator_flag_code_id.map(|i| i.0),
            aircraft_registered_owner_id.0,
            self.aircraft_id,
        )
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Delete any unused/dangling entries after an aircraft update
    async fn remove_unused(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "
DELETE FROM aircraft_type
WHERE aircraft_type_id IN (
    SELECT at.aircraft_type_id
    FROM aircraft_type at
    LEFT JOIN aircraft a ON at.aircraft_type_id = a.aircraft_type_id
    WHERE a.aircraft_type_id IS NULL
)"
        )
        .execute(&mut **transaction)
        .await?;

        sqlx::query!(
            "
DELETE FROM aircraft_icao_type
WHERE aircraft_icao_type_id IN (
    SELECT ait.aircraft_icao_type_id
    FROM aircraft_icao_type ait
    LEFT JOIN aircraft a ON ait.aircraft_icao_type_id = a.aircraft_icao_type_id
    WHERE a.aircraft_icao_type_id IS NULL
)"
        )
        .execute(&mut **transaction)
        .await?;

        sqlx::query!(
            "
DELETE FROM aircraft_manufacturer
WHERE aircraft_manufacturer_id IN (
    SELECT am.aircraft_manufacturer_id
    FROM aircraft_manufacturer am
    LEFT JOIN aircraft a ON am.aircraft_manufacturer_id = a.aircraft_manufacturer_id
    WHERE a.aircraft_manufacturer_id IS NULL
)"
        )
        .execute(&mut **transaction)
        .await?;

        sqlx::query!(
            "
DELETE FROM aircraft_registration
WHERE aircraft_registration_id IN (
    SELECT ar.aircraft_registration_id
    FROM aircraft_registration ar
    LEFT JOIN aircraft a ON ar.aircraft_registration_id = a.aircraft_registration_id
    WHERE a.aircraft_registration_id IS NULL
)"
        )
        .execute(&mut **transaction)
        .await?;

        sqlx::query!(
            "
DELETE FROM aircraft_operator_flag_code
WHERE aircraft_operator_flag_code_id IN (
    SELECT aofc.aircraft_operator_flag_code_id
    FROM aircraft_operator_flag_code aofc
    LEFT JOIN aircraft a ON aofc.aircraft_operator_flag_code_id = a.aircraft_operator_flag_code_id
    WHERE a.aircraft_operator_flag_code_id IS NULL
)"
        )
        .execute(&mut **transaction)
        .await?;

        sqlx::query!(
            "
DELETE FROM aircraft_registered_owner
WHERE aircraft_registered_owner_id IN (
    SELECT aro.aircraft_registered_owner_id
    FROM aircraft_registered_owner aro
    LEFT JOIN aircraft a ON aro.aircraft_registered_owner_id = a.aircraft_registered_owner_id
    WHERE a.aircraft_registered_owner_id IS NULL
)"
        )
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub async fn update(&self, postgres: PgPool, input: &ResponseAircraft) -> Result<(), AppError> {
        let mut transaction = postgres.begin().await?;
        self.update_transaction(&mut transaction, input).await?;
        self.remove_unused(&mut transaction).await?;
        transaction.commit().await?;
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
    use crate::{
        S,
        api::{ModeS, Registration, Validate, tests::test_setup},
    };

    #[tokio::test]
    async fn model_aircraft_photo_transaction_mode_s() {
        let test_setup = test_setup().await;

        let mut transaction = test_setup.postgres.begin().await.unwrap();

        let photodata = PhotoData {
            image: S!("example.jpg"),
        };

        let mode_s = "A51D23";

        let url_prefix = "http://www.example.com/";

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: S!("CRJ 200LR"),
            icao_type: S!("CRJ2"),
            manufacturer: S!("Bombardier"),
            mode_s: S!("A51D23"),
            registration: S!("N429AW"),
            registered_owner_country_iso_name: S!("US"),
            registered_owner_country_name: S!("United States"),
            registered_owner_operator_flag_code: Some(S!("AWI")),
            registered_owner: S!("United Express"),
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
            aircraft_type: S!("CRJ 200LR"),
            registration: S!("N429AW"),
            icao_type: S!("CRJ2"),
            manufacturer: S!("Bombardier"),
            mode_s: S!("A51D23"),
            registered_owner_country_iso_name: S!("US"),
            registered_owner_country_name: S!("United States"),
            registered_owner_operator_flag_code: Some(S!("AWI")),
            registered_owner: S!("United Express"),
            url_photo: Some(S!("http://www.example.com/example.jpg")),
            url_photo_thumbnail: Some(S!("http://www.example.com/thumbnails/example.jpg")),
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
            image: S!("example.jpg"),
        };

        let registration = "N429AW";

        let url_prefix = "http://www.example.com/";

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: S!("CRJ 200LR"),
            icao_type: S!("CRJ2"),
            manufacturer: S!("Bombardier"),
            mode_s: S!("A51D23"),
            registration: S!("N429AW"),
            registered_owner_country_iso_name: S!("US"),
            registered_owner_country_name: S!("United States"),
            registered_owner_operator_flag_code: Some(S!("AWI")),
            registered_owner: S!("United Express"),
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
            aircraft_type: S!("CRJ 200LR"),
            registration: S!("N429AW"),
            icao_type: S!("CRJ2"),
            manufacturer: S!("Bombardier"),
            mode_s: S!("A51D23"),
            registered_owner_country_iso_name: S!("US"),
            registered_owner_country_name: S!("United States"),
            registered_owner_operator_flag_code: Some(S!("AWI")),
            registered_owner: S!("United Express"),
            url_photo: Some(S!("http://www.example.com/example.jpg")),
            url_photo_thumbnail: Some(S!("http://www.example.com/thumbnails/example.jpg")),
        };

        assert_eq!(result, expected);

        // Cancel transaction, so can continually re-test with this route
        transaction.rollback().await.unwrap();
    }

    // Update
    //

    fn generate_aircraft() -> ModelAircraft {
        ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: S!("CRJ 200LR"),
            registration: S!("N429AW"),
            icao_type: S!("CRJ2"),
            manufacturer: S!("Bombardier"),
            mode_s: S!("A51D23"),
            registered_owner_country_iso_name: S!("US"),
            registered_owner_country_name: S!("United States"),
            registered_owner_operator_flag_code: Some(S!("AWI")),
            registered_owner: S!("United Express"),
            url_photo: Some(S!("http://www.example.com/example.jpg")),
            url_photo_thumbnail: Some(S!("http://www.example.com/thumbnails/example.jpg")),
        }
    }

    #[derive(Deserialize)]
    struct TestOutput {
        value: String,
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    /// Update an aircraft, set each updatable value to XX, and validate that the aircraft details have been changed
    async fn model_aircraft_update_new_values() {
        let test_setup = test_setup().await;

        // aircraft type
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.aircraft_type = S!("XXX");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT at.type AS value
            FROM aircraft a
            LEFT JOIN aircraft_type at USING (aircraft_type_id) WHERE aircraft_id = $1",
            aircraft.aircraft_id
        )
        .fetch_one(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, "XXX");
        transaction.rollback().await.unwrap();

        // aircraft icao_type
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.icao_type = S!("XXX");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT ait.icao_type AS value
            FROM aircraft a
            LEFT JOIN aircraft_icao_type ait USING (aircraft_icao_type_id) WHERE aircraft_id = $1",
            aircraft.aircraft_id
        )
        .fetch_one(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, "XXX");
        transaction.rollback().await.unwrap();

        // Manufacturer
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.manufacturer = S!("XXX");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT am.manufacturer AS value
            FROM aircraft a
            LEFT JOIN aircraft_manufacturer am USING (aircraft_manufacturer_id) WHERE aircraft_id = $1",
            aircraft.aircraft_id
        )
        .fetch_one(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, "XXX");
        transaction.rollback().await.unwrap();

        // Registered owner
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner = S!("XXX");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_ok());
        let result = sqlx::query_as!(TestOutput,
            "SELECT aro.registered_owner AS value
            FROM aircraft a
            LEFT JOIN aircraft_registered_owner aro USING (aircraft_registered_owner_id) WHERE aircraft_id = $1", aircraft.aircraft_id).fetch_one(&mut *transaction).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, "XXX");
        transaction.rollback().await.unwrap();

        // Registration
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());

        input.registration = S!("NXXX");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT ar.registration AS value
            FROM aircraft a
            LEFT JOIN aircraft_registration ar USING (aircraft_registration_id) WHERE aircraft_id = $1",
            aircraft.aircraft_id
        )
        .fetch_one(&mut *transaction)
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, "NXXX");
        transaction.rollback().await.unwrap();

        // Flag code
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());

        input.registered_owner_operator_flag_code = Some(S!("XXX"));
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT aofc.operator_flag_code AS value
            FROM aircraft a
            LEFT JOIN aircraft_operator_flag_code aofc USING (aircraft_operator_flag_code_id) WHERE aircraft_id = $1",
            aircraft.aircraft_id
        )
        .fetch_one(&mut *transaction)
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, "XXX");
        transaction.rollback().await.unwrap();
    }

    async fn reset_aircraft(transaction: &mut Transaction<'_, Postgres>) {
        let aircraft = generate_aircraft();
        aircraft
            .update_transaction(transaction, &ResponseAircraft::from(aircraft.clone()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    /// Assert that unused/danlging values are removed from the database
    async fn model_aircraft_update_remove_unused() {
        let test_setup = test_setup().await;

        // aircraft type
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.aircraft_type = S!("XXX");
        aircraft
            .update_transaction(&mut transaction, &input)
            .await
            .unwrap();
        reset_aircraft(&mut transaction).await;
        let removal = aircraft.remove_unused(&mut transaction).await;
        assert!(removal.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT type AS value
            FROM aircraft_type WHERE type = $1",
            "XXX"
        )
        .fetch_optional(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        transaction.rollback().await.unwrap();

        // aircraft icao type
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.icao_type = S!("XXX");
        aircraft
            .update_transaction(&mut transaction, &input)
            .await
            .unwrap();
        reset_aircraft(&mut transaction).await;
        let removal = aircraft.remove_unused(&mut transaction).await;
        assert!(removal.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT icao_type AS value
            FROM aircraft_icao_type WHERE icao_type = $1",
            "XXX"
        )
        .fetch_optional(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        transaction.rollback().await.unwrap();

        // manufacturer
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.manufacturer = S!("XXX");
        aircraft
            .update_transaction(&mut transaction, &input)
            .await
            .unwrap();
        reset_aircraft(&mut transaction).await;
        let removal = aircraft.remove_unused(&mut transaction).await;
        assert!(removal.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT manufacturer AS value
             FROM aircraft_manufacturer WHERE manufacturer = $1",
            "XXX"
        )
        .fetch_optional(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        transaction.rollback().await.unwrap();

        // registered owner
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner = S!("XXX");
        aircraft
            .update_transaction(&mut transaction, &input)
            .await
            .unwrap();
        reset_aircraft(&mut transaction).await;
        let removal = aircraft.remove_unused(&mut transaction).await;
        assert!(removal.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT registered_owner AS value
               FROM aircraft_registered_owner WHERE registered_owner = $1",
            "XXX"
        )
        .fetch_optional(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        transaction.rollback().await.unwrap();

        // registration
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registration = S!("NXXX");
        aircraft
            .update_transaction(&mut transaction, &input)
            .await
            .unwrap();
        reset_aircraft(&mut transaction).await;
        let removal = aircraft.remove_unused(&mut transaction).await;
        assert!(removal.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT registration AS value
               FROM aircraft_registration WHERE registration = $1",
            "NXXX"
        )
        .fetch_optional(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        transaction.rollback().await.unwrap();

        // Flag code
        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner_operator_flag_code = Some(S!("XXX"));
        aircraft
            .update_transaction(&mut transaction, &input)
            .await
            .unwrap();
        reset_aircraft(&mut transaction).await;
        let removal = aircraft.remove_unused(&mut transaction).await;
        assert!(removal.is_ok());
        let result = sqlx::query_as!(
            TestOutput,
            "SELECT operator_flag_code AS value
               FROM aircraft_operator_flag_code WHERE operator_flag_code = $1",
            "XXX"
        )
        .fetch_optional(&mut *transaction)
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        transaction.rollback().await.unwrap();
    }

    #[tokio::test]
    /// Update fails if registration prefix and country don't match
    async fn model_aircraft_update_country_registration_err() {
        let test_setup = test_setup().await;

        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner_country_name = S!("Vietnam");
        input.registered_owner_country_iso_name = S!("VN");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_err());
        transaction.rollback().await.unwrap();

        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner_country_iso_name = S!("VN");
        input.registration = S!("GXXX");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_err());
        transaction.rollback().await.unwrap();

        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner_country_iso_name = S!("VN");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_err());
        transaction.rollback().await.unwrap();

        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner_country_iso_name = S!("Vietnam");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_err());
        transaction.rollback().await.unwrap();

        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner_country_name = S!("Vietnam");
        input.registered_owner_country_iso_name = S!("VN");
        input.registration = S!("GXXX");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_err());
        transaction.rollback().await.unwrap();
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    /// Registration and country updated correctly when all match
    async fn model_aircraft_update_country_registration_ok() {
        let test_setup = test_setup().await;

        let mut transaction = test_setup.postgres.begin().await.unwrap();
        let aircraft = generate_aircraft();
        let mut input = ResponseAircraft::from(aircraft.clone());
        input.registered_owner_country_name = S!("Vietnam");
        input.registered_owner_country_iso_name = S!("VN");
        input.registration = S!("VNXXX");
        let update = aircraft.update_transaction(&mut transaction, &input).await;
        assert!(update.is_ok());
        transaction.rollback().await.unwrap();
    }
}
