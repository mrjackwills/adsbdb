use std::collections::HashMap;

use axum::{
    extract::{OriginalUri, State},
    http::StatusCode,
};

use super::input::{AircraftSearch, AirlineCode, Callsign, ModeS, NNumber, Validate};
use super::response::{
    AircraftAndRoute, AsJsonRes, Online, ResponseAircraft, ResponseAirline, ResponseFlightRoute,
    ResponseJson,
};
use super::{app_error::UnknownAC, AppError, ApplicationState};
use crate::n_number::{mode_s_to_n_number, n_number_to_mode_s};
use crate::{
    db_postgres::{ModelAircraft, ModelAirline, ModelFlightroute},
    db_redis::{get_cache, insert_cache, RedisKey},
};

/// Get flightroute, refactored so can use in either `get_mode_s` (with a callsign query param), or `get_callsign`.
/// Check redis cache for Option\<ModelFlightroute>, else query postgres
async fn find_flightroute(
    state: ApplicationState,
    callsign: &Callsign,
) -> Result<Option<ModelFlightroute>, AppError> {
    let redis_key = RedisKey::Callsign(callsign);
    if let Some(flightroute) = get_cache::<ModelFlightroute>(&state.redis, &redis_key).await? {
        flightroute.map_or(Err(AppError::UnknownInDb(UnknownAC::Callsign)), |route| {
            Ok(Some(route))
        })
    } else {
        let mut flightroute = ModelFlightroute::get(&state.postgres, callsign).await;
        if flightroute.is_none() {
            flightroute = state
                .scraper
                .scrape_flightroute(&state.postgres, callsign, &state.scraper_threads)
                .await?;
        }
        insert_cache(&state.redis, &flightroute, &redis_key).await?;
        Ok(flightroute)
    }
}

/// Check redis cache for Option\<ModelAircraft>, else query postgres
async fn find_aircraft(
    state: ApplicationState,
    aircraft_search: &AircraftSearch,
) -> Result<Option<ModelAircraft>, AppError> {
    let redis_key = RedisKey::from(aircraft_search);

    if let Some(aircraft) = get_cache::<ModelAircraft>(&state.redis, &redis_key).await? {
        aircraft.map_or(Err(AppError::UnknownInDb(UnknownAC::Aircraft)), |craft| {
            Ok(Some(craft))
        })
    } else {
        let mut aircraft =
            ModelAircraft::get(&state.postgres, aircraft_search, &state.url_prefix).await?;
        if let Some(craft) = aircraft.as_ref() {
            if craft.url_photo.is_none() {
                state
                    .scraper
                    .scrape_photo(&state.postgres, craft, &state.scraper_threads)
                    .await;
                aircraft =
                    ModelAircraft::get(&state.postgres, aircraft_search, &state.url_prefix).await?;
            }
        }
        insert_cache(&state.redis, &aircraft, &redis_key).await?;
        Ok(aircraft)
    }
}

/// Check redis cache for Option\<ModelAircraft>, else query postgres
async fn find_airline(
    state: ApplicationState,
    airline: &AirlineCode,
) -> Result<Option<Vec<ModelAirline>>, AppError> {
    let redis_key = RedisKey::Airline(airline);

    if let Some(airline) = get_cache::<Vec<ModelAirline>>(&state.redis, &redis_key).await? {
        airline.map_or(Err(AppError::UnknownInDb(UnknownAC::Airline)), |airline| {
            Ok(Some(airline))
        })
    } else {
        let airline = ModelAirline::get_all_by_airline_code(&state.postgres, airline).await?;
        insert_cache(&state.redis, &airline, &redis_key).await?;
        Ok(airline)
    }
}

/// Return an aircraft detail from a modes input
/// optional query param of callsign, so can get both aircraft and flightroute in a single request
/// TODO turn this optional into an extractor?
pub async fn aircraft_get(
    State(state): State<ApplicationState>,
    aircraft_search: AircraftSearch,
    axum::extract::Query(queries): axum::extract::Query<HashMap<String, String>>,
) -> Result<(StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
    // Check if optional callsign query param
    if let Some(query_param) = queries.get("callsign") {
        let callsign = Callsign::validate(query_param)?;
        let (aircraft, flightroute) = tokio::try_join!(
            find_aircraft(state.clone(), &aircraft_search),
            find_flightroute(state, &callsign),
        )?;
        aircraft.map_or(Err(AppError::UnknownInDb(UnknownAC::Aircraft)), |a| {
            Ok((
                StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: Some(ResponseAircraft::from(a)),
                    flightroute: ResponseFlightRoute::from_model(&flightroute),
                }),
            ))
        })
    } else {
        find_aircraft(state, &aircraft_search).await?.map_or(
            Err(AppError::UnknownInDb(UnknownAC::Aircraft)),
            |aircraft| {
                Ok((
                    StatusCode::OK,
                    ResponseJson::new(AircraftAndRoute {
                        aircraft: Some(ResponseAircraft::from(aircraft)),
                        flightroute: None,
                    }),
                ))
            },
        )
    }
}

