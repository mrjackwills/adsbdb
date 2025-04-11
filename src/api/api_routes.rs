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
use super::{AppError, ApplicationState, app_error::UnknownAC};
use crate::{
    S,
    db_postgres::{ModelAircraft, ModelAirline, ModelFlightroute},
    db_redis::{RedisKey, get_cache, insert_cache},
    n_number::{mode_s_to_n_number, n_number_to_mode_s},
};

/// Get flightroute, refactored so can use in either `get_mode_s` (with a callsign query param), or `get_callsign`.
/// Check redis cache for Option\<ModelFlightroute>, else query postgres
async fn find_flightroute(
    state: &ApplicationState,
    callsign: &Callsign,
) -> Result<Option<ModelFlightroute>, AppError> {
    let redis_key = RedisKey::Callsign(callsign);
    if let Some(flightroute) = get_cache::<ModelFlightroute>(&state.redis, &redis_key).await? {
        flightroute.map_or(Err(AppError::UnknownInDb(UnknownAC::Callsign)), |route| {
            Ok(Some(route))
        })
    } else {
        let mut flightroute = ModelFlightroute::get(&state.postgres, callsign).await;
        if flightroute.is_none()  {
            let (one_tx, one_rx) = tokio::sync::oneshot::channel();

            if state
                .scraper_tx
                .send(crate::scraper::ScraperMsg::CallSign((
                    one_tx,
                    callsign.clone(),
                )))
                .await
                .is_ok()
            {
                flightroute = one_rx.await.unwrap_or(None);
            }
        }
        insert_cache(&state.redis, flightroute.as_ref(), &redis_key).await?;
        Ok(flightroute)
    }
}

/// Check redis cache for Option\<ModelAircraft>, else query postgres
async fn find_aircraft(
    state: &ApplicationState,
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
                let (one_tx, one_rx) = tokio::sync::oneshot::channel();
                if state
                    .scraper_tx
                    .send(crate::scraper::ScraperMsg::Photo((
                        one_tx,
                        craft.mode_s.clone(),
                    )))
                    .await
                    .is_ok()
                {
                    one_rx.await.ok();
                }
                aircraft =
                    ModelAircraft::get(&state.postgres, aircraft_search, &state.url_prefix).await?;
            }
        }
        insert_cache(&state.redis, aircraft.as_ref(), &redis_key).await?;
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
        insert_cache(&state.redis, airline.as_ref(), &redis_key).await?;
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
            find_aircraft(&state, &aircraft_search),
            find_flightroute(&state, &callsign),
        )?;
        aircraft.map_or(
            Err(AppError::UnknownInDb(UnknownAC::Aircraft)),
            |aircraft| {
                Ok((
                    StatusCode::OK,
                    ResponseJson::new(AircraftAndRoute {
                        aircraft: Some(ResponseAircraft::from(aircraft)),
                        flightroute: ResponseFlightRoute::from_model(flightroute.as_ref()),
                    }),
                ))
            },
        )
    } else {
        find_aircraft(&state, &aircraft_search).await?.map_or(
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
    find_flightroute(&state, &callsign).await?.map_or(
        Err(AppError::UnknownInDb(UnknownAC::Callsign)),
        |model| {
            Ok((
                StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: None,
                    flightroute: ResponseFlightRoute::from_model(Some(&model)),
                }),
            ))
        },
    )
}

/// Route to convert N-Number to Mode_S
pub async fn n_number_get(
    n_number: NNumber,
) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
    Ok((
        StatusCode::OK,
        ResponseJson::new(n_number_to_mode_s(&n_number).map_or(S!(), |f| f.to_string())),
    ))
}

/// Route to convert Mode_S to N-Number
pub async fn mode_s_get(
    mode_s: ModeS,
) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
    Ok((
        StatusCode::OK,
        ResponseJson::new(mode_s_to_n_number(&mode_s).map_or(S!(), |f| f.to_string())),
    ))
}

