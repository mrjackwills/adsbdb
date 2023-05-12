use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::api::AppError;

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelAirport {
    pub airport_id: i64,
}

impl ModelAirport {
    /// Used for checking that a scraped airport is in db
    pub async fn get(db: &PgPool, airport_icao: &str) -> Result<Option<Self>, AppError> {
        let query = r#"
SELECT
    airport_id
FROM
    airport
LEFT JOIN
    airport_icao_code ar USING(airport_icao_code_id)
WHERE
    ar.icao_code = $1"#;
        Ok(sqlx::query_as::<_, Self>(query)
            .bind(airport_icao)
            .fetch_optional(db)
            .await?)
    }
}
