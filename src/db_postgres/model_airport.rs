use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::api::AppError;

// use super::Model;

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelAirport {
    pub airport_icao_code_id: i64,
}

impl ModelAirport {
    /// Used for checking that a scraped airport is in db
    pub async fn get(db: &PgPool, airport_icao: &str) -> Result<Option<Self>, AppError> {
        let query = r#"
SELECT
	airport_icao_code_id
FROM
	airport_icao_code
WHERE
	icao_code = $1"#;
        Ok(sqlx::query_as::<_, Self>(query)
            .bind(airport_icao)
            .fetch_optional(db)
            .await?)
    }
}