/// Return an airline detail from a ICAO or IATA airline prefix
pub async fn airline_get(
    State(state): State<ApplicationState>,
    airline_code: AirlineCode,
) -> Result<(axum::http::StatusCode, AsJsonRes<Vec<ResponseAirline>>), AppError> {
    find_airline(state, &airline_code).await?.map_or(
        Err(AppError::UnknownInDb(UnknownAC::Airline)),
        |a| {
            Ok((
                StatusCode::OK,
                ResponseJson::new(a.into_iter().map(ResponseAirline::from).collect::<Vec<_>>()),
            ))
        },
    )
}

/// Return a flightroute detail from a callsign input
pub async fn callsign_get(
    State(state): State<ApplicationState>,
    callsign: Callsign,
) -> Result<(axum::http::StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
    find_flightroute(state, &callsign).await?.map_or(
        Err(AppError::UnknownInDb(UnknownAC::Callsign)),
        |a| {
            Ok((
                StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: None,
                    flightroute: ResponseFlightRoute::from_model(&Some(a)),
                }),
            ))
        },
    )
}

/// Route to convert N-Number to Mode_S
#[allow(clippy::unused_async)]
pub async fn n_number_get(
    n_number: NNumber,
) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
    Ok((
        StatusCode::OK,
        ResponseJson::new(n_number_to_mode_s(&n_number).map_or(String::new(), |f| f.to_string())),
    ))
}

/// Route to convert Mode_S to N-Number
#[allow(clippy::unused_async)]
pub async fn mode_s_get(
    mode_s: ModeS,
) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
    Ok((
        StatusCode::OK,
        ResponseJson::new(mode_s_to_n_number(&mode_s).map_or(String::new(), |f| f.to_string())),
    ))
}

/// Return a simple online status response
#[allow(clippy::unused_async)]
pub async fn online_get(
    State(state): State<ApplicationState>,
) -> (axum::http::StatusCode, AsJsonRes<Online>) {
    (
        StatusCode::OK,
        ResponseJson::new(Online {
            uptime: state.uptime.elapsed().as_secs(),
            api_version: env!("CARGO_PKG_VERSION").into(),
        }),
    )
}

/// return a unknown endpoint response
#[allow(clippy::unused_async)]
pub async fn fallback(OriginalUri(original_uri): OriginalUri) -> (StatusCode, AsJsonRes<String>) {
    (
        StatusCode::NOT_FOUND,
        ResponseJson::new(format!("unknown endpoint: {original_uri}")),
    )
}

/// ApiRoutes tests
/// cargo watch -q -c -w src/ -x 'test http_api -- --test-threads=1 --nocapture'
#[cfg(test)]
#[allow(
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used,
    unused_must_use
)]
mod tests {
    use std::sync::Arc;

    use super::*;

    use axum::http::Uri;
    use fred::error::RedisError;
    use fred::interfaces::HashesInterface;
    use fred::interfaces::KeysInterface;
    use fred::interfaces::ServerInterface;
    use tokio::sync::Mutex;

    use crate::api::input::Validate;
    use crate::api::response::Airline;
    use crate::api::response::Airport;
    use crate::api::Registration;
    use crate::db_postgres;
    use crate::db_redis;
    use crate::parse_env;
    use crate::scraper::ScraperThreadMap;
    use crate::sleep;

    const CALLSIGN: &str = "ANA460";

    async fn get_application_state() -> State<ApplicationState> {
        let app_env = parse_env::AppEnv::get_env();
        let postgres = db_postgres::get_pool(&app_env).await.unwrap();
        let redis = db_redis::get_pool(&app_env).await.unwrap();
        let scraper_threads = Arc::new(Mutex::new(ScraperThreadMap::new()));
        redis.flushall::<()>(true).await.unwrap();
        State(ApplicationState::new(
            &app_env,
            postgres,
            redis,
            scraper_threads,
        ))
    }

