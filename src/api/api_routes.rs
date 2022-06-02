use std::collections::HashMap;

use axum::Extension;

use super::input::{Callsign, ModeS, NNumber};
use super::response::{AircraftAndRoute, AsJsonRes, Online, ResponseJson};
use super::{AppError, ApplicationState};
use crate::db_postgres::{Model, ModelAircraft, ModelFlightroute};
use crate::db_redis::{get_cache, insert_cache, RedisKey};
use crate::n_number::n_to_icao;

/// Get flightroute, refactored so can use in either get_mode_s (with a callsign query param), or get_callsign.
/// Check redis cache for aircraft (or 'none'), or hit postgres
async fn find_flightroute(
    path: &Callsign,
    state: ApplicationState,
) -> Result<Option<ModelFlightroute>, AppError> {
    let redis_key = RedisKey::Callsign(path.callsign.to_owned());
    let cache: Option<Option<ModelFlightroute>> = get_cache(&state.redis, &redis_key).await?;
    if let Some(flightroute) = cache {
        Ok(flightroute)
    } else {
        let mut flightroute = ModelFlightroute::get(&state.postgres, &path.callsign).await?;

        if flightroute.is_none() {
            flightroute = state
                .scraper
                .scrape_flightroute(&state.postgres, &path.callsign)
                .await?
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
    let redis_key = RedisKey::ModeS(mode_s.to_string());
    let cache: Option<Option<ModelAircraft>> = get_cache(&state.redis, &redis_key).await?;
    if let Some(aircraft) = cache {
        Ok(aircraft)
    } else {
        let mut aircraft = ModelAircraft::get(&state.postgres, mode_s, &state.url_prefix).await?;
        if let Some(craft) = aircraft.clone() {
            if craft.url_photo.is_none() {
                state.scraper.scrape_photo(&state.postgres, mode_s).await?;
                aircraft = ModelAircraft::get(&state.postgres, mode_s, &state.url_prefix).await?;
            }
        }
        insert_cache(&state.redis, &aircraft, &redis_key).await?;
        Ok(aircraft)
    }
}

pub async fn get_n_number(
    n_number: NNumber,
) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
    let icao = match n_to_icao(&n_number) {
        Ok(data) => data,
        Err(_) => String::from(""),
    };
    Ok((axum::http::StatusCode::OK, ResponseJson::new(icao)))
}

/// Return an aircraft detail from a modes input
/// optional query param of callsign, so can get both aircraft and flightroute in a single request
pub async fn get_aircraft(
    Extension(state): Extension<ApplicationState>,
    path: ModeS,
    axum::extract::Query(queries): axum::extract::Query<HashMap<String, String>>,
) -> Result<(axum::http::StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
    // Check if optional callsign query param
    if let Some(query_param) = queries.get("callsign") {
        let callsign = Callsign::new(query_param.to_owned())?;
        let (aircraft, flightroute) = tokio::join!(
            find_aircraft(&path, state.clone()),
            find_flightroute(&callsign, state)
        );
        if let Ok(Some(a)) = aircraft {
            let flightroute = flightroute?;
            Ok((
                axum::http::StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: Some(a),
                    flightroute,
                }),
            ))
        } else {
            Err(AppError::UnknownInDb("aircraft"))
        }
    } else {
        let aircraft = find_aircraft(&path, state).await?;
        if let Some(a) = aircraft {
            Ok((
                axum::http::StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: Some(a),
                    flightroute: None,
                }),
            ))
        } else {
            Err(AppError::UnknownInDb("aircraft"))
        }
    }
}

/// Return a flightroute detail from a callsign input
pub async fn get_callsign(
    Extension(state): Extension<ApplicationState>,
    path: Callsign,
) -> Result<(axum::http::StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
    let flightroute = find_flightroute(&path, state).await?;

    if let Some(a) = flightroute {
        Ok((
            axum::http::StatusCode::OK,
            ResponseJson::new(AircraftAndRoute {
                aircraft: None,
                flightroute: Some(a),
            }),
        ))
    } else {
        Err(AppError::UnknownInDb("callsign"))
    }
}

/// Return a simple online status response
pub async fn get_online(
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
pub async fn fallback(uri: axum::http::Uri) -> (axum::http::StatusCode, AsJsonRes<String>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        ResponseJson::new(format!("unknown endpoint: {}", uri)),
    )
}

/// ApiRoutes tests
/// cargo watch -q -c -w src/ -x 'test http_api -- --test-threads=1 --nocapture'
#[cfg(test)]
mod tests {
    use super::*;