/// Return a simple online status response
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

    use super::*;

    use axum::http::Uri;
    use fred::interfaces::ClientLike;
    use fred::interfaces::HashesInterface;
    use fred::interfaces::KeysInterface;

    use crate::S;
    use crate::api::Registration;
    use crate::api::input::Validate;
    use crate::api::response::Airline;
    use crate::api::response::Airport;
    use crate::db_postgres;
    use crate::db_redis;
    use crate::parse_env;
    use crate::scraper;
    use crate::scraper::tests::{TEST_CALLSIGN, remove_scraped_data};
    use crate::sleep;

    async fn get_application_state() -> State<ApplicationState> {
        let app_env = parse_env::AppEnv::get_env();
        let postgres = db_postgres::get_pool(&app_env).await.unwrap();
        let redis = db_redis::get_pool(&app_env).await.unwrap();
        redis.flushall::<()>(true).await.unwrap();

        let scraper_tx = scraper::Scraper::start(&app_env, &postgres);

        State(ApplicationState::new(&app_env, postgres, redis, scraper_tx))
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
        let mode_s = S!("A44F3B");
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ResponseAircraft {
            aircraft_type: S!("Citation Sovereign"),
            icao_type: S!("C680"),
            manufacturer: S!("Cessna"),
            mode_s,
            registration: S!("N377QS"),
            registered_owner: S!("NetJets"),
            registered_owner_operator_flag_code: Some(S!("EJA")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
        let registration = S!("N377QS");
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ResponseAircraft {
            aircraft_type: S!("Citation Sovereign"),
            icao_type: S!("C680"),
            manufacturer: S!("Cessna"),
            mode_s: S!("A44F3B"),
            registration,
            registered_owner: S!("NetJets"),
            registered_owner_operator_flag_code: Some(S!("EJA")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
            aircraft_type: S!("737MAX 9"),
            icao_type: S!("B39M"),
            manufacturer: S!("Boeing"),
            mode_s: mode_s.to_owned(),
            registration: S!("N37522"),
            registered_owner: S!("United Airlines"),
            registered_owner_operator_flag_code: Some(S!("UAL")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
            aircraft_type: S!("737MAX 9"),
            icao_type: S!("B39M"),
            manufacturer: S!("Boeing"),
            mode_s: S!("A44917"),
            registration: registration.to_owned(),
            registered_owner: S!("United Airlines"),
            registered_owner_operator_flag_code: Some(S!("UAL")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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

        let result: Result<String, fred::error::Error> =
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

        let result: Result<String, fred::error::Error> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: ResponseAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_mode_s_cached_no_photo() {
        let mode_s = S!("A44917");
        let tmp_mode_s = ModeS::validate(&mode_s).unwrap();
        let key = RedisKey::ModeS(&tmp_mode_s);
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap();

        let result: Result<String, fred::error::Error> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: ResponseAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_registration_cached_no_photo() {
        let registration = S!("N37522");
        let tmp_registration = Registration::validate(&registration).unwrap();
        let key = RedisKey::Registration(&tmp_registration);
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap();
        let result: Result<String, fred::error::Error> =
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
        let mode_s = S!("ABABAB");
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
        let result: Result<String, fred::error::Error> =
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

        let result: Result<String, fred::error::Error> =
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
        let registration = S!("AB-ABAB");
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
        let result: Result<String, fred::error::Error> =
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

        let result: Result<String, fred::error::Error> =
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
        // Refactor me, put in CONST

        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some(S!("AC959")),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
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
        let response: (StatusCode, axum::Json<ResponseJson<AircraftAndRoute>>) = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(S!("ACA959")),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
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
            callsign_iata: Some(S!("QF31")),
            callsign_icao: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: S!("Qantas"),
                icao: S!("QFA"),
                iata: Some(S!("QF")),
                callsign: Some(S!("QANTAS")),
                country: S!("Australia"),
                country_iso: S!("AU"),
            }),
            origin: Airport {
                country_iso_name: S!("AU"),
                country_name: S!("Australia"),
                elevation: 21,
                iata_code: S!("SYD"),
                icao_code: S!("YSSY"),
                latitude: -33.946_098_327_636_72,
                longitude: 151.177_001_953_125,
                municipality: S!("Sydney"),
                name: S!("Sydney Kingsford Smith International Airport"),
            },
            midpoint: Some(Airport {
                country_iso_name: S!("SG"),
                country_name: S!("Singapore"),
                elevation: 22,
                iata_code: S!("SIN"),
                icao_code: S!("WSSS"),
                latitude: 1.35019,
                longitude: 103.994_003,
                municipality: S!("Singapore"),
                name: S!("Singapore Changi Airport"),
            }),
            destination: Airport {
                country_iso_name: S!("GB"),
                country_name: S!("United Kingdom"),
                elevation: 83,
                iata_code: S!("LHR"),
                icao_code: S!("EGLL"),
                latitude: 51.4706,
                longitude: -0.461_941,
                municipality: S!("London"),
                name: S!("London Heathrow Airport"),
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
            callsign_icao: Some(S!("QFA31")),
            airline: Some(Airline {
                name: S!("Qantas"),
                icao: S!("QFA"),
                iata: Some(S!("QF")),
                callsign: Some(S!("QANTAS")),
                country: S!("Australia"),
                country_iso: S!("AU"),
            }),
            origin: Airport {
                country_iso_name: S!("AU"),
                country_name: S!("Australia"),
                elevation: 21,
                iata_code: S!("SYD"),
                icao_code: S!("YSSY"),
                latitude: -33.946_098_327_636_72,
                longitude: 151.177_001_953_125,
                municipality: S!("Sydney"),
                name: S!("Sydney Kingsford Smith International Airport"),
            },
            midpoint: Some(Airport {
                country_iso_name: S!("SG"),
                country_name: S!("Singapore"),
                elevation: 22,
                iata_code: S!("SIN"),
                icao_code: S!("WSSS"),
                latitude: 1.35019,
                longitude: 103.994_003,
                municipality: S!("Singapore"),
                name: S!("Singapore Changi Airport"),
            }),
            destination: Airport {
                country_iso_name: S!("GB"),
                country_name: S!("United Kingdom"),
                elevation: 83,
                iata_code: S!("LHR"),
                icao_code: S!("EGLL"),
                latitude: 51.4706,
                longitude: -0.461_941,
                municipality: S!("London"),
                name: S!("London Heathrow Airport"),
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

        let result: Result<ModelFlightroute, fred::error::Error> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        let result = result.unwrap();

        assert_eq!(result.airline_callsign, Some(S!("AIR CANADA")));
        assert_eq!(result.airline_country_iso_name, Some(S!("CA")));
        assert_eq!(result.airline_country_name, Some(S!("Canada")));
        assert_eq!(result.airline_iata, Some(S!("AC")));
        assert_eq!(result.airline_icao, Some(S!("ACA")));
        assert_eq!(result.airline_name, Some(S!("Air Canada")));

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

        let result: Result<String, fred::error::Error> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        let result: ModelFlightroute = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(result.callsign, callsign);

        assert_eq!(result.airline_callsign, Some(S!("QANTAS")));
        assert_eq!(result.airline_country_iso_name, Some(S!("AU")));
        assert_eq!(result.airline_country_name, Some(S!("Australia")));
        assert_eq!(result.airline_iata, Some(S!("QF")));
        assert_eq!(result.airline_icao, Some(S!("QFA")));
        assert_eq!(result.airline_name, Some(S!("Qantas")));

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

        assert_eq!(result.midpoint_airport_country_iso_name, Some(S!("SG")));
        assert_eq!(result.midpoint_airport_country_name, Some(S!("Singapore")));
        assert_eq!(result.midpoint_airport_elevation, Some(22));
        assert_eq!(result.midpoint_airport_iata_code, Some(S!("SIN")));
        assert_eq!(result.midpoint_airport_icao_code, Some(S!("WSSS")));
        assert_eq!(result.midpoint_airport_latitude, Some(1.35019));
        assert_eq!(result.midpoint_airport_longitude, Some(103.994_003));
        assert_eq!(result.midpoint_airport_municipality, Some(S!("Singapore")));
        assert_eq!(
            result.midpoint_airport_name,
            Some(S!("Singapore Changi Airport"))
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
        let path = Callsign::validate(TEST_CALLSIGN).unwrap();
        let response = callsign_get(application_state.clone(), path).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let expected = ResponseFlightRoute {
            callsign: S!("ANA460"),
            callsign_icao: Some(S!("ANA460")),
            callsign_iata: Some(S!("NH460")),
            airline: Some(Airline {
                name: S!("All Nippon Airways"),
                icao: S!("ANA"),
                iata: Some(S!("NH")),
                country: S!("Japan"),
                country_iso: S!("JP"),
                callsign: Some(S!("ALL NIPPON")),
            }),
            origin: Airport {
                country_iso_name: S!("JP"),
                country_name: S!("Japan"),
                elevation: 12,
                iata_code: S!("OKA"),
                icao_code: S!("ROAH"),
                latitude: 26.195_801,
                longitude: 127.646_004,
                municipality: S!("Naha"),
                name: S!("Naha Airport / JASDF Naha Air Base"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("JP"),
                country_name: S!("Japan"),
                elevation: 35,
                iata_code: S!("HND"),
                icao_code: S!("RJTT"),
                latitude: 35.552_299,
                longitude: 139.779_999,
                municipality: S!("Tokyo"),
                name: S!("Tokyo Haneda International Airport"),
            },
        };

        assert!(response.1.response.flightroute.is_some());
        let result = response.1.response.flightroute.clone().unwrap();
        assert_eq!(result, expected);
        remove_scraped_data(&application_state.postgres).await;
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

        let result: Result<String, fred::error::Error> =
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
        let result: Result<String, fred::error::Error> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state.redis.ttl(key.to_string()).await.unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_icao_callsign_and_flightroute_mode_s_ok_with_photo() {
        let callsign = S!("ACA959");
        let mode_s = S!("A44F3B");
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let mut hm = HashMap::new();
        hm.insert(S!("callsign"), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some(S!("AC959")),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: S!("Citation Sovereign"),
            icao_type: S!("C680"),
            manufacturer: S!("Cessna"),
            mode_s: mode_s.clone(),
            registration: S!("N377QS"),
            registered_owner: S!("NetJets"),
            registered_owner_operator_flag_code: Some(S!("EJA")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
        let callsign = S!("AC959");
        let mode_s = S!("A44F3B");
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());
        let mut hm = HashMap::new();
        hm.insert(S!("callsign"), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(S!("ACA959")),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: S!("Citation Sovereign"),
            icao_type: S!("C680"),
            manufacturer: S!("Cessna"),
            mode_s: mode_s.clone(),
            registration: S!("N377QS"),
            registered_owner: S!("NetJets"),
            registered_owner_operator_flag_code: Some(S!("EJA")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
        let callsign = S!("ACA959");
        let registration = S!("N377QS");
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let mut hm = HashMap::new();
        hm.insert(S!("callsign"), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some(S!("AC959")),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
            },
        };
        let aircraft = ResponseAircraft {
            aircraft_type: S!("Citation Sovereign"),
            icao_type: S!("C680"),
            manufacturer: S!("Cessna"),
            mode_s: S!("A44F3B"),
            registration: registration.to_owned(),
            registered_owner: S!("NetJets"),
            registered_owner_operator_flag_code: Some(S!("EJA")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
        let callsign = S!("AC959");
        let registration = S!("N377QS");
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());
        let mut hm = HashMap::new();
        hm.insert(S!("callsign"), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(S!("ACA959")),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
            },
        };
        let aircraft = ResponseAircraft {
            aircraft_type: S!("Citation Sovereign"),
            icao_type: S!("C680"),
            manufacturer: S!("Cessna"),
            mode_s: S!("A44F3B"),
            registration: registration.to_owned(),
            registered_owner: S!("NetJets"),
            registered_owner_operator_flag_code: Some(S!("EJA")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
        let callsign = S!("ACA959");
        let mode_s = S!("A44917");
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());

        let mut hm = HashMap::new();
        hm.insert(S!("callsign"), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some(S!("AC959")),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: S!("737MAX 9"),
            icao_type: S!("B39M"),
            manufacturer: S!("Boeing"),
            mode_s: mode_s.clone(),
            registration: S!("N37522"),
            registered_owner: S!("United Airlines"),
            registered_owner_operator_flag_code: Some(S!("UAL")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
        let callsign = S!("AC959");
        let mode_s = S!("A44917");
        let application_state = get_application_state().await;
        let path = AircraftSearch::ModeS(ModeS::validate(&mode_s).unwrap());

        let mut hm = HashMap::new();
        hm.insert(S!("callsign"), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(S!("ACA959")),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: S!("737MAX 9"),
            icao_type: S!("B39M"),
            manufacturer: S!("Boeing"),
            mode_s: mode_s.clone(),
            registration: S!("N37522"),
            registered_owner: S!("United Airlines"),
            registered_owner_operator_flag_code: Some(S!("UAL")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
        let callsign = S!("ACA959");
        let registration = S!("N37522");
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());

        let mut hm = HashMap::new();
        hm.insert(S!("callsign"), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(callsign.to_owned()),
            callsign_iata: Some(S!("AC959")),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: S!("737MAX 9"),
            icao_type: S!("B39M"),
            manufacturer: S!("Boeing"),
            mode_s: S!("A44917"),
            registration: registration.clone(),
            registered_owner: S!("United Airlines"),
            registered_owner_operator_flag_code: Some(S!("UAL")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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
        let callsign = S!("AC959");
        let registration = S!("N37522");
        let application_state = get_application_state().await;
        let path = AircraftSearch::Registration(Registration::validate(&registration).unwrap());

        let mut hm = HashMap::new();
        hm.insert(S!("callsign"), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            callsign_icao: Some(S!("ACA959")),
            callsign_iata: Some(callsign.to_owned()),
            airline: Some(Airline {
                name: S!("Air Canada"),
                icao: S!("ACA"),
                iata: Some(S!("AC")),
                country: S!("Canada"),
                country_iso: S!("CA"),
                callsign: Some(S!("AIR CANADA")),
            }),
            origin: Airport {
                country_iso_name: S!("CA"),
                country_name: S!("Canada"),
                elevation: 118,
                iata_code: S!("YUL"),
                icao_code: S!("CYUL"),
                latitude: 45.4706001282,
                longitude: -73.7407989502,
                municipality: S!("Montréal"),
                name: S!("Montreal / Pierre Elliott Trudeau International Airport"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("CR"),
                country_name: S!("Costa Rica"),
                elevation: 3021,
                iata_code: S!("SJO"),
                icao_code: S!("MROC"),
                latitude: 9.99386,
                longitude: -84.208801,
                municipality: S!("San José (Alajuela)"),
                name: S!("Juan Santamaría International Airport"),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: S!("737MAX 9"),
            icao_type: S!("B39M"),
            manufacturer: S!("Boeing"),
            mode_s: S!("A44917"),
            registration: registration.clone(),
            registered_owner: S!("United Airlines"),
            registered_owner_operator_flag_code: Some(S!("UAL")),
            registered_owner_country_name: S!("United States"),
            registered_owner_country_iso_name: S!("US"),
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

        let result: Result<String, fred::error::Error> =
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
        let result: Result<String, fred::error::Error> =
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

        let result: Result<String, fred::error::Error> =
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
        let result: Result<String, fred::error::Error> =
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
            name: S!("Faroejet"),
            icao: S!("RCK"),
            iata: Some(S!("F6")),
            country: S!("Faroe Islands"),
            country_iso: S!("FO"),
            callsign: Some(S!("ROCKROSE")),
        }];
        assert_eq!(response.1.response, expected);

        let key = RedisKey::Airline(&path);

        let result: Result<String, fred::error::Error> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: Vec<ModelAirline> = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].airline_name, S!("Faroejet"));
        assert_eq!(result[0].country_name, S!("Faroe Islands"));
        assert_eq!(result[0].country_iso_name, S!("FO"));
        assert_eq!(result[0].iata_prefix, Some(S!("F6")));
        assert_eq!(result[0].icao_prefix, S!("RCK"));
        assert_eq!(result[0].airline_callsign, Some(S!("ROCKROSE")));

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
                name: S!("Aero California"),
                icao: S!("SER"),
                iata: Some(S!("JR")),
                country: S!("Mexico"),
                country_iso: S!("MX"),
                callsign: Some(S!("AEROCALIFORNIA")),
            },
            ResponseAirline {
                name: S!("Joy Air"),
                icao: S!("JOY"),
                iata: Some(S!("JR")),
                country: S!("China"),
                country_iso: S!("CN"),
                callsign: Some(S!("JOY AIR")),
            },
        ];
        assert_eq!(response.1.response, expected);

        let key = RedisKey::Airline(&path);

        let result: Result<String, fred::error::Error> =
            application_state.redis.hget(key.to_string(), "data").await;
        assert!(result.is_ok());

        let result: Vec<ModelAirline> = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].airline_name, S!("Aero California"));
        assert_eq!(result[0].country_name, S!("Mexico"));
        assert_eq!(result[0].country_iso_name, S!("MX"));
        assert_eq!(result[0].iata_prefix, Some(S!("JR")));
        assert_eq!(result[0].icao_prefix, S!("SER"));
        assert_eq!(result[0].airline_callsign, Some(S!("AEROCALIFORNIA")));

        assert_eq!(result[1].airline_name, S!("Joy Air"));
        assert_eq!(result[1].country_name, S!("China"));
        assert_eq!(result[1].country_iso_name, S!("CN"));
        assert_eq!(result[1].iata_prefix, Some(S!("JR")));
        assert_eq!(result[1].icao_prefix, S!("JOY"));
        assert_eq!(result[1].airline_callsign, Some(S!("JOY AIR")));

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
