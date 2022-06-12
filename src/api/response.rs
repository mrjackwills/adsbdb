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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResponseAircraft {
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

impl ResponseAircraft {
    pub fn from(a: ModelAircraft) -> Self {
        Self {
            aircraft_type: a.aircraft_type,
            icao_type: a.icao_type,
            manufacturer: a.manufacturer,
            mode_s: a.mode_s,
            n_number: a.n_number,
            registered_owner_country_iso_name: a.registered_owner_country_iso_name,
            registered_owner_country_name: a.registered_owner_country_name,
            registered_owner_operator_flag_code: a.registered_owner_operator_flag_code,
            registered_owner: a.registered_owner,
            url_photo: a.url_photo,
            url_photo_thumbnail: a.url_photo_thumbnail,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResponseFlightRoute {
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint_airport_country_iso_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint_airport_country_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint_airport_elevation: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint_airport_iata_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint_airport_icao_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint_airport_latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint_airport_longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint_airport_municipality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

impl ResponseFlightRoute {
    pub fn from(op_fl: Option<ModelFlightroute>) -> Option<Self> {
        if let Some(fl) = op_fl {
            Some(Self {
                callsign: fl.callsign,
                origin_airport_country_iso_name: fl.origin_airport_country_iso_name,
                origin_airport_country_name: fl.origin_airport_country_name,
                origin_airport_elevation: fl.origin_airport_elevation,
                origin_airport_iata_code: fl.origin_airport_iata_code,
                origin_airport_icao_code: fl.origin_airport_icao_code,
                origin_airport_latitude: fl.origin_airport_latitude,
                origin_airport_longitude: fl.origin_airport_longitude,
                origin_airport_municipality: fl.origin_airport_municipality,
                origin_airport_name: fl.origin_airport_name,
                midpoint_airport_country_iso_name: fl.midpoint_airport_country_iso_name,
                midpoint_airport_country_name: fl.midpoint_airport_country_name,
                midpoint_airport_elevation: fl.midpoint_airport_elevation,
                midpoint_airport_iata_code: fl.midpoint_airport_iata_code,
                midpoint_airport_icao_code: fl.midpoint_airport_icao_code,
                midpoint_airport_latitude: fl.midpoint_airport_latitude,
                midpoint_airport_longitude: fl.midpoint_airport_longitude,
                midpoint_airport_municipality: fl.midpoint_airport_municipality,
                midpoint_airport_name: fl.midpoint_airport_name,
                destination_airport_country_iso_name: fl.destination_airport_country_iso_name,
                destination_airport_country_name: fl.destination_airport_country_name,
                destination_airport_elevation: fl.destination_airport_elevation,
                destination_airport_iata_code: fl.destination_airport_iata_code,
                destination_airport_icao_code: fl.destination_airport_icao_code,
                destination_airport_latitude: fl.destination_airport_latitude,
                destination_airport_longitude: fl.destination_airport_longitude,
                destination_airport_municipality: fl.destination_airport_municipality,
                destination_airport_name: fl.destination_airport_name,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AircraftAndRoute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aircraft: Option<ResponseAircraft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flightroute: Option<ResponseFlightRoute>,
}
