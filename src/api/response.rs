use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db_postgres::{ModelAircraft, ModelFlightroute};

pub type AsJsonRes<T> = Json<ResponseJson<T>>;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, PartialOrd)]
pub struct ResponseJson<T> {
    pub response: T,
}

impl<T> ResponseJson<T> {
    pub const fn new(response: T) -> Json<Self> {
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

impl From<ModelAircraft> for ResponseAircraft {
    fn from(model: ModelAircraft) -> Self {
        Self {
            aircraft_type: model.aircraft_type,
            icao_type: model.icao_type,
            manufacturer: model.manufacturer,
            mode_s: model.mode_s,
            n_number: model.n_number,
            registered_owner_country_iso_name: model.registered_owner_country_iso_name,
            registered_owner_country_name: model.registered_owner_country_name,
            registered_owner_operator_flag_code: model.registered_owner_operator_flag_code,
            registered_owner: model.registered_owner,
            url_photo: model.url_photo,
            url_photo_thumbnail: model.url_photo_thumbnail,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Airport {
    pub country_iso_name: String,
    pub country_name: String,
    pub elevation: i32,
    pub iata_code: String,
    pub icao_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub municipality: String,
    pub name: String,
}

impl Airport {
    fn from_model(flightroute: &ModelFlightroute) -> (Self, Option<Self>, Self) {
        let origin = Self {
            name: flightroute.origin_airport_name.clone(),
            country_iso_name: flightroute.origin_airport_country_iso_name.clone(),
            country_name: flightroute.origin_airport_country_name.clone(),
            elevation: flightroute.origin_airport_elevation,
            iata_code: flightroute.origin_airport_iata_code.clone(),
            icao_code: flightroute.origin_airport_icao_code.clone(),
            latitude: flightroute.origin_airport_latitude,
            longitude: flightroute.origin_airport_longitude,
            municipality: flightroute.origin_airport_municipality.clone(),
        };

        // This is a messy way to do it, but it works
        // If midpoint_airport_name is_some, then all midpoint values are some
        let midpoint = if flightroute.midpoint_airport_name.is_some() {
            Some(Self {
                name: flightroute
                    .midpoint_airport_name
                    .clone()
                    .unwrap_or_default(),
                country_iso_name: flightroute
                    .midpoint_airport_country_iso_name
                    .clone()
                    .unwrap_or_default(),
                country_name: flightroute
                    .midpoint_airport_country_name
                    .clone()
                    .unwrap_or_default(),
                elevation: flightroute.midpoint_airport_elevation.unwrap_or_default(),
                iata_code: flightroute
                    .midpoint_airport_iata_code
                    .clone()
                    .unwrap_or_default(),
                icao_code: flightroute
                    .midpoint_airport_icao_code
                    .clone()
                    .unwrap_or_default(),
                latitude: flightroute.midpoint_airport_latitude.unwrap_or_default(),
                longitude: flightroute.midpoint_airport_longitude.unwrap_or_default(),
                municipality: flightroute
                    .midpoint_airport_municipality
                    .clone()
                    .unwrap_or_default(),
            })
        } else {
            None
        };

        let destination = Self {
            name: flightroute.destination_airport_name.clone(),
            country_iso_name: flightroute.destination_airport_country_iso_name.clone(),
            country_name: flightroute.destination_airport_country_name.clone(),
            elevation: flightroute.destination_airport_elevation,
            iata_code: flightroute.destination_airport_iata_code.clone(),
            icao_code: flightroute.destination_airport_icao_code.clone(),
            latitude: flightroute.destination_airport_latitude,
            longitude: flightroute.destination_airport_longitude,
            municipality: flightroute.destination_airport_municipality.clone(),
        };
        (origin, midpoint, destination)
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ResponseFlightRoute {
    pub callsign: String,
    pub origin: Airport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint: Option<Airport>,
    pub destination: Airport,
}

impl ResponseFlightRoute {
    pub fn from_model(op_flightroute: &Option<ModelFlightroute>) -> Option<Self> {
        op_flightroute.as_ref().map(|flightroute| {
            let airports = Airport::from_model(flightroute);
            Self {
                callsign: flightroute.callsign.clone(),
                origin: airports.0,
                midpoint: airports.1,
                destination: airports.2,
            }
        })
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct AircraftAndRoute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aircraft: Option<ResponseAircraft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flightroute: Option<ResponseFlightRoute>,
}