    use axum::http::Uri;
    use redis::{AsyncCommands, RedisError};
    use sqlx::PgPool;

    use crate::db_postgres;
    use crate::db_redis as Redis;
    use crate::parse_env;

    const CALLSIGN: &str = "ANA460";

    // Also flushed redis of all keys!
    async fn get_application_state() -> Extension<ApplicationState> {
        let app_env = parse_env::AppEnv::get_env();
        let postgres = db_postgres::db_pool(&app_env).await.unwrap();
        let mut redis = Redis::get_connection(&app_env).await.unwrap();
        let _: () = redis::cmd("FLUSHDB").query_async(&mut redis).await.unwrap();
        Extension(ApplicationState::new(postgres, redis, &app_env))
    }

    async fn sleep(ms: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await
    }

    async fn remove_scraped_flightroute(db: &PgPool) {
        let query = "DELETE FROM flightroute WHERE flightroute_callsign_id = (SELECT flightroute_callsign_id FROM flightroute_callsign WHERE callsign = $1)";
        sqlx::query(query).bind(CALLSIGN).execute(db).await.unwrap();
        let query = "DELETE FROM flightroute_callsign WHERE callsign = $1";
        sqlx::query(query).bind(CALLSIGN).execute(db).await.unwrap();
        let app_env = parse_env::AppEnv::get_env();
        let mut redis = Redis::get_connection(&app_env).await.unwrap();
        let _: () = redis::cmd("FLUSHDB").query_async(&mut redis).await.unwrap();
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
        let response = get_online(application_state).await;

        assert_eq!(response.0, axum::http::StatusCode::OK);
        assert_eq!(env!("CARGO_PKG_VERSION"), response.1.response.api_version);
        assert!(response.1.response.uptime >= 1);
    }

	#[tokio::test]
    async fn http_api_n_number_route() {
		let n_number = NNumber::new("N123AB".to_owned()).unwrap();
        let response = get_n_number(n_number).await;

		assert!(response.is_ok());
		let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        assert_eq!(response.1.response, "A05ED9");
    }

