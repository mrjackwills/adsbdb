use std::collections::HashMap;

use axum::Extension;
use serde::{Deserialize, Serialize};

use super::{AppError, ApplicationState, AsJsonRes, ResponseJson};
use crate::db_postgres::{Model, ModelAircraft, ModelFlightroute};
use crate::db_redis::{get_cache, insert_cache, RedisKey};

/// Response for the /online api route
#[derive(Serialize, Deserialize)]
pub struct Online {
    uptime: u64,
    api_version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AircraftAndRoute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aircraft: Option<ModelAircraft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flightroute: Option<ModelFlightroute>,
}

// Check if input char is 0-9, a-end
fn is_charset(c: char, end: char) -> bool {
    c.is_ascii_digit() || ('a'..=end).contains(&c.to_ascii_lowercase())
}

//  Make sure that input is a valid callsignstring, validitiy is [a-z]{4-8}
// Should accept str or string
pub fn check_callsign(input: String) -> Result<String, AppError> {
    let valid = (4..=8).contains(&input.len()) && input.chars().all(|c| is_charset(c, 'z'));
    if valid {
        Ok(input.to_uppercase())
    } else {
        Err(AppError::Callsign(input))
    }
}

/// Make sure that input is a valid mode_s string, validitiy is [a-f]{6}
fn check_mode_s(input: String) -> Result<String, AppError> {
    let valid = input.len() == 6 && input.chars().all(|c| is_charset(c, 'f'));
    if valid {
        Ok(input.to_uppercase())
    } else {
        Err(AppError::ModeS(input))
    }
}

/// Get flightroute, refactored so can use in either get_mode_s (with a callsign query param), or get_callsign.
/// Check redis cache for aircraft (or 'none'), or hit postgres
async fn find_flightroute(
    callsign: String,
    state: ApplicationState,
) -> Result<Option<ModelFlightroute>, AppError> {
    let redis_key = RedisKey::Callsign(callsign.clone());
    let cache: Option<Option<ModelFlightroute>> = get_cache(&state.redis, &redis_key).await?;
    if let Some(flightroute) = cache {
        Ok(flightroute)
    } else {
        let mut flightroute = ModelFlightroute::get(&state.postgres, &callsign).await?;

        if flightroute.is_none() {
            flightroute = state
                .scraper
                .scrape_flightroute(&state.postgres, &callsign)
                .await?
        }
        insert_cache(&state.redis, &flightroute, &redis_key).await?;
        Ok(flightroute)
    }
}

/// Check redis cache for aircraft (or 'none'), or hit postgres
async fn find_aircraft(
    mode_s: String,
    state: ApplicationState,
) -> Result<Option<ModelAircraft>, AppError> {
    let mode_s = check_mode_s(mode_s)?;
    let redis_key = RedisKey::ModeS(mode_s.clone());
    let cache: Option<Option<ModelAircraft>> = get_cache(&state.redis, &redis_key).await?;
    if let Some(aircraft) = cache {
        Ok(aircraft)
    } else {
        let mut aircraft = ModelAircraft::get(&state.postgres, &mode_s, &state.url_prefix).await?;
        if let Some(craft) = aircraft.clone() {
            if craft.url_photo.is_none() {
                state
                    .scraper
                    .scrape_photo(&state.postgres, mode_s.clone())
                    .await?;
                aircraft = ModelAircraft::get(&state.postgres, &mode_s, &state.url_prefix).await?;
            }
        }
        insert_cache(&state.redis, &aircraft, &redis_key).await?;
        Ok(aircraft)
    }
}

