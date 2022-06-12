use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Error, PgPool};

use crate::api::AppError;

use super::Model;

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelAirport {
    pub airport_icao_code_id: i64,
}

#[async_trait]
impl Model<Self> for ModelAirport {
    /// Used for checking that a scraped airport is in db
    async fn get(db: &PgPool, airpot_icao: &str) -> Result<Option<Self>, AppError> {
        let query = r#"
SELECT
	airport_icao_code_id
FROM
	airport_icao_code
WHERE
	icao_code = $1"#;
        Ok(sqlx::query_as::<_, Self>(query)
            .bind(airpot_icao)
            .fetch_optional(db)
            .await?)
    }
}