    #[tokio::test]
    async fn http_api_get_mode_s_ok_with_photo() {
        let mode_s = "A44F3B".to_owned();
        let application_state = get_application_state().await;
        let path = ModeS::new(mode_s.clone()).unwrap();
        let hm = axum::extract::Query(HashMap::new());
        let response = get_aircraft(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ModelAircraft {
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
        let mode_s = "A44917".to_owned();
        let path = ModeS::new(mode_s.clone()).unwrap();
        let application_state = get_application_state().await;
        let hm = axum::extract::Query(HashMap::new());
        let response = get_aircraft(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ModelAircraft {
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
        let mode_s = "A44F3B".to_owned();
        let key = RedisKey::ModeS(mode_s.clone());
        let application_state = get_application_state().await;
        let path = ModeS::new(mode_s.clone()).unwrap();
        let hm = axum::extract::Query(HashMap::new());
        let response = get_aircraft(application_state.clone(), path, hm)
            .await
            .unwrap();

        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .get(key.to_string())
            .await;
        assert!(result.is_ok());

        let result: ModelAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604800);
    }

    #[tokio::test]
    async fn http_api_get_mode_s_cached_no_photo() {
        let mode_s = "A44917".to_owned();
        let key = RedisKey::ModeS(mode_s.clone());
        let application_state = get_application_state().await;
        let path = ModeS::new(mode_s.clone()).unwrap();
        let hm = axum::extract::Query(HashMap::new());
        let response = get_aircraft(application_state.clone(), path, hm)
            .await
            .unwrap();

        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .get(key.to_string())
            .await;
        assert!(result.is_ok());

        let result: ModelAircraft = serde_json::from_str(&result.unwrap()).unwrap();

        assert_eq!(&result, response.1.response.aircraft.as_ref().unwrap());

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604800);
    }

    #[tokio::test]
    // Make sure unknown aircraft gets placed into cache as "null"
    // and a second request will extend the ttl
    async fn http_api_get_mode_s_unknown_cached() {
        let mode_s = "ABABAB".to_owned();
        let key = RedisKey::ModeS(mode_s.clone());
        let application_state = get_application_state().await;
        let path = ModeS::new(mode_s.clone()).unwrap();
        let hm = axum::extract::Query(HashMap::new());
        let response = get_aircraft(application_state.clone(), path.clone(), hm)
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
            .get(key.to_string())
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "null");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604800);

        sleep(1000).await;

        // make sure a second requst to an unknown mode_s will extend cache ttl
        let hm = axum::extract::Query(HashMap::new());
        let response = get_aircraft(application_state.clone(), path, hm)
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
            .get(key.to_string())
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "null");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604800);
    }

    #[tokio::test]
    async fn http_api_get_callsign_ok() {
        let callsign = "TOM35MR".to_owned();
        let application_state = get_application_state().await;
        let path = Callsign::new(callsign.clone()).unwrap();
        let response = get_callsign(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ModelFlightroute {
            callsign: callsign.clone(),
            origin_airport_country_iso_name: "ES".to_owned(),
            origin_airport_country_name: "Spain".to_owned(),
            origin_airport_elevation: 27,
            origin_airport_iata_code: "PMI".to_owned(),
            origin_airport_icao_code: "LEPA".to_owned(),
            origin_airport_latitude: 39.551701,
            origin_airport_longitude: 2.73881,
            origin_airport_municipality: "Palma De Mallorca".to_owned(),
            origin_airport_name: "Palma de Mallorca Airport".to_owned(),
            midpoint_airport_country_iso_name: None,
            midpoint_airport_country_name: None,
            midpoint_airport_elevation: None,
            midpoint_airport_iata_code: None,
            midpoint_airport_icao_code: None,
            midpoint_airport_latitude: None,
            midpoint_airport_longitude: None,
            midpoint_airport_municipality: None,
            midpoint_airport_name: None,
            destination_airport_country_iso_name: "GB".to_owned(),
            destination_airport_country_name: "United Kingdom".to_owned(),
            destination_airport_elevation: 622,
            destination_airport_iata_code: "BRS".to_owned(),
            destination_airport_icao_code: "EGGD".to_owned(),
            destination_airport_latitude: 51.382702,
            destination_airport_longitude: -2.71909,
            destination_airport_municipality: "Bristol".to_owned(),
            destination_airport_name: "Bristol Airport".to_owned(),
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
        let callsign = "TOM35MR".to_owned();
        let application_state = get_application_state().await;
        let path = Callsign::new(callsign.clone()).unwrap();
        let response = get_callsign(application_state.clone(), path).await.unwrap();

        let key = RedisKey::Callsign(callsign);
        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .get(key.to_string())
            .await;
        assert!(result.is_ok());

        let result: ModelFlightroute = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(&result, response.1.response.flightroute.as_ref().unwrap());
        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604800);
    }

    #[tokio::test]
    // Insert a new flightroute using the scraper
    async fn http_api_get_callsign_scraper() {
        let application_state = get_application_state().await;
        let path = Callsign::new(CALLSIGN.to_owned()).unwrap();

        let response = get_callsign(application_state.clone(), path).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);

        let expected = ModelFlightroute {
            callsign: "ANA460".to_owned(),
            origin_airport_country_iso_name: "JP".to_owned(),
            origin_airport_country_name: "Japan".to_owned(),
            origin_airport_elevation: 12,
            origin_airport_iata_code: "OKA".to_owned(),
            origin_airport_icao_code: "ROAH".to_owned(),
            origin_airport_latitude: 26.195801,
            origin_airport_longitude: 127.646004,
            origin_airport_municipality: "Naha".to_owned(),
            origin_airport_name: "Naha Airport / JASDF Naha Air Base".to_owned(),
            midpoint_airport_country_iso_name: None,
            midpoint_airport_country_name: None,
            midpoint_airport_elevation: None,
            midpoint_airport_iata_code: None,
            midpoint_airport_icao_code: None,
            midpoint_airport_latitude: None,
            midpoint_airport_longitude: None,
            midpoint_airport_municipality: None,
            midpoint_airport_name: None,
            destination_airport_country_iso_name: "JP".to_owned(),
            destination_airport_country_name: "Japan".to_owned(),
            destination_airport_elevation: 35,
            destination_airport_iata_code: "HND".to_owned(),
            destination_airport_icao_code: "RJTT".to_owned(),
            destination_airport_latitude: 35.552299,
            destination_airport_longitude: 139.779999,
            destination_airport_municipality: "Tokyo".to_owned(),
            destination_airport_name: "Tokyo Haneda International Airport".to_owned(),
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
        let callsign = "ABABAB".to_owned();
        let application_state = get_application_state().await;
        let path = Callsign::new(callsign.clone()).unwrap();

        let response = get_callsign(application_state.clone(), path.clone())
            .await
            .unwrap_err();
        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, "callsign"),
            _ => unreachable!(),
        };
        let key = RedisKey::Callsign(callsign.clone());
        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .get(key.to_string())
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "null");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604800);

        sleep(1000).await;

        // Check second request is also in redis, and cache ttl gets reset
        let response = get_callsign(application_state.clone(), path)
            .await
            .unwrap_err();

        match response {
            AppError::UnknownInDb(x) => assert_eq!(x, "callsign"),
            _ => unreachable!(),
        };

        let key = RedisKey::Callsign(callsign);
        let result: Result<String, RedisError> = application_state
            .redis
            .lock()
            .await
            .get(key.to_string())
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "null");

        let ttl: usize = application_state
            .redis
            .lock()
            .await
            .ttl(key.to_string())
            .await
            .unwrap();
        assert_eq!(ttl, 604800);
    }

    #[tokio::test]
    async fn http_api_get_callsign_and_flightroute_ok_with_photo() {
        let callsign = "TOM35MR".to_owned();
        let mode_s = "A44F3B".to_owned();
        let application_state = get_application_state().await;
        let path = ModeS::new(mode_s.clone()).unwrap();
        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = get_aircraft(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ModelFlightroute {
            callsign: callsign.clone(),
            origin_airport_country_iso_name: "ES".to_owned(),
            origin_airport_country_name: "Spain".to_owned(),
            origin_airport_elevation: 27,
            origin_airport_iata_code: "PMI".to_owned(),
            origin_airport_icao_code: "LEPA".to_owned(),
            origin_airport_latitude: 39.551701,
            origin_airport_longitude: 2.73881,
            origin_airport_municipality: "Palma De Mallorca".to_owned(),
            origin_airport_name: "Palma de Mallorca Airport".to_owned(),
            midpoint_airport_country_iso_name: None,
            midpoint_airport_country_name: None,
            midpoint_airport_elevation: None,
            midpoint_airport_iata_code: None,
            midpoint_airport_icao_code: None,
            midpoint_airport_latitude: None,
            midpoint_airport_longitude: None,
            midpoint_airport_municipality: None,
            midpoint_airport_name: None,
            destination_airport_country_iso_name: "GB".to_owned(),
            destination_airport_country_name: "United Kingdom".to_owned(),
            destination_airport_elevation: 622,
            destination_airport_iata_code: "BRS".to_owned(),
            destination_airport_icao_code: "EGGD".to_owned(),
            destination_airport_latitude: 51.382702,
            destination_airport_longitude: -2.71909,
            destination_airport_municipality: "Bristol".to_owned(),
            destination_airport_name: "Bristol Airport".to_owned(),
        };

        let aircraft = ModelAircraft {
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
        let path = ModeS::new(mode_s.clone()).unwrap();

        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = get_aircraft(application_state.clone(), path, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();

        assert_eq!(response.0, axum::http::StatusCode::OK);
        let flightroute = ModelFlightroute {
            callsign: callsign.clone(),
            origin_airport_country_iso_name: "ES".to_owned(),
            origin_airport_country_name: "Spain".to_owned(),
            origin_airport_elevation: 27,
            origin_airport_iata_code: "PMI".to_owned(),
            origin_airport_icao_code: "LEPA".to_owned(),
            origin_airport_latitude: 39.551701,
            origin_airport_longitude: 2.73881,
            origin_airport_municipality: "Palma De Mallorca".to_owned(),
            origin_airport_name: "Palma de Mallorca Airport".to_owned(),
            midpoint_airport_country_iso_name: None,
            midpoint_airport_country_name: None,
            midpoint_airport_elevation: None,
            midpoint_airport_iata_code: None,
            midpoint_airport_icao_code: None,
            midpoint_airport_latitude: None,
            midpoint_airport_longitude: None,
            midpoint_airport_municipality: None,
            midpoint_airport_name: None,
            destination_airport_country_iso_name: "GB".to_owned(),
            destination_airport_country_name: "United Kingdom".to_owned(),
            destination_airport_elevation: 622,
            destination_airport_iata_code: "BRS".to_owned(),
            destination_airport_icao_code: "EGGD".to_owned(),
            destination_airport_latitude: 51.382702,
            destination_airport_longitude: -2.71909,
            destination_airport_municipality: "Bristol".to_owned(),
            destination_airport_name: "Bristol Airport".to_owned(),
        };

        let aircraft = ModelAircraft {
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
