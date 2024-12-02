use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db_postgres::{ModelAircraft, ModelAirline, ModelFlightroute};

pub type AsJsonRes<T> = Json<ResponseJson<T>>;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, PartialOrd)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResponseAircraft {
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

impl From<ModelAircraft> for ResponseAircraft {
    fn from(model: ModelAircraft) -> Self {
        Self {
            aircraft_type: model.aircraft_type,
            icao_type: model.icao_type,
            manufacturer: model.manufacturer,
            mode_s: model.mode_s,
            registration: model.registration,
            registered_owner_country_iso_name: model.registered_owner_country_iso_name,
            registered_owner_country_name: model.registered_owner_country_name,
            registered_owner_operator_flag_code: model.registered_owner_operator_flag_code,
            registered_owner: model.registered_owner,
            url_photo: model.url_photo,
            url_photo_thumbnail: model.url_photo_thumbnail,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResponseAirline {
    pub name: String,
    pub icao: String,
    pub iata: Option<String>,
    pub country: String,
    pub country_iso: String,
    pub callsign: Option<String>,
}

impl From<ModelAirline> for ResponseAirline {
    fn from(model: ModelAirline) -> Self {
        Self {
            name: model.airline_name,
            icao: model.icao_prefix,
            iata: model.iata_prefix,
            country: model.country_name,
            country_iso: model.country_iso_name,
            callsign: model.airline_callsign,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Airline {
    pub name: String,
    pub icao: String,
    pub iata: Option<String>,
    pub country: String,
    pub country_iso: String,
    pub callsign: Option<String>,
}

// should be option none
// impl From<&ModelFlightroute> for Option<Airline> {
impl Airline {
    fn from_model(value: &ModelFlightroute) -> Option<Self> {
        value.airline_name.as_ref().map(|name| Self {
            name: name.clone(),
            icao: value.airline_icao.clone().unwrap_or_default(),
            iata: value.airline_iata.clone(),
            country: value.airline_country_name.clone().unwrap_or_default(),
            country_iso: value.airline_country_iso_name.clone().unwrap_or_default(),
            callsign: value.airline_callsign.clone(),
        })
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
    pub callsign_icao: Option<String>,
    pub callsign_iata: Option<String>,
    pub airline: Option<Airline>,
    pub origin: Airport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midpoint: Option<Airport>,
    pub destination: Airport,
}

impl ResponseFlightRoute {
    pub fn from_model(flightroute_airline: Option<&ModelFlightroute>) -> Option<Self> {
        flightroute_airline.as_ref().map(|flightroute| {
            let airports = Airport::from_model(flightroute);
            Self {
                callsign: flightroute.callsign.clone(),
                callsign_icao: flightroute.callsign_icao.clone(),
                callsign_iata: flightroute.callsign_iata.clone(),
                origin: airports.0,
                airline: Airline::from_model(flightroute),
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
