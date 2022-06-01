use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db_postgres::{ModelAircraft, ModelFlightroute};

pub type AsJsonRes<T> = Json<ResponseJson<T>>;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, PartialOrd)]
pub struct ResponseJson<T> {
    pub response: T,
}

impl<T> ResponseJson<T> {
    pub fn new(response: T) -> Json<ResponseJson<T>> {
        Json(Self { response })
    }
}

/// Response for the /online api route
#[derive(Serialize, Deserialize)]
pub struct Online {
    pub uptime: u64,
    pub api_version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AircraftAndRoute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aircraft: Option<ModelAircraft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flightroute: Option<ModelFlightroute>,
}