    #[tokio::test]
    // basically a 404 handler
    async fn http_api_fallback_route() {
        let uri = "/test/uri".parse::<Uri>().unwrap();
        let response = fallback(OriginalUri(uri.clone())).await;
        assert_eq!(response.0, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(response.1.response, format!("unknown endpoint: {uri}"));
    }

    #[tokio::test]
    async fn http_api_online_route() {
        let application_state = get_application_state().await;

        sleep!();
        let response = online_get(application_state).await;

        assert_eq!(response.0, axum::http::StatusCode::OK);
        assert_eq!(env!("CARGO_PKG_VERSION"), response.1.response.api_version);
        assert!(response.1.response.uptime >= 1);
    }

    #[tokio::test]
    async fn http_api_n_number_route() {
        let n_number = NNumber::validate("N123AB").unwrap();
        let response = n_number_get(n_number).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        assert_eq!(response.1.response, "A05ED9");
    }

    #[tokio::test]
    async fn http_api_get_mode_s_ok_with_photo() {
        let mode_s = "A44F3B".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ResponseAircraft {
            aircraft_type: "Citation Sovereign".to_owned(),
            icao_type: "C680".to_owned(),
            manufacturer: "Cessna".to_owned(),
            mode_s,
            registration: "N377QS".to_owned(),
            registered_owner: "NetJets".to_owned(),
            registered_owner_operator_flag_code: "EJA".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: Some(format!(
                "{}{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
            url_photo_thumbnail: Some(format!(
                "{}thumbnails/{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
        };

        match &response.1.response.aircraft {
            Some(x) => assert_eq!(x, &aircraft),
            None => unreachable!(),
        }

        assert!(response.1.response.flightroute.is_none());
    }

    #[tokio::test]
    async fn http_api_get_registration_ok_with_photo() {
        let registration = "N377QS".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ResponseAircraft {
            aircraft_type: "Citation Sovereign".to_owned(),
            icao_type: "C680".to_owned(),
            manufacturer: "Cessna".to_owned(),
            mode_s: "A44F3B".to_owned(),
            registration,
            registered_owner: "NetJets".to_owned(),
            registered_owner_operator_flag_code: "EJA".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: Some(format!(
                "{}{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
            url_photo_thumbnail: Some(format!(
                "{}thumbnails/{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
        };

        match &response.1.response.aircraft {
            Some(x) => assert_eq!(x, &aircraft),
            None => unreachable!(),
        }

        assert!(response.1.response.flightroute.is_none());
    }

    #[tokio::test]
    async fn http_api_get_mode_s_ok_no_photo() {
        let mode_s = "A44917";
        let path = AircraftSearch::ModeS(ModeS::validate(mode_s).unwrap());
        let application_state = get_application_state().await;
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ResponseAircraft {
            aircraft_type: "737MAX 9".to_owned(),
            icao_type: "B39M".to_owned(),
            manufacturer: "Boeing".to_owned(),
            mode_s: mode_s.to_owned(),
            registration: "N37522".to_owned(),
            registered_owner: "United Airlines".to_owned(),
            registered_owner_operator_flag_code: "UAL".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        match &response.1.response.aircraft {
            Some(x) => assert_eq!(x, &aircraft),
            None => unreachable!(),
        }

        assert!(response.1.response.flightroute.is_none());
    }

    #[tokio::test]
    async fn http_api_get_registration_ok_no_photo() {
        let registration = "N37522";
        let path = AircraftSearch::Registration(Registration::validate(registration).unwrap());
        let application_state = get_application_state().await;
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ResponseAircraft {
            aircraft_type: "737MAX 9".to_owned(),
            icao_type: "B39M".to_owned(),
            manufacturer: "Boeing".to_owned(),
            mode_s: "A44917".to_owned(),
            registration: registration.to_owned(),
            registered_owner: "United Airlines".to_owned(),
            registered_owner_operator_flag_code: "UAL".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        match &response.1.response.aircraft {
            Some(x) => assert_eq!(x, &aircraft),
            None => unreachable!(),
        }

        assert!(response.1.response.flightroute.is_none());
    }

    #[tokio::test]
    // Make sure aircraft is inserted correctly into redis cache and has ttl of 604800
    // this is with photo, need to use A44917 for without photo
    async fn http_api_get_mode_s_cached_with_photo() {
        let mode_s = "A44F3B";
        let tmp_mode_s = ModeS::validate(mode_s).unwrap();
        let key = RedisKey::ModeS(&tmp_mode_s);
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(mode_s).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap();

        let result: Result<String, fred::error::RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: ResponseAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    // Make sure aircraft is inserted correctly into redis cache and has ttl of 604800
    // this is with photo, need to use A44917 for without photo
    async fn http_api_get_registration_cached_with_photo() {
        let registration = "N377QS";
        let tmp_registration = Registration::validate(registration).unwrap();
        let key = RedisKey::Registration(&tmp_registration);
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(registration).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap();

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: ResponseAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_mode_s_cached_no_photo() {
        let mode_s = "A44917".to_owned();
        let tmp_mode_s = ModeS::validate(&mode_s).unwrap();
        let key = RedisKey::ModeS(&tmp_mode_s);
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap();

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: ResponseAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_registration_cached_no_photo() {
        let registration = "N37522".to_owned();
        let tmp_registration = Registration::validate(&registration).unwrap();
        let key = RedisKey::Registration(&tmp_registration);
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap();
        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: ResponseAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state
            .redis
            .clone()
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    // Make sure unknown aircraft gets placed into cache as ""
    // and a second request will extend the ttl
    async fn http_api_get_mode_s_unknown_cached() {
        let mode_s = "ABABAB".to_owned();
        let tmp_mode_s = ModeS::validate(&mode_s).unwrap();
        let key = RedisKey::ModeS(&tmp_mode_s);
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path.clone(), hm)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Aircraft),
            _ => unreachable!(),
        };
        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state
            .redis
            .clone()
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);

        sleep!();

        // make sure a second request to an unknown mode_s will extend cache ttl
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Aircraft),
            _ => unreachable!(),
        };

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    // Make sure unknown aircraft gets placed into cache as ""
    // and a second request will extend the ttl
    async fn http_api_get_registration_unknown_cached() {
        let registration = "AB-ABAB".to_owned();
        let tmp_registration = Registration::validate(&registration).unwrap();
        let key = RedisKey::Registration(&tmp_registration);
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path.clone(), hm)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Aircraft),
            _ => unreachable!(),
        };
        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);

        sleep!();

        // make sure a second request to an unknown mode_s will extend cache ttl
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Aircraft),
            _ => unreachable!(),
        };

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_icao_callsign_ok() {
        let callsign = "ACA959";
        let application_state = get_application_state().await;
        let path = Callsign::validate(callsign).unwrap();
        let response = callsign_get(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some("AC959".to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        assert!(response.1.response.aircraft.is_none());
    }

    #[tokio::test]
    async fn http_api_get_iata_callsign_ok() {
        let callsign = "AC959";
        let application_state = get_application_state().await;
        let path = Callsign::validate(callsign).unwrap();
        let response = callsign_get(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some("ACA959".to_owned()),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        assert!(response.1.response.aircraft.is_none());
    }

    #[tokio::test]
    async fn http_api_get_icao_callsign_with_midpoint_ok() {
        let callsign = "QFA31";
        let application_state = get_application_state().await;
        let path = Callsign::validate(callsign).unwrap();
        let response = callsign_get(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_iata: Some("QF31".to_owned()),
            callsign_icao: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: "Qantas".to_owned(),
                icao: "QFA".to_owned(),
                iata: Some("QF".to_owned()),
                callsign: Some("QANTAS".to_owned()),
                country: "Australia".to_owned(),
                country_iso: "AU".to_owned(),
            }),
            origin: Airport {
                country_iso_name: "AU".to_owned(),
                country_name: "Australia".to_owned(),
                elevation: 21,
                iata_code: "SYD".to_owned(),
                icao_code: "YSSY".to_owned(),
                latitude: -33.946_098_327_636_72,
                longitude: 151.177_001_953_125,
                municipality: "Sydney".to_owned(),
                name: "Sydney Kingsford Smith International Airport".to_owned(),
            },
            midpoint: Some(Airport {
                country_iso_name: "SG".to_owned(),
                country_name: "Singapore".to_owned(),
                elevation: 22,
                iata_code: "SIN".to_owned(),
                icao_code: "WSSS".to_owned(),
                latitude: 1.35019,
                longitude: 103.994_003,
                municipality: "Singapore".to_owned(),
                name: "Singapore Changi Airport".to_owned(),
            }),
            destination: Airport {
                country_iso_name: "GB".to_owned(),
                country_name: "United Kingdom".to_owned(),
                elevation: 83,
                iata_code: "LHR".to_owned(),
                icao_code: "EGLL".to_owned(),
                latitude: 51.4706,
                longitude: -0.461_941,
                municipality: "London".to_owned(),
                name: "London Heathrow Airport".to_owned(),
            },
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        assert!(response.1.response.aircraft.is_none());
    }

    #[tokio::test]
    async fn http_api_get_iata_callsign_with_midpoint_ok() {
        let callsign = "QF31";
        let application_state = get_application_state().await;
        let path = Callsign::validate(callsign).unwrap();
        let response = callsign_get(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_iata: Some(callsign.to_owned()),
            callsign_icao: Some("QFA31".to_owned()),
            airline: Some(Airline {
                name: "Qantas".to_owned(),
                icao: "QFA".to_owned(),
                iata: Some("QF".to_owned()),
                callsign: Some("QANTAS".to_owned()),
                country: "Australia".to_owned(),
                country_iso: "AU".to_owned(),
            }),
            origin: Airport {
                country_iso_name: "AU".to_owned(),
                country_name: "Australia".to_owned(),
                elevation: 21,
                iata_code: "SYD".to_owned(),
                icao_code: "YSSY".to_owned(),
                latitude: -33.946_098_327_636_72,
                longitude: 151.177_001_953_125,
                municipality: "Sydney".to_owned(),
                name: "Sydney Kingsford Smith International Airport".to_owned(),
            },
            midpoint: Some(Airport {
                country_iso_name: "SG".to_owned(),
                country_name: "Singapore".to_owned(),
                elevation: 22,
                iata_code: "SIN".to_owned(),
                icao_code: "WSSS".to_owned(),
                latitude: 1.35019,
                longitude: 103.994_003,
                municipality: "Singapore".to_owned(),
                name: "Singapore Changi Airport".to_owned(),
            }),
            destination: Airport {
                country_iso_name: "GB".to_owned(),
                country_name: "United Kingdom".to_owned(),
                elevation: 83,
                iata_code: "LHR".to_owned(),
                icao_code: "EGLL".to_owned(),
                latitude: 51.4706,
                longitude: -0.461_941,
                municipality: "London".to_owned(),
                name: "London Heathrow Airport".to_owned(),
            },
        };
        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        assert!(response.1.response.aircraft.is_none());
    }

    #[tokio::test]
    /// Make sure flightroute is inserted correctly into redis cache and has ttl of 604800
    async fn http_api_get_callsign_cached() {
        let callsign = "AC959";
        let application_state = get_application_state().await;
        let path = Callsign::validate(callsign).unwrap();
        callsign_get(application_state.clone(), path).await.unwrap();

        let tmp_callsign = Callsign::validate(callsign).unwrap();
        let key = RedisKey::Callsign(&tmp_callsign);

        let result: Result<ModelFlightroute, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        let result = result.unwrap();

        assert_eq!(result.airline_callsign, Some("AIR CANADA".to_owned()));
        assert_eq!(result.airline_country_iso_name, Some("CA".to_owned()));
        assert_eq!(result.airline_country_name, Some("Canada".to_owned()));
        assert_eq!(result.airline_iata, Some("AC".to_owned()));
        assert_eq!(result.airline_icao, Some("ACA".to_owned()));
        assert_eq!(result.airline_name, Some("Air Canada".to_owned()));

        assert_eq!(result.callsign, callsign);
        assert_eq!(result.origin_airport_country_iso_name, "CA");
        assert_eq!(result.origin_airport_country_name, "Canada");
        assert_eq!(result.origin_airport_elevation, 118);
        assert_eq!(result.origin_airport_iata_code, "YUL");
        assert_eq!(result.origin_airport_icao_code, "CYUL");
        assert_eq!(result.origin_airport_latitude, 45.470_600_128_2);
        assert_eq!(result.origin_airport_longitude, -73.740_798_950_2);
        assert_eq!(result.origin_airport_municipality, "Montréal");
        assert_eq!(
            result.origin_airport_name,
            "Montreal / Pierre Elliott Trudeau International Airport"
        );
        assert!(result.midpoint_airport_country_iso_name.is_none());
        assert!(result.midpoint_airport_country_name.is_none());
        assert!(result.midpoint_airport_elevation.is_none());
        assert!(result.midpoint_airport_iata_code.is_none());
        assert!(result.midpoint_airport_icao_code.is_none());
        assert!(result.midpoint_airport_latitude.is_none());
        assert!(result.midpoint_airport_longitude.is_none());
        assert!(result.midpoint_airport_municipality.is_none());
        assert!(result.midpoint_airport_name.is_none());
        assert_eq!(result.destination_airport_country_iso_name, "CR");
        assert_eq!(result.destination_airport_country_name, "Costa Rica");
        assert_eq!(result.destination_airport_elevation, 3021);
        assert_eq!(result.destination_airport_iata_code, "SJO");
        assert_eq!(result.destination_airport_icao_code, "MROC");
        assert_eq!(result.destination_airport_latitude, 9.993_86);
        assert_eq!(result.destination_airport_longitude, -84.208_801);
        assert_eq!(
            result.destination_airport_municipality,
            "San José (Alajuela)"
        );
        assert_eq!(
            result.destination_airport_name,
            "Juan Santamaría International Airport"
        );

        let ttl: usize = application_state
            .redis
            .clone()
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    // Make sure flightroute is inserted correctly into redis cache and has ttl of 604800
    async fn http_api_get_midpoint_callsign_cached() {
        let callsign = "QFA31";
        let application_state = get_application_state().await;
        let path = Callsign::validate(callsign).unwrap();
        callsign_get(application_state.clone(), path).await.unwrap();
        let tmp_callsign = Callsign::validate(callsign).unwrap();
        let key = RedisKey::Callsign(&tmp_callsign);

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        let result: ModelFlightroute = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(result.callsign, callsign);

        assert_eq!(result.airline_callsign, Some("QANTAS".to_owned()));
        assert_eq!(result.airline_country_iso_name, Some("AU".to_owned()));
        assert_eq!(result.airline_country_name, Some("Australia".to_owned()));
        assert_eq!(result.airline_iata, Some("QF".to_owned()));
        assert_eq!(result.airline_icao, Some("QFA".to_owned()));
        assert_eq!(result.airline_name, Some("Qantas".to_owned()));

        assert_eq!(result.origin_airport_country_iso_name, "AU");
        assert_eq!(result.origin_airport_country_name, "Australia");
        assert_eq!(result.origin_airport_elevation, 21);
        assert_eq!(result.origin_airport_iata_code, "SYD");
        assert_eq!(result.origin_airport_icao_code, "YSSY");
        assert_eq!(result.origin_airport_latitude, -33.946_098_327_636_72);
        assert_eq!(result.origin_airport_longitude, 151.177_001_953_125);
        assert_eq!(result.origin_airport_municipality, "Sydney");
        assert_eq!(
            result.origin_airport_name,
            "Sydney Kingsford Smith International Airport"
        );

        assert_eq!(
            result.midpoint_airport_country_iso_name,
            Some("SG".to_owned())
        );
        assert_eq!(
            result.midpoint_airport_country_name,
            Some("Singapore".to_owned())
        );
        assert_eq!(result.midpoint_airport_elevation, Some(22));
        assert_eq!(result.midpoint_airport_iata_code, Some("SIN".to_owned()));
        assert_eq!(result.midpoint_airport_icao_code, Some("WSSS".to_owned()));
        assert_eq!(result.midpoint_airport_latitude, Some(1.35019));
        assert_eq!(result.midpoint_airport_longitude, Some(103.994_003));
        assert_eq!(
            result.midpoint_airport_municipality,
            Some("Singapore".to_owned())
        );
        assert_eq!(
            result.midpoint_airport_name,
            Some("Singapore Changi Airport".to_owned())
        );

        assert_eq!(result.destination_airport_country_iso_name, "GB");
        assert_eq!(result.destination_airport_country_name, "United Kingdom");
        assert_eq!(result.destination_airport_elevation, 83);
        assert_eq!(result.destination_airport_iata_code, "LHR");
        assert_eq!(result.destination_airport_icao_code, "EGLL");
        assert_eq!(result.destination_airport_latitude, 51.4706);
        assert_eq!(result.destination_airport_longitude, -0.461_941);
        assert_eq!(result.destination_airport_municipality, "London");
        assert_eq!(result.destination_airport_name, "London Heathrow Airport");

        let ttl: usize = application_state
            .redis
            .clone()
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    /// Insert a new flightroute using the scraper
    async fn http_api_get_callsign_scraper() {
        let application_state = get_application_state().await;
        let path = Callsign::validate(CALLSIGN).unwrap();

        let response = callsign_get(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let expected = ResponseFlightRoute {
            callsign: "ANA460".to_owned(),
            callsign_icao: Some("ANA460".to_owned()),
            callsign_iata: Some("NH460".to_owned()),
            airline: Some(Airline {
                name: "All Nippon Airways".to_owned(),
                icao: "ANA".to_owned(),
                iata: Some("NH".to_owned()),
                country: "Japan".to_owned(),
                country_iso: "JP".to_owned(),
                callsign: Some("ALL NIPPON".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "JP".to_owned(),
                country_name: "Japan".to_owned(),
                elevation: 12,
                iata_code: "OKA".to_owned(),
                icao_code: "ROAH".to_owned(),
                latitude: 26.195_801,
                longitude: 127.646_004,
                municipality: "Naha".to_owned(),
                name: "Naha Airport / JASDF Naha Air Base".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "JP".to_owned(),
                country_name: "Japan".to_owned(),
                elevation: 35,
                iata_code: "HND".to_owned(),
                icao_code: "RJTT".to_owned(),
                latitude: 35.552_299,
                longitude: 139.779_999,
                municipality: "Tokyo".to_owned(),
                name: "Tokyo Haneda International Airport".to_owned(),
            },
        };

        assert!(response.1.response.flightroute.is_some());
        let result = response.1.response.flightroute.clone().unwrap();
        assert_eq!(result, expected);
    }

    #[tokio::test]
    /// Make sure that an unknown flightroute is inserted correctly into redis cache as NULL and has ttl of 604800
    /// and another request extends the tll to 604800 again
    async fn http_api_get_callsign_none_cached() {
        let callsign = "ABABAB";
        let application_state = get_application_state().await;
        let path = Callsign::validate(callsign).unwrap();

        let response = callsign_get(application_state.clone(), path.clone())
            .await
            .unwrap_err();
        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Callsign),
            _ => unreachable!(),
        };
        let tmp_callsign = Callsign::validate(callsign).unwrap();
        let key = RedisKey::Callsign(&tmp_callsign);

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);

        sleep!();

        // Check second request is also in redis, and cache ttl gets reset
        let response = callsign_get(application_state.clone(), path)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Callsign),
            _ => unreachable!(),
        };

        let key = RedisKey::Callsign(&tmp_callsign);
        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_icao_callsign_and_flightroute_mode_s_ok_with_photo() {
        let callsign = "ACA959".to_owned();
        let mode_s = "A44F3B".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some("AC959".to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: "Citation Sovereign".to_owned(),
            icao_type: "C680".to_owned(),
            manufacturer: "Cessna".to_owned(),
            mode_s: mode_s.clone(),
            registration: "N377QS".to_owned(),
            registered_owner: "NetJets".to_owned(),
            registered_owner_operator_flag_code: "EJA".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: Some(format!(
                "{}{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
            url_photo_thumbnail: Some(format!(
                "{}thumbnails/{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        match &response.1.response.aircraft {
            Some(d) => assert_eq!(d, &aircraft),
            None => unreachable!(),
        }
    }

    #[tokio::test]
    async fn http_api_get_iata_callsign_and_flightroute_mode_s_ok_with_photo() {
        let callsign = "AC959".to_owned();
        let mode_s = "A44F3B".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some("ACA959".to_owned()),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: "Citation Sovereign".to_owned(),
            icao_type: "C680".to_owned(),
            manufacturer: "Cessna".to_owned(),
            mode_s: mode_s.clone(),
            registration: "N377QS".to_owned(),
            registered_owner: "NetJets".to_owned(),
            registered_owner_operator_flag_code: "EJA".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: Some(format!(
                "{}{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
            url_photo_thumbnail: Some(format!(
                "{}thumbnails/{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        match &response.1.response.aircraft {
            Some(d) => assert_eq!(d, &aircraft),
            None => unreachable!(),
        }
    }

    #[tokio::test]
    async fn http_api_get_icao_callsign_and_flightroute_registration_ok_with_photo() {
        let callsign = "ACA959".to_owned();
        let registration = "N377QS".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some("AC959".to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };
        let aircraft = ResponseAircraft {
            aircraft_type: "Citation Sovereign".to_owned(),
            icao_type: "C680".to_owned(),
            manufacturer: "Cessna".to_owned(),
            mode_s: "A44F3B".to_owned(),
            registration: registration.to_owned(),
            registered_owner: "NetJets".to_owned(),
            registered_owner_operator_flag_code: "EJA".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: Some(format!(
                "{}{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
            url_photo_thumbnail: Some(format!(
                "{}thumbnails/{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        match &response.1.response.aircraft {
            Some(d) => assert_eq!(d, &aircraft),
            None => unreachable!(),
        }
    }

    #[tokio::test]
    async fn http_api_get_iata_callsign_and_flightroute_registration_ok_with_photo() {
        let callsign = "AC959".to_owned();
        let registration = "N377QS".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some("ACA959".to_owned()),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };
        let aircraft = ResponseAircraft {
            aircraft_type: "Citation Sovereign".to_owned(),
            icao_type: "C680".to_owned(),
            manufacturer: "Cessna".to_owned(),
            mode_s: "A44F3B".to_owned(),
            registration: registration.to_owned(),
            registered_owner: "NetJets".to_owned(),
            registered_owner_operator_flag_code: "EJA".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: Some(format!(
                "{}{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
            url_photo_thumbnail: Some(format!(
                "{}thumbnails/{}",
                application_state.url_prefix, "001/572/001572354.jpg"
            )),
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        match &response.1.response.aircraft {
            Some(d) => assert_eq!(d, &aircraft),
            None => unreachable!(),
        }
    }

    #[tokio::test]
    async fn http_api_get_icao_callsign_mode_s_and_flightroute_ok_no_photo() {
        let callsign = "ACA959".to_owned();
        let mode_s = "A44917".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());

        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some("AC959".to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: "737MAX 9".to_owned(),
            icao_type: "B39M".to_owned(),
            manufacturer: "Boeing".to_owned(),
            mode_s: mode_s.clone(),
            registration: "N37522".to_owned(),
            registered_owner: "United Airlines".to_owned(),
            registered_owner_operator_flag_code: "UAL".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        match &response.1.response.aircraft {
            Some(d) => assert_eq!(d, &aircraft),
            None => unreachable!(),
        }
    }

    #[tokio::test]
    async fn http_api_get_iata_callsign_mode_s_and_flightroute_ok_no_photo() {
        let callsign = "AC959".to_owned();
        let mode_s = "A44917".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());

        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some("ACA959".to_owned()),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: "737MAX 9".to_owned(),
            icao_type: "B39M".to_owned(),
            manufacturer: "Boeing".to_owned(),
            mode_s: mode_s.clone(),
            registration: "N37522".to_owned(),
            registered_owner: "United Airlines".to_owned(),
            registered_owner_operator_flag_code: "UAL".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        match &response.1.response.aircraft {
            Some(d) => assert_eq!(d, &aircraft),
            None => unreachable!(),
        }
    }

    #[tokio::test]
    async fn http_api_get_icao_callsign_registration_and_flightroute_ok_no_photo() {
        let callsign = "ACA959".to_owned();
        let registration = "N37522".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());

        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some("AC959".to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: "737MAX 9".to_owned(),
            icao_type: "B39M".to_owned(),
            manufacturer: "Boeing".to_owned(),
            mode_s: "A44917".to_owned(),
            registration: registration.clone(),
            registered_owner: "United Airlines".to_owned(),
            registered_owner_operator_flag_code: "UAL".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        match &response.1.response.aircraft {
            Some(d) => assert_eq!(d, &aircraft),
            None => unreachable!(),
        }
    }

    #[tokio::test]
    async fn http_api_get_iata_callsign_registration_and_flightroute_ok_no_photo() {
        let callsign = "AC959".to_owned();
        let registration = "N37522".to_owned();
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());

        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some("ACA959".to_owned()),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: "Air Canada".to_owned(),
                icao: "ACA".to_owned(),
                iata: Some("AC".to_owned()),
                country: "Canada".to_owned(),
                country_iso: "CA".to_owned(),
                callsign: Some("AIR CANADA".to_owned()),
            }),
            origin: Airport {
                country_iso_name: "CA".to_owned(),
                country_name: "Canada".to_owned(),
                elevation: 118,
                iata_code: "YUL".to_owned(),
                icao_code: "CYUL".to_owned(),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: "Montréal".to_owned(),
                name: "Montreal / Pierre Elliott Trudeau International Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "CR".to_owned(),
                country_name: "Costa Rica".to_owned(),
                elevation: 3021,
                iata_code: "SJO".to_owned(),
                icao_code: "MROC".to_owned(),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: "San José (Alajuela)".to_owned(),
                name: "Juan Santamaría International Airport".to_owned(),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: "737MAX 9".to_owned(),
            icao_type: "B39M".to_owned(),
            manufacturer: "Boeing".to_owned(),
            mode_s: "A44917".to_owned(),
            registration: registration.clone(),
            registered_owner: "United Airlines".to_owned(),
            registered_owner_operator_flag_code: "UAL".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        match &response.1.response.aircraft {
            Some(d) => assert_eq!(d, &aircraft),
            None => unreachable!(),
        }
    }

    /// Airline Route
    /// `/airline/[short_code]`

    #[tokio::test]
    /// Make sure that an unknown iata Airline is inserted correctly into redis cache as NULL and has ttl of 604800
    /// and another request extends the tll to 604800 again
    async fn http_api_get_iata_airline_none_cached() {
        let callsign = "R56";
        let application_state = get_application_state().await;
        let path = AirlineCode::Iata(callsign.to_owned());

        let response = airline_get(application_state.clone(), path.clone())
            .await
            .unwrap_err();
        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Airline),
            _ => unreachable!(),
        };
        let key = RedisKey::Airline(&path);

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);

        sleep!();

        // Check second request is also in redis, and cache ttl gets reset
        let response = airline_get(application_state.clone(), path.clone())
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Airline),
            _ => unreachable!(),
        };

        let key = RedisKey::Airline(&path);
        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    /// Make sure that an unknown icao Airline is inserted correctly into redis cache as NULL and has ttl of 604800
    /// and another request extends the tll to 604800 again
    async fn http_api_get_icao_airline_none_cached() {
        let callsign = "RTT";
        let application_state = get_application_state().await;
        let path = AirlineCode::Icao(callsign.to_owned());

        let response = airline_get(application_state.clone(), path.clone())
            .await
            .unwrap_err();
        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Airline),
            _ => unreachable!(),
        };
        let key = RedisKey::Airline(&path);

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);

        sleep!();

        // Check second request is also in redis, and cache ttl gets reset
        let response = airline_get(application_state.clone(), path.clone())
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Airline),
            _ => unreachable!(),
        };

        let key = RedisKey::Airline(&path);
        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    /// Make sure that a known icao Airline is returned, and inserted correctly into redis cache
    /// and another request extends the tll to 604800 again
    async fn http_api_get_icao_airline_ok_and_cached() {
        let callsign = "RCK";
        let application_state = get_application_state().await;
        let path = AirlineCode::Icao(callsign.to_owned());

        let response = airline_get(application_state.clone(), path.clone()).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let expected = [ResponseAirline {
            name: "Faroejet".to_owned(),
            icao: "RCK".to_owned(),
            iata: Some("F6".to_owned()),
            country: "Faroe Islands".to_owned(),
            country_iso: "FO".to_owned(),
            callsign: Some("ROCKROSE".to_owned()),
        }];
        assert_eq!(response.1.response, expected);

        let key = RedisKey::Airline(&path);

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: Vec<ModelAirline> = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].airline_name, "Faroejet".to_owned());
        assert_eq!(result[0].country_name, "Faroe Islands".to_owned());
        assert_eq!(result[0].country_iso_name, "FO".to_owned());
        assert_eq!(result[0].iata_prefix, Some("F6".to_owned()));
        assert_eq!(result[0].icao_prefix, "RCK".to_owned());
        assert_eq!(result[0].airline_callsign, Some("ROCKROSE".to_owned()));

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);

        sleep!();
        let response = airline_get(application_state.clone(), path.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    /// Make sure that a known iata code returns multiple Airlines, and inserted correctly into redis cache
    /// and another request extends the tll to 604800 again
    async fn http_api_get_iata_airline_ok_and_cached() {
        let callsign = "JR";
        let application_state = get_application_state().await;
        let path = AirlineCode::Iata(callsign.to_owned());

        let response = airline_get(application_state.clone(), path.clone()).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let expected = [
            ResponseAirline {
                name: "Aero California".to_owned(),
                icao: "SER".to_owned(),
                iata: Some("JR".to_owned()),
                country: "Mexico".to_owned(),
                country_iso: "MX".to_owned(),
                callsign: Some("AEROCALIFORNIA".to_owned()),
            },
            ResponseAirline {
                name: "Joy Air".to_owned(),
                icao: "JOY".to_owned(),
                iata: Some("JR".to_owned()),
                country: "China".to_owned(),
                country_iso: "CN".to_owned(),
                callsign: Some("JOY AIR".to_owned()),
            },
        ];
        assert_eq!(response.1.response, expected);

        let key = RedisKey::Airline(&path);

        let result: Result<String, RedisError> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: Vec<ModelAirline> = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].airline_name, "Aero California".to_owned());
        assert_eq!(result[0].country_name, "Mexico".to_owned());
        assert_eq!(result[0].country_iso_name, "MX".to_owned());
        assert_eq!(result[0].iata_prefix, Some("JR".to_owned()));
        assert_eq!(result[0].icao_prefix, "SER".to_owned());
        assert_eq!(
            result[0].airline_callsign,
            Some("AEROCALIFORNIA".to_owned())
        );

        assert_eq!(result[1].airline_name, "Joy Air".to_owned());
        assert_eq!(result[1].country_name, "China".to_owned());
        assert_eq!(result[1].country_iso_name, "CN".to_owned());
        assert_eq!(result[1].iata_prefix, Some("JR".to_owned()));
        assert_eq!(result[1].icao_prefix, "JOY".to_owned());
        assert_eq!(result[1].airline_callsign, Some("JOY AIR".to_owned()));

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);

        sleep!();
        let response = airline_get(application_state.clone(), path.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }
}
