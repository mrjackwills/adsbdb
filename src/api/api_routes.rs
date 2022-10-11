use std::collections::HashMap;

use axum::Extension;
use tracing::info;

use super::input::{Callsign, ModeS, NNumber};
use super::response::{
    AircraftAndRoute, AsJsonRes, Online, ResponseAircraft, ResponseFlightRoute, ResponseJson,
};
use super::{AppError, ApplicationState};
use crate::db_postgres::{ModelAircraft, ModelFlightroute};
// use crate::db_redis::{get_cache, insert_cache, RedisKey, Cache};
use crate::db_redis::{get_cache, insert_cache, Cache, RedisKey};
use crate::n_number::{mode_s_to_n_number, n_number_to_mode_s};

/// Get flightroute, refactored so can use in either `get_mode_s` (with a callsign query param), or `get_callsign`.
/// Check redis cache for aircraft (or 'none'), or hit postgres
async fn find_flightroute(
    path: &Callsign,
    state: ApplicationState,
) -> Result<Option<ModelFlightroute>, AppError> {
    let redis_key = RedisKey::Callsign(path);
    if let Some(flightroute) = get_cache::<ModelFlightroute>(&state.redis, &redis_key).await? {
		match flightroute {
			Cache::Data(t) => Ok(Some(t)),
			Cache::Empty => Err(AppError::UnknownInDb("callsign"))
		}
    } else {
        let mut flightroute = ModelFlightroute::get(&state.postgres, path).await?;
        if flightroute.is_none() {
            flightroute = state
                .scraper
                .scrape_flightroute(&state.postgres, path)
                .await?;
        }
        insert_cache(&state.redis, &flightroute, &redis_key).await?;
        Ok(flightroute)
    }
}

/// Check redis cache for aircraft (or 'none'), or hit postgres
async fn find_aircraft(
    mode_s: &ModeS,
    state: ApplicationState,
) -> Result<Option<ModelAircraft>, AppError> {
    let redis_key = RedisKey::ModeS(mode_s);
    if let Some(aircraft) = get_cache::<ModelAircraft>(&state.redis, &redis_key).await? {
		match aircraft {
			Cache::Data(t) => Ok(Some(t)),
			Cache::Empty => Err(AppError::UnknownInDb("aircraft"))
		}
    } else {
        let mut aircraft = ModelAircraft::get(&state.postgres, mode_s, &state.url_prefix).await?;
        if let Some(craft) = aircraft.as_ref() {
            if craft.url_photo.is_none() {
                state.scraper.scrape_photo(&state.postgres, craft).await?;
                aircraft = ModelAircraft::get(&state.postgres, mode_s, &state.url_prefix).await?;
            }
        }
        insert_cache(&state.redis, &aircraft, &redis_key).await?;
        Ok(aircraft)
    }
}

/// Route to convert N-Number to Mode_S
#[allow(clippy::unused_async)]
pub async fn n_number_get(
    n_number: NNumber,
) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
    Ok((
        axum::http::StatusCode::OK,
        ResponseJson::new(n_number_to_mode_s(&n_number).map_or("".to_owned(), |f| f.to_string())),
    ))
}

/// Route to convert Mode_S to N-Number
#[allow(clippy::unused_async)]
pub async fn mode_s_get(
    mode_s: ModeS,
) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
    Ok((
        axum::http::StatusCode::OK,
        ResponseJson::new(mode_s_to_n_number(&mode_s).map_or("".to_owned(), |f| f.to_string())),
    ))
}

/// Return an aircraft detail from a modes input
/// optional query param of callsign, so can get both aircraft and flightroute in a single request
pub async fn aircraft_get(
    Extension(state): Extension<ApplicationState>,
    path: ModeS,
    axum::extract::Query(queries): axum::extract::Query<HashMap<String, String>>,
) -> Result<(axum::http::StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
    // Check if optional callsign query param
    if let Some(query_param) = queries.get("callsign") {
        let callsign = Callsign::try_from(query_param)?;
        let (aircraft, flightroute) = tokio::try_join!(
            find_aircraft(&path, state.clone()),
            find_flightroute(&callsign, state)
        )?;
        aircraft.map_or(Err(AppError::UnknownInDb("aircraft")), |a| {
            Ok((
                axum::http::StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: Some(ResponseAircraft::from(a)),
                    flightroute: ResponseFlightRoute::from_model(&flightroute),
                }),
            ))
        })
    } else {
        find_aircraft(&path, state).await?.map_or(
            Err(AppError::UnknownInDb("aircraft")),
            |aircraft| {
                Ok((
                    axum::http::StatusCode::OK,
                    ResponseJson::new(AircraftAndRoute {
                        aircraft: Some(ResponseAircraft::from(aircraft)),
                        flightroute: None,
                    }),
                ))
            },
        )
    }
}

