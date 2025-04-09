use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;

use crate::api::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelAirport {
    pub airport_id: i64,
}

impl ModelAirport {
    /// Used for checking that a scraped airport is in db
    pub async fn get(
        db: impl PgExecutor<'_>,
        airport_icao: &str,
    ) -> Result<Option<Self>, AppError> {
        Ok(sqlx::query_as!(
            Self,
            "
SELECT
    airport_id
FROM
    airport
    JOIN airport_icao_code ar USING (airport_icao_code_id)
WHERE
    ar.icao_code = UPPER($1)",
            airport_icao
        )
        .fetch_optional(db)
        .await?)
    }
}