/// Return an aircraft detail from a modes input
/// optional query param of callsign, so can get both aircraft and flightroute in a single request
pub async fn get_mode_s(
    Extension(state): Extension<ApplicationState>,
    axum::extract::Path(mode_s): axum::extract::Path<String>,
    axum::extract::Query(callsign): axum::extract::Query<HashMap<String, String>>,
) -> Result<(axum::http::StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
    if let Some(callsign) = callsign.get("callsign") {
        let (aircraft, flightroute) = tokio::join!(
            find_aircraft(mode_s, state.clone()),
            find_flightroute(callsign.to_owned(), state)
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
        let aircraft = find_aircraft(mode_s, state).await?;
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
    axum::extract::Path(callsign): axum::extract::Path<String>,
) -> Result<(axum::http::StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
    let callsign = check_callsign(callsign)?;
    let flightroute = find_flightroute(callsign, state).await?;

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
    use axum::response::IntoResponse;
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

    #[test]
    fn http_api_is_charset_valid() {
        let char = 'a';
        let result = is_charset(char, 'z');
        assert!(result);

        let char = '1';
        let result = is_charset(char, 'b');
        assert!(result);
    }

    #[test]
    fn http_api_is_charset_invalid() {
        let char = 'g';
        let result = is_charset(char, 'b');
        assert!(!result);

        let char = '%';
        let result = is_charset(char, 'b');
        assert!(!result);
    }

    #[test]
    fn http_api_check_callsign_ok() {
        let valid = String::from("AaBb12");
        let result = check_callsign(valid);
        assert_eq!(result.unwrap(), "AABB12".to_owned());

        let valid = String::from("AaaA1111");
        let result = check_callsign(valid);
        assert_eq!(result.unwrap(), "AAAA1111".to_owned());
    }

    #[test]
    fn http_api_check_callsign_err() {
        // Too short
        let valid = String::from("aaa");
        let result = check_callsign(valid);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Callsign(a) => assert_eq!(a, "aaa".to_owned()),
            _ => unreachable!(),
        };

        // Too long
        let valid = String::from("bbbbbbbbb");
        let result = check_callsign(valid);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Callsign(a) => assert_eq!(a, "bbbbbbbbb".to_owned()),
            _ => unreachable!(),
        };

        // contains invalid char
        let valid = String::from("aaa124*");
        let result = check_callsign(valid);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Callsign(a) => assert_eq!(a, "aaa124*".to_owned()),
            _ => unreachable!(),
        };
    }

    #[test]
    fn http_api_check_mode_s_ok() {
        let valid = String::from("AaBb12");
        let result = check_mode_s(valid);
        assert_eq!(result.unwrap(), "AABB12".to_owned());

        let valid = String::from("FFF999");
        let result = check_mode_s(valid);
        assert_eq!(result.unwrap(), "FFF999".to_owned());
    }

    #[test]
    fn http_api_check_mode_s_err() {
        // Too short
        let valid = String::from("aaaaa");
        let result = check_mode_s(valid);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ModeS(a) => assert_eq!(a, "aaaaa".to_owned()),
            _ => unreachable!(),
        };

        // Too long
        let valid = String::from("bbbbbbb");
        let result = check_mode_s(valid);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ModeS(a) => assert_eq!(a, "bbbbbbb".to_owned()),
            _ => unreachable!(),
        };

        // contains invalid alpha char
        let valid = String::from("aaa12h");
        let result = check_mode_s(valid);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ModeS(a) => assert_eq!(a, "aaa12h".to_owned()),
            _ => unreachable!(),
        };

        // contains invalid non-alpha char
        let valid = String::from("aaa12$");
        let result = check_mode_s(valid);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ModeS(a) => assert_eq!(a, "aaa12$".to_owned()),
            _ => unreachable!(),
        };
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
    async fn http_api_get_mode_s_err() {
        let application_state = get_application_state().await;
        let invalid_mode_s = axum::extract::Path("bbbbbbb".to_owned());
        let hm = axum::extract::Query(HashMap::new());
        let response = get_mode_s(application_state, invalid_mode_s, hm).await;
        assert!(response.is_err());
        let response = response.unwrap_err();
        match &response {
            AppError::ModeS(x) => assert_eq!(x.to_owned(), "bbbbbbb".to_owned()),
            _ => unreachable!(),
        };
        assert_eq!(response.to_string(), "invalid modeS:".to_owned());
        assert_eq!(
            response.into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn http_api_get_mode_s_ok_with_photo() {
        let mode_s = "A44F3B".to_owned();
        let application_state = get_application_state().await;
        let invalid_mode_s = axum::extract::Path(mode_s.clone());
        let hm = axum::extract::Query(HashMap::new());
        let response = get_mode_s(application_state.clone(), invalid_mode_s, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ModelAircraft {
            aircraft_type: "Citation Sovereign".to_owned(),
            icao_type: "C680".to_owned(),
            manufacturer: "Cessna".to_owned(),
            mode_s: mode_s.clone(),
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
        let application_state = get_application_state().await;
        let invalid_mode_s = axum::extract::Path(mode_s.clone());
        let hm = axum::extract::Query(HashMap::new());
        let response = get_mode_s(application_state.clone(), invalid_mode_s, hm).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.0, axum::http::StatusCode::OK);
        let aircraft = ModelAircraft {
            aircraft_type: "737MAX 9".to_owned(),
            icao_type: "B39M".to_owned(),
            manufacturer: "Boeing".to_owned(),
            mode_s: mode_s.clone(),
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
        let valid_mode_s = axum::extract::Path(mode_s.clone());
        let hm = axum::extract::Query(HashMap::new());
        let response = get_mode_s(application_state.clone(), valid_mode_s, hm)
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
        let valid_mode_s = axum::extract::Path(mode_s.clone());
        let hm = axum::extract::Query(HashMap::new());
        let response = get_mode_s(application_state.clone(), valid_mode_s, hm)
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
        let valid_mode_s = axum::extract::Path(mode_s.clone());
        let hm = axum::extract::Query(HashMap::new());
        let response = get_mode_s(application_state.clone(), valid_mode_s, hm)
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
        let valid_mode_s = axum::extract::Path(mode_s.clone());
        let hm = axum::extract::Query(HashMap::new());
        let response = get_mode_s(application_state.clone(), valid_mode_s, hm)
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
    async fn http_api_get_callsign_s_err() {
        let application_state = get_application_state().await;
        let invalid_callsign = axum::extract::Path("bbbbbbbbb".to_owned());
        let response = get_callsign(application_state, invalid_callsign).await;
        assert!(response.is_err());
        let response = response.unwrap_err();
        match &response {
            AppError::Callsign(x) => assert_eq!(x.to_owned(), "bbbbbbbbb".to_owned()),
            _ => unreachable!(),
        };
        assert_eq!(response.to_string(), "invalid callsign:".to_owned());
        assert_eq!(
            response.into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn http_api_get_callsign_ok() {
        let callsign = "TOM35MR".to_owned();
        let application_state = get_application_state().await;
        let invalid_mode_s = axum::extract::Path(callsign.clone());
        let response = get_callsign(application_state.clone(), invalid_mode_s).await;

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
        let valid_callsign = axum::extract::Path(callsign.clone());
        let response = get_callsign(application_state.clone(), valid_callsign)
            .await
            .unwrap();

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
        let valid_callsign = axum::extract::Path(CALLSIGN.to_owned());
        let response = get_callsign(application_state.clone(), valid_callsign).await;

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
        let valid_callsign = axum::extract::Path(callsign.clone());
        let response = get_callsign(application_state.clone(), valid_callsign)
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

        let valid_callsign = axum::extract::Path(callsign.clone());

        // Check second request is also in redis, and cache ttl gets reset
        let response = get_callsign(application_state.clone(), valid_callsign)
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
        let mode_s_path = axum::extract::Path(mode_s.clone());
        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = get_mode_s(application_state.clone(), mode_s_path, hm).await;

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
        let mode_s_path = axum::extract::Path(mode_s.clone());
        let mut hm = HashMap::new();
        hm.insert("callsign".to_owned(), callsign.clone());
        let hm = axum::extract::Query(hm);
        let response = get_mode_s(application_state.clone(), mode_s_path, hm).await;

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