/// Return a flightroute detail from a callsign input
pub async fn callsign_get(
    Extension(state): Extension<ApplicationState>,
    path: Callsign,
) -> Result<(axum::http::StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
    find_flightroute(&path, state)
        .await?
        .map_or(Err(AppError::UnknownInDb("callsign")), |a| {
            Ok((
                axum::http::StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: None,
                    flightroute: ResponseFlightRoute::from_model(&Some(a)),
                }),
            ))
        })
}

/// Return a simple online status response
#[allow(clippy::unused_async)]
pub async fn online_get(
    Extension(state): Extension<ApplicationState>,
) -> (axum::http::StatusCode, AsJsonRes<Online>) {
    (
        axum::http::StatusCode::OK,
        ResponseJson::new(Online {
            uptime: state.uptime.elapsed().as_secs(),
            api_version: env!("CARGO_PKG_VERSION").into(),
        }),
    )
}

/// return a unknown endpoint response
#[allow(clippy::unused_async)]
pub async fn fallback(uri: axum::http::Uri) -> (axum::http::StatusCode, AsJsonRes<String>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        ResponseJson::new(format!("unknown endpoint: {}", uri)),
    )
}

/// ApiRoutes tests
/// cargo watch -q -c -w src/ -x 'test http_api -- --test-threads=1 --nocapture'
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use super::*;

    use axum::http::Uri;
    use redis::{AsyncCommands, RedisError};
    use sqlx::PgPool;
    use tokio::sync::Mutex;

    use crate::api::response::Airport;
    use crate::db_postgres;
    use crate::db_redis as Redis;
    use crate::parse_env;

    const CALLSIGN: &str = "ANA460";

    async fn get_application_state() -> Extension<ApplicationState> {
        let app_env = parse_env::AppEnv::get_env();
        let postgres = db_postgres::db_pool(&app_env).await.unwrap();
        let mut redis = Redis::get_connection(&app_env).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<_, ()>(&mut redis)
            .await
            .unwrap();
        Extension(ApplicationState::new(
            postgres,
            Arc::new(Mutex::new(redis)),
            &app_env,
        ))
    }

    async fn sleep(ms: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }

    async fn remove_scraped_flightroute(db: &PgPool) {
        let query = "DELETE FROM flightroute WHERE flightroute_callsign_id = (SELECT flightroute_callsign_id FROM flightroute_callsign WHERE callsign = $1)";
        sqlx::query(query).bind(CALLSIGN).execute(db).await.unwrap();
        let query = "DELETE FROM flightroute_callsign WHERE callsign = $1";
        sqlx::query(query).bind(CALLSIGN).execute(db).await.unwrap();
        let app_env = parse_env::AppEnv::get_env();
        let mut redis = Redis::get_connection(&app_env).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<_, ()>(&mut redis)
            .await
            .unwrap();
    }

    #[tokio::test]
    // basically a 404 handler
    async fn http_api_fallback_route() {
        let uri = "/test/uri".parse::<Uri>().unwrap();
        let response = fallback(uri.clone()).await;
        assert_eq!(response.0, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(response.1.response, format!("unknown endpoint: {}", uri));
    }

    #[tokio::test]
    async fn http_api_online_route() {
        let application_state = get_application_state().await;

        sleep(1000).await;
        let response = online_get(application_state).await;

        assert_eq!(response.0, axum::http::StatusCode::OK);
        assert_eq!(env!("CARGO_PKG_VERSION"), response.1.response.api_version);
        assert!(response.1.response.uptime >= 1);
    }

    #[tokio::test]
    async fn http_api_n_number_route() {
        let n_number = NNumber::try_from("N123AB").unwrap();
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
        let path = ModeS::try_from(&mode_s).unwrap();
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
            n_number: "N377QS".to_owned(),
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
        let path = ModeS::try_from(mode_s).unwrap();
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
            n_number: "N37522".to_owned(),
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
        let tmp_mode_s = ModeS::try_from(mode_s).unwrap();
        let key = RedisKey::ModeS(&tmp_mode_s);
        let application_state = get_application_state().await;
        let path = ModeS::try_from(mode_s).unwrap();
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap();

        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .hget(key.to_string(), "data")
            .await;
        assert!(result.is_ok());

        let result: ResponseAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_mode_s_cached_no_photo() {
        let mode_s = "A44917".to_owned();
        let tmp_mode_s = ModeS::try_from(&mode_s).unwrap();
        let key = RedisKey::ModeS(&tmp_mode_s);
        let application_state = get_application_state().await;
        let path = ModeS::try_from(&mode_s).unwrap();
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap();

        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .hget(key.to_string(), "data")
            .await;
        assert!(result.is_ok());

        let result: ResponseAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state
            .redis
            .lock()
            .await
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
        let tmp_mode_s = ModeS::try_from(&mode_s).unwrap();
        let key = RedisKey::ModeS(&tmp_mode_s);
        let application_state = get_application_state().await;
        let path = ModeS::try_from(mode_s).unwrap();
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path.clone(), hm)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, "aircraft"),
            _ => unreachable!(),
        };

        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .hget(key.to_string(), "data")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);

        sleep(1000).await;

        // make sure a second requst to an unknown mode_s will extend cache ttl
        let hm = axum::extract::Query(HashMap::new());
        let response = aircraft_get(application_state.clone(), path, hm)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, "aircraft"),
            _ => unreachable!(),
        };

        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .hget(key.to_string(), "data")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_callsign_ok() {
        let callsign = "TOM35MR";
        let application_state = get_application_state().await;
        let path = Callsign::try_from(callsign).unwrap();
        let response = callsign_get(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
            origin: Airport {
                country_iso_name: "ES".to_owned(),
                country_name: "Spain".to_owned(),
                elevation: 27,
                iata_code: "PMI".to_owned(),
                icao_code: "LEPA".to_owned(),
                latitude: 39.551_701,
                longitude: 2.73881,
                municipality: "Palma De Mallorca".to_owned(),
                name: "Palma de Mallorca Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "GB".to_owned(),
                country_name: "United Kingdom".to_owned(),
                elevation: 622,
                iata_code: "BRS".to_owned(),
                icao_code: "EGGD".to_owned(),
                latitude: 51.382_702,
                longitude: -2.71909,
                municipality: "Bristol".to_owned(),
                name: "Bristol Airport".to_owned(),
            },
        };

        match &response.1.response.flightroute {
            Some(d) => assert_eq!(d, &flightroute),
            None => unreachable!(),
        }

        assert!(response.1.response.aircraft.is_none());
    }

    #[tokio::test]
    async fn http_api_get_callsign_with_midpoint_ok() {
        let callsign = "QFA031";
        let application_state = get_application_state().await;
        let path = Callsign::try_from(callsign).unwrap();
        let response = callsign_get(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let flightroute = ResponseFlightRoute {
            callsign: callsign.to_owned(),
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
    // Make sure flightroute is inserted correctly into redis cache and has ttl of 604800
    async fn http_api_get_callsign_cached() {
        let callsign = "TOM35MR";
        let application_state = get_application_state().await;
        let path = Callsign::try_from(callsign).unwrap();
        callsign_get(application_state.clone(), path).await.unwrap();

        let tmp_callsign = Callsign::try_from(callsign).unwrap();
        let key = RedisKey::Callsign(&tmp_callsign);
        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .hget(key.to_string(), "data")
            .await;
        assert!(result.is_ok());
        let result: ModelFlightroute = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(result.callsign, callsign);
        assert_eq!(result.origin_airport_country_iso_name, "ES");
        assert_eq!(result.origin_airport_country_name, "Spain");
        assert_eq!(result.origin_airport_elevation, 27);
        assert_eq!(result.origin_airport_iata_code, "PMI");
        assert_eq!(result.origin_airport_icao_code, "LEPA");
        assert_eq!(result.origin_airport_latitude, 39.551_701);
        assert_eq!(result.origin_airport_longitude, 2.73881);
        assert_eq!(result.origin_airport_municipality, "Palma De Mallorca");
        assert_eq!(result.origin_airport_name, "Palma de Mallorca Airport");
        assert!(result.midpoint_airport_country_iso_name.is_none());
        assert!(result.midpoint_airport_country_name.is_none());
        assert!(result.midpoint_airport_elevation.is_none());
        assert!(result.midpoint_airport_iata_code.is_none());
        assert!(result.midpoint_airport_icao_code.is_none());
        assert!(result.midpoint_airport_latitude.is_none());
        assert!(result.midpoint_airport_longitude.is_none());
        assert!(result.midpoint_airport_municipality.is_none());
        assert!(result.midpoint_airport_name.is_none());
        assert_eq!(result.destination_airport_country_iso_name, "GB");
        assert_eq!(result.destination_airport_country_name, "United Kingdom");
        assert_eq!(result.destination_airport_elevation, 622);
        assert_eq!(result.destination_airport_iata_code, "BRS");
        assert_eq!(result.destination_airport_icao_code, "EGGD");
        assert_eq!(result.destination_airport_latitude, 51.382_702);
        assert_eq!(result.destination_airport_longitude, -2.719_09);
        assert_eq!(result.destination_airport_municipality, "Bristol");
        assert_eq!(result.destination_airport_name, "Bristol Airport");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    // Make sure flightroute is inserted correctly into redis cache and has ttl of 604800
    async fn http_api_get_midpoint_callsign_cached() {
        let callsign = "QFA031";
        let application_state = get_application_state().await;
        let path = Callsign::try_from(callsign).unwrap();
        callsign_get(application_state.clone(), path).await.unwrap();
        let tmp_callsign = Callsign::try_from(callsign).unwrap();
        let key = RedisKey::Callsign(&tmp_callsign);
        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .hget(key.to_string(), "data")
            .await;
        assert!(result.is_ok());
        let result: ModelFlightroute = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(result.callsign, callsign);

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
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    // Insert a new flightroute using the scraper
    async fn http_api_get_callsign_scraper() {
        let application_state = get_application_state().await;
        let path = Callsign::try_from(CALLSIGN).unwrap();

        let response = callsign_get(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let expected = ResponseFlightRoute {
            callsign: "ANA460".to_owned(),

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

        match &response.1.response.flightroute {
            Some(x) => assert_eq!(x, &expected),
            None => unreachable!(),
        }
        remove_scraped_flightroute(&application_state.postgres).await;
    }

    #[tokio::test]
    // Make sure that an unknown flightroute is inserted correctly into redis cache as NULL and has ttl of 604800
    // and another request extends the tll to 604800 again
    async fn http_api_get_callsign_none_cached() {
        let callsign = "ABABAB";
        let application_state = get_application_state().await;
        let path = Callsign::try_from(callsign).unwrap();

        let response = callsign_get(application_state.clone(), path.clone())
            .await
            .unwrap_err();
        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, "callsign"),
            _ => unreachable!(),
        };
        let tmp_callsign = Callsign::try_from(callsign).unwrap();
        let key = RedisKey::Callsign(&tmp_callsign);
        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .hget(key.to_string(), "data")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);

        sleep(1000).await;

        // Check second request is also in redis, and cache ttl gets reset
        let response = callsign_get(application_state.clone(), path)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, "callsign"),
            _ => unreachable!(),
        };

        let key = RedisKey::Callsign(&tmp_callsign);
        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .hget(key.to_string(), "data")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604_800);
    }

    #[tokio::test]
    async fn http_api_get_callsign_and_flightroute_ok_with_photo() {
        let callsign = "TOM35MR".to_owned();
        let mode_s = "A44F3B".to_owned();
        let application_state = get_application_state().await;
        let path = ModeS::try_from(&mode_s).unwrap();
        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.clone(),
            origin: Airport {
                country_iso_name: "ES".to_owned(),
                country_name: "Spain".to_owned(),
                elevation: 27,
                iata_code: "PMI".to_owned(),
                icao_code: "LEPA".to_owned(),
                latitude: 39.551_701,
                longitude: 2.73881,
                municipality: "Palma De Mallorca".to_owned(),
                name: "Palma de Mallorca Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "GB".to_owned(),
                country_name: "United Kingdom".to_owned(),
                elevation: 622,
                iata_code: "BRS".to_owned(),
                icao_code: "EGGD".to_owned(),
                latitude: 51.382_702,
                longitude: -2.71909,
                municipality: "Bristol".to_owned(),
                name: "Bristol Airport".to_owned(),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: "Citation Sovereign".to_owned(),
            icao_type: "C680".to_owned(),
            manufacturer: "Cessna".to_owned(),
            mode_s: mode_s.clone(),
            n_number: "N377QS".to_owned(),
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
    async fn http_api_get_callsign_and_flightroute_ok_no_photo() {
        let callsign = "TOM35MR".to_owned();
        let mode_s = "A44917".to_owned();
        let application_state = get_application_state().await;
        let path = ModeS::try_from(&mode_s).unwrap();

        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = aircraft_get(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ResponseFlightRoute {
            callsign: callsign.clone(),
            origin: Airport {
                country_iso_name: "ES".to_owned(),
                country_name: "Spain".to_owned(),
                elevation: 27,
                iata_code: "PMI".to_owned(),
                icao_code: "LEPA".to_owned(),
                latitude: 39.551_701,
                longitude: 2.73881,
                municipality: "Palma De Mallorca".to_owned(),
                name: "Palma de Mallorca Airport".to_owned(),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: "GB".to_owned(),
                country_name: "United Kingdom".to_owned(),
                elevation: 622,
                iata_code: "BRS".to_owned(),
                icao_code: "EGGD".to_owned(),
                latitude: 51.382_702,
                longitude: -2.71909,
                municipality: "Bristol".to_owned(),
                name: "Bristol Airport".to_owned(),
            },
        };

        let aircraft = ResponseAircraft {
            aircraft_type: "737MAX 9".to_owned(),
            icao_type: "B39M".to_owned(),
            manufacturer: "Boeing".to_owned(),
            mode_s: mode_s.clone(),
            n_number: "N37522".to_owned(),
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
}
