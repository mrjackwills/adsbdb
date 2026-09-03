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
    api::response::Stats,
    create_oneshot,
    db::postgres::{ModelAircraft, ModelAirline, ModelFlightroute, ModelIncomingRequest},
    db::redis::{RedisKey, get_cache, insert_cache},
    n_number::{mode_s_to_n_number, n_number_to_mode_s},
};

pub struct RouterHelper;

impl RouterHelper {
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
            if flightroute.is_none() {
                let (one_tx, one_rx) = create_oneshot();

                if state
                    .scraper_tx
                    .send(crate::scraper::MsgScraper::CallSign((
                        one_tx,
                        callsign.clone(),
                    )))
                    .await
                    .is_ok()
                {
                    flightroute = one_rx.await.unwrap_or(None);
                }
            }
            insert_cache(&state.redis, flightroute.as_ref(), redis_key).await?;
            Ok(flightroute)
        }
    }

    /// Get random aircraft, and insert into cache using mode_s
    async fn find_random_flightroute(
        state: &ApplicationState,
    ) -> Result<ModelFlightroute, AppError> {
        let flightroute = ModelFlightroute::get_random(&state.postgres).await?;

        if let Ok(callsign) = Callsign::validate(flightroute.callsign.as_ref()) {
            insert_cache(
                &state.redis,
                Some(&flightroute),
                RedisKey::Callsign(&callsign),
            )
            .await?;
        }

        if let Some(callsign_iata) = flightroute.callsign_iata.as_ref()
            && let Ok(callsign) = Callsign::validate(callsign_iata)
        {
            insert_cache(
                &state.redis,
                Some(&flightroute),
                RedisKey::Callsign(&callsign),
            )
            .await?;
        }
        if let Some(callsign) = flightroute.callsign_icao.as_ref()
            && let Ok(c) = Callsign::validate(callsign)
        {
            insert_cache(&state.redis, Some(&flightroute), RedisKey::Callsign(&c)).await?;
        }

        Ok(flightroute)
    }

    /// Get random aircraft, and insert into cache using mode_s
    async fn find_random_aircraft(state: &ApplicationState) -> Result<ModelAircraft, AppError> {
        let aircraft = ModelAircraft::get_random(&state.postgres, &state.photo_prefixes).await?;

        tokio::try_join!(
            insert_cache(
                &state.redis,
                Some(&aircraft),
                RedisKey::ModeS(&aircraft.mode_s),
            ),
            insert_cache(
                &state.redis,
                Some(&aircraft),
                RedisKey::Registration(&aircraft.registration),
            )
        )?;

        Ok(aircraft)
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
                ModelAircraft::get(&state.postgres, aircraft_search, &state.photo_prefixes).await?;
            if let Some(craft) = aircraft.as_ref()
                && craft.url_photo.is_none()
            {
                let (one_tx, one_rx) = create_oneshot();
                if state
                    .scraper_tx
                    .send(crate::scraper::MsgScraper::Photo((
                        one_tx,
                        craft.mode_s.clone(),
                    )))
                    .await
                    .is_ok()
                {
                    one_rx.await.ok();
                }
                aircraft =
                    ModelAircraft::get(&state.postgres, aircraft_search, &state.photo_prefixes)
                        .await?;
            }
            insert_cache(&state.redis, aircraft.as_ref(), redis_key).await?;
            Ok(aircraft)
        }
    }

    // Return a random airline - not caching at the moment
    async fn find_random_airline(state: &ApplicationState) -> Result<ModelAirline, AppError> {
        ModelAirline::get_random(&state.postgres).await
    }

    /// Check redis cache for Option\<ModelAircraft>, else query postgres
    async fn find_airline(
        state: &ApplicationState,
        airline: &AirlineCode,
    ) -> Result<Option<Vec<ModelAirline>>, AppError> {
        let redis_key = RedisKey::Airline(airline);

        if let Some(airline) = get_cache::<Vec<ModelAirline>>(&state.redis, &redis_key).await? {
            airline.map_or(Err(AppError::UnknownInDb(UnknownAC::Airline)), |airline| {
                Ok(Some(airline))
            })
        } else {
            let airline = ModelAirline::get_all_by_airline_code(&state.postgres, airline).await?;
            insert_cache(&state.redis, airline.as_ref(), redis_key).await?;
            Ok(airline)
        }
    }
}
pub struct ApiRoutes;

impl ApiRoutes {
    /// Return random aircraft, /aircraft/random
    pub async fn aircraft_random_get(
        State(state): State<ApplicationState>,
    ) -> Result<(StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
        Ok((
            StatusCode::OK,
            ResponseJson::new(AircraftAndRoute {
                aircraft: Some(ResponseAircraft::from(
                    RouterHelper::find_random_aircraft(&state).await?,
                )),
                flightroute: None,
            }),
        ))
    }

    /// Return an aircraft detail from a modes input
    /// optional query param of callsign, so can get both aircraft and flightroute in a single request
    /// /aircraft/[:REGISTRATION/:MODE-S] *or* /aircraft/[:REGISTRATION/:MODE-S]?callsign=[:CALLSIGN]
    pub async fn aircraft_get(
        State(state): State<ApplicationState>,
        aircraft_search: AircraftSearch,
        axum::extract::Query(queries): axum::extract::Query<HashMap<String, String>>,
    ) -> Result<(StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
        // Check if optional callsign query param
        if let Some(query_param) = queries.get("callsign") {
            let callsign = Callsign::validate(query_param)?;

            let Some(aircraft) = RouterHelper::find_aircraft(&state, &aircraft_search).await?
            else {
                return Err(AppError::UnknownInDb(UnknownAC::Aircraft));
            };
            let flightroute = RouterHelper::find_flightroute(&state, &callsign).await?;

            Ok((
                StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: Some(ResponseAircraft::from(aircraft)),
                    flightroute: ResponseFlightRoute::from_model(flightroute.as_ref()),
                }),
            ))
        } else {
            let Some(aircraft) = RouterHelper::find_aircraft(&state, &aircraft_search).await?
            else {
                return Err(AppError::UnknownInDb(UnknownAC::Aircraft));
            };
            Ok((
                StatusCode::OK,
                ResponseJson::new(AircraftAndRoute {
                    aircraft: Some(ResponseAircraft::from(aircraft)),
                    flightroute: None,
                }),
            ))
        }
    }

    /// Return a vec random airline, vec will be len 1
    /// /airline/random
    pub async fn airline_random_get(
        State(state): State<ApplicationState>,
    ) -> Result<(axum::http::StatusCode, AsJsonRes<Vec<ResponseAirline>>), AppError> {
        Ok((
            StatusCode::OK,
            ResponseJson::new(vec![ResponseAirline::from(
                RouterHelper::find_random_airline(&state).await?,
            )]),
        ))
    }

    /// Return an airline detail from a ICAO or IATA airline prefix
    /// /airline/[:AIRLINE_CODE]
    pub async fn airline_get(
        State(state): State<ApplicationState>,
        airline_code: AirlineCode,
    ) -> Result<(axum::http::StatusCode, AsJsonRes<Vec<ResponseAirline>>), AppError> {
        let Some(airline) = RouterHelper::find_airline(&state, &airline_code).await? else {
            return Err(AppError::UnknownInDb(UnknownAC::Airline));
        };

        Ok((
            StatusCode::OK,
            ResponseJson::new(
                airline
                    .into_iter()
                    .map(ResponseAirline::from)
                    .collect::<Vec<_>>(),
            ),
        ))
    }

    /// Return an flightroute
    /// /callsign/random
    pub async fn callsign_random_get(
        State(state): State<ApplicationState>,
    ) -> Result<(StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
        Ok((
            StatusCode::OK,
            ResponseJson::new(AircraftAndRoute {
                aircraft: None,
                flightroute: ResponseFlightRoute::from_model(Some(
                    &RouterHelper::find_random_flightroute(&state).await?,
                )),
            }),
        ))
    }

    /// Return a flightroute detail from a callsign input
    /// /callsign/[:CALLSIGN]
    pub async fn callsign_get(
        State(state): State<ApplicationState>,
        callsign: Callsign,
    ) -> Result<(axum::http::StatusCode, AsJsonRes<AircraftAndRoute>), AppError> {
        let Some(flightroute) = RouterHelper::find_flightroute(&state, &callsign).await? else {
            return Err(AppError::UnknownInDb(UnknownAC::Callsign));
        };

        Ok((
            StatusCode::OK,
            ResponseJson::new(AircraftAndRoute {
                aircraft: None,
                flightroute: ResponseFlightRoute::from_model(Some(&flightroute)),
            }),
        ))
    }

    /// Route to convert Mode_S to N-Number
    /// /mode-s/[:MODE-S]
    #[allow(clippy::unused_async)]
    pub async fn mode_s_get(
        mode_s: ModeS,
    ) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
        match mode_s_to_n_number(&mode_s) {
            Ok(n_number) => Ok((StatusCode::OK, ResponseJson::new(n_number.to_string()))),
            Err(_) => Err(AppError::UnknownInDb(UnknownAC::ModeS)),
        }
    }

    /// Route to convert N-Number to Mode_S
    /// /n-number/[:N-NUMBER]
    #[allow(clippy::unused_async)]
    pub async fn n_number_get(
        n_number: NNumber,
    ) -> Result<(axum::http::StatusCode, AsJsonRes<String>), AppError> {
        Ok((
            StatusCode::OK,
            ResponseJson::new(n_number_to_mode_s(&n_number).map_or(S!(), |f| f.to_string())),
        ))
    }

    /// Get adsbdb request stats
    /// /stats
    pub async fn stats_get(
        State(state): State<ApplicationState>,
    ) -> Result<(axum::http::StatusCode, AsJsonRes<Stats>), AppError> {
        Ok((
            StatusCode::OK,
            ResponseJson::new(
                ModelIncomingRequest::get_insert_stats(&state.postgres, &state.redis).await?,
            ),
        ))
    }

    /// Return a simple online status response
    /// /online
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
    /// /*
    #[allow(clippy::unused_async)]
    pub async fn fallback(
        OriginalUri(original_uri): OriginalUri,
    ) -> (StatusCode, AsJsonRes<String>) {
        (
            StatusCode::NOT_FOUND,
            ResponseJson::new(format!("unknown endpoint: {original_uri}")),
        )
    }
}
/// ApiRoutes tests
/// cargo watch -q -c -w src/ -x 'test api_router -- --test-threads=1 --nocapture'
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
    use fred::clients::Pool;
    use fred::interfaces::{ClientLike, HashesInterface, KeysInterface};
    use fred::types::FromValue;
    use serde::de::DeserializeOwned;

    use crate::api::tests::{
        AIRCRAFT_NO_PHOTO, EMPTY_QUERY, FLIGHTROUTE, FLIGHTROUTE_MIDPOINT, FLIGHTROUTE_OTHER,
        UNKNOWN_AIRLINE_IATA, UNKNOWN_AIRLINE_ICAO, UNKNOWN_CALLSIGN_IATA, UNKNOWN_CALLSIGN_ICAO,
        UNKNOWN_CALLSIGN_OTHER, VALID_MODE_S_NO_N_NUMBER, gen_aircraft_with_photo,
    };
    use crate::db::redis::HASH_FIELD;
    use crate::scraper::tests::{TEST_CALLSIGN, remove_scraped_data};
    use crate::sleep;
    use crate::{S, start_incoming_requests};
    use crate::{
        api::{
            Registration,
            input::Validate,
            response::{Airport, ResponseAirline},
        },
        scraper::Scraper,
    };
    use crate::{db, parse_env};

    /// Validate a 200 response
    fn validate_response_ok<T>(response: &(StatusCode, axum::Json<ResponseJson<T>>)) {
        assert_eq!(&response.0, &axum::http::StatusCode::OK);
    }

    /// Get application state for test, also delete recent request stats
    async fn get_application_state() -> State<ApplicationState> {
        let app_env = parse_env::AppEnv::get_env();
        let postgres = db::postgres::get_pool(&app_env).await.unwrap();
        let redis = db::redis::get_pool(&app_env).await.unwrap();
        redis.flushall::<()>(true).await.unwrap();

        let tx_scraper = Scraper::start(&app_env, &postgres);
        let tx_stats = start_incoming_requests(&postgres, &redis).await.unwrap();

        State(ApplicationState::new(
            &app_env, postgres, redis, tx_scraper, tx_stats,
        ))
    }

    async fn validate_ttl(redis: &Pool, key: &str) {
        let ttl: usize = redis.ttl(key).await.unwrap();
        assert!([604_799, 604_800].contains(&ttl));
    }

    /// Validate that the reponse has been cached with the correct TTL
    async fn validate_get_cache<T: DeserializeOwned + FromValue>(redis: &Pool, key: &str) -> T {
        let key = key.to_string();
        validate_ttl(redis, &key).await;
        let data: String = redis.hget(key, HASH_FIELD).await.unwrap();
        serde_json::from_str::<T>(&data).unwrap()
    }

    /// Validate that the cache exists, with a valid TTL, but it's empty, for unknown aircraft/flightroutes etc
    async fn validate_empty_cache(redis: &Pool, key: &str) {
        let key = key.to_string();
        validate_ttl(redis, &key).await;
        let data: String = redis.hget(key, HASH_FIELD).await.unwrap();
        assert_eq!(data, "");
    }

    // Route Handlers

    #[tokio::test]
    // basically a 404 handler
    async fn api_router_fallback_route() {
        let uri = "/test/uri".parse::<Uri>().unwrap();
        let response = ApiRoutes::fallback(OriginalUri(uri.clone())).await;
        assert_eq!(response.0, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(response.1.response, format!("unknown endpoint: {uri}"));
    }

    // Online route - just returns api version and uptime
    // /v[API_VERSION]/online
    #[tokio::test]
    async fn api_router_online_route() {
        let state = get_application_state().await;

        sleep!();
        let response = ApiRoutes::online_get(state).await;

        validate_response_ok(&response);
        assert_eq!(env!("CARGO_PKG_VERSION"), response.1.response.api_version);
        assert!(response.1.response.uptime >= 1);
    }

    // N-Number route - converts from N-Number to Mode-S
    // /v[API_VERSION]/n-number/[N-NUMBER]
    #[tokio::test]
    async fn api_router_n_number_route_valid() {
        let n_number = NNumber::validate(&AIRCRAFT_NO_PHOTO.registration).unwrap();
        let response = ApiRoutes::n_number_get(n_number).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert_eq!(response.1.response, AIRCRAFT_NO_PHOTO.mode_s);
    }

    // Mode-S route - converts from Mode-S to N-Number
    // /v[API_VERSION]/mode-s/[MODE-S]
    #[tokio::test]
    async fn api_router_mode_s_route() {
        let mode_s = ModeS::validate(&AIRCRAFT_NO_PHOTO.mode_s).unwrap();
        let response = ApiRoutes::mode_s_get(mode_s).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert_eq!(response.1.response, AIRCRAFT_NO_PHOTO.registration);
    }

    // Mode-S route - valid ModeS, but not matching N-Number
    // /v[API_VERSION]/mode-s/[MODE-S]
    #[tokio::test]
    async fn api_router_mode_s_route_no_conversion() {
        let mode_s = ModeS::validate(VALID_MODE_S_NO_N_NUMBER).unwrap();
        let response = ApiRoutes::mode_s_get(mode_s).await;
        assert!(response.is_err());
        match response.err().unwrap() {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::ModeS),
            _ => unreachable!(),
        };
    }

    /// Stats route
    /// /v[API_VERSION]/stats
    #[tokio::test]
    async fn api_router_get_stats() {
        let state = get_application_state().await;

        let response = ApiRoutes::stats_get(state.clone()).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[airline_icao]
    #[tokio::test]
    async fn api_router_airline_get_icao() {
        let state = get_application_state().await;
        let known_flightroute = FLIGHTROUTE.clone();
        let airline_code = AirlineCode::Icao(known_flightroute.airline.clone().unwrap().icao);
        let response = ApiRoutes::airline_get(state.clone(), airline_code.clone()).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert_eq!(response.1.response[0], known_flightroute.airline.unwrap());

        // Check cache
        let key = RedisKey::Airline(&airline_code).to_string();
        let cache = validate_get_cache::<Vec<ModelAirline>>(&state.redis, &key).await;
        assert!(cache.len() == 1);

        let cached = ResponseAirline::from(cache.first().unwrap().to_owned());
        assert_eq!(response.1.response[0], cached);
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[airline_iata]
    #[tokio::test]
    async fn api_router_airline_get_iata() {
        let state = get_application_state().await;
        let known_flightroute = FLIGHTROUTE.clone();
        let airline_code =
            AirlineCode::Iata(known_flightroute.airline.clone().unwrap().iata.unwrap());
        let response = ApiRoutes::airline_get(state.clone(), airline_code.clone()).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert_eq!(response.1.response[0], known_flightroute.airline.unwrap());

        // Check cache
        let key = RedisKey::Airline(&airline_code).to_string();
        let cache = validate_get_cache::<Vec<ModelAirline>>(&state.redis, &key).await;
        assert!(cache.len() == 1);

        let cache = ResponseAirline::from(cache.first().unwrap().to_owned());
        assert_eq!(response.1.response[0], cache);
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[unknown_icao]
    #[tokio::test]
    async fn api_router_airline_get_icao_unknown() {
        let state = get_application_state().await;
        let airline_code = AirlineCode::Icao(UNKNOWN_AIRLINE_ICAO.to_owned());
        let response = ApiRoutes::airline_get(state.clone(), airline_code.clone()).await;

        assert!(response.is_err());
        match response.err().unwrap() {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Airline),
            _ => unreachable!(),
        };

        validate_empty_cache(&state.redis, &RedisKey::Airline(&airline_code).to_string()).await;
    }

    /// Airline route
    /// /v[API_VERSION]/airline/[unknown_iata]
    #[tokio::test]
    async fn api_router_airline_get_iata_unknown() {
        let state = get_application_state().await;
        let airline_code = AirlineCode::Iata(UNKNOWN_AIRLINE_IATA.to_owned());
        let response = ApiRoutes::airline_get(state.clone(), airline_code.clone()).await;

        assert!(response.is_err());
        match response.err().unwrap() {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Airline),
            _ => unreachable!(),
        };

        validate_empty_cache(&state.redis, &RedisKey::Airline(&airline_code).to_string()).await;
    }

    /// Airline Random route
    /// /v[API_VERSION]/airline/random
    #[tokio::test]
    async fn api_router_airline_get_random() {
        let state = get_application_state().await;

        for _ in 0..=20 {
            let response = ApiRoutes::airline_random_get(state.clone()).await;
            assert!(response.is_ok());
            let response = response.unwrap();
            validate_response_ok(&response);
            assert!(response.1.response.len() == 1);
        }
    }

    /// Callsign Random route
    /// /v[API_VERSION]/callsign/random
    #[tokio::test]
    async fn api_router_callsign_get_random() {
        let state = get_application_state().await;
        for _ in 0..=20 {
            let response = ApiRoutes::callsign_random_get(state.clone()).await;
            assert!(response.is_ok());
            let response = response.unwrap();
            validate_response_ok(&response);
            let cache = validate_get_cache::<ModelFlightroute>(
                &state.redis,
                &RedisKey::Callsign(
                    &Callsign::validate(
                        &response.1.response.flightroute.as_ref().unwrap().callsign,
                    )
                    .unwrap(),
                )
                .to_string(),
            )
            .await;

            assert_eq!(
                response.1.response.flightroute,
                ResponseFlightRoute::from_model(Some(&cache))
            );

            // TODO!(this is failing)
            // fix with callsign de-normalization

            // if let Some(iata_callsign) = &response
            //     .1
            //     .response
            //     .flightroute
            //     .as_ref()
            //     .unwrap()
            //     .callsign_iata
            // {
            //     let cache = validate_get_cache::<ModelFlightroute>(
            //         &state.redis,
            //         &RedisKey::Callsign(&Callsign::validate(iata_callsign).unwrap()).to_string(),
            //     )
            //     .await;
            //     // Might need to cahgne call to iata callsign?
            //     assert_eq!(
            //         response.1.response.flightroute,
            //         ResponseFlightRoute::from_model(Some(&cache))
            //     );
            // }
        }
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[icao_callsign]
    #[tokio::test]
    async fn api_router_callsign_get_icao() {
        let state = get_application_state().await;
        let known_flightroute = FLIGHTROUTE.clone();
        let callsign =
            Callsign::validate(&known_flightroute.callsign_icao.clone().unwrap()).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), callsign.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.flightroute.is_some());
        let response = response.1.response.flightroute.as_ref().unwrap();
        assert_eq!(&known_flightroute, response);

        // check cache
        let cache = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&callsign).to_string(),
        )
        .await;

        assert_eq!(
            response,
            &ResponseFlightRoute::from_model(Some(&cache)).unwrap()
        );
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[icao_callsign]
    #[tokio::test]
    async fn api_router_callsign_get_icao_404() {
        let state = get_application_state().await;
        let callsign = Callsign::validate(UNKNOWN_CALLSIGN_ICAO).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), callsign.clone()).await;
        assert!(response.is_err());

        match response.err().unwrap() {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Callsign),
            _ => unreachable!(),
        };

        validate_empty_cache(&state.redis, &RedisKey::Callsign(&callsign).to_string()).await;
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[iata_callsign]
    #[tokio::test]
    async fn api_router_callsign_get_iata() {
        let state = get_application_state().await;
        let mut known_flightroute = FLIGHTROUTE.clone();
        let callsign =
            Callsign::validate(&known_flightroute.callsign_iata.clone().unwrap()).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), callsign.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.flightroute.is_some());
        let response = response.1.response.flightroute.as_ref().unwrap();
        known_flightroute.callsign = known_flightroute.callsign_iata.as_ref().unwrap().to_owned();
        assert_eq!(&known_flightroute, response);

        // check cache
        let cache = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&callsign).to_string(),
        )
        .await;

        assert_eq!(
            response,
            &ResponseFlightRoute::from_model(Some(&cache)).unwrap()
        );
    }

    #[tokio::test]
    async fn api_router_callsign_get_iata_404() {
        let state = get_application_state().await;
        let callsign = Callsign::validate(UNKNOWN_CALLSIGN_IATA).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), callsign.clone()).await;
        assert!(response.is_err());

        match response.err().unwrap() {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Callsign),
            _ => unreachable!(),
        };

        validate_empty_cache(&state.redis, &RedisKey::Callsign(&callsign).to_string()).await;
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[other - aka n-number flights]
    #[tokio::test]
    async fn api_router_callsign_get_other() {
        let state = get_application_state().await;
        let known_flightroute = FLIGHTROUTE_OTHER.clone();
        let callsign = Callsign::validate(&known_flightroute.callsign.clone()).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), callsign.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.flightroute.is_some());
        assert_eq!(
            &known_flightroute,
            response.1.response.flightroute.as_ref().unwrap()
        );

        // check cache
        let cache = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&callsign).to_string(),
        )
        .await;

        assert_eq!(
            response.1.response.flightroute.as_ref().unwrap(),
            &ResponseFlightRoute::from_model(Some(&cache)).unwrap()
        );
    }

    /// Callsign route
    /// /v[API_VERSION]/callsign/[other - aka n-number flights]
    #[tokio::test]
    async fn api_router_callsign_get_other_404() {
        let state = get_application_state().await;
        let callsign = Callsign::validate(UNKNOWN_CALLSIGN_OTHER).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), callsign.clone()).await;
        assert!(response.is_err());

        match response.err().unwrap() {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Callsign),
            _ => unreachable!(),
        };

        validate_empty_cache(&state.redis, &RedisKey::Callsign(&callsign).to_string()).await;
    }

    /// Callsign route - with midpoint
    /// /v[API_VERSION]/callsign/[icao_callsign]
    #[tokio::test]
    async fn api_router_callsign_get_icao_midpoint() {
        let state = get_application_state().await;
        let known_flightroute = FLIGHTROUTE_MIDPOINT.clone();
        let callsign =
            Callsign::validate(&known_flightroute.callsign_icao.clone().unwrap()).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), callsign.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.flightroute.is_some());
        let response = response.1.response.flightroute.as_ref().unwrap();
        assert_eq!(&known_flightroute, response);

        // check cache
        let cache = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&callsign).to_string(),
        )
        .await;

        assert_eq!(
            response,
            &ResponseFlightRoute::from_model(Some(&cache)).unwrap()
        );
    }

    /// Callsign route - with midpoint
    /// /v[API_VERSION]/callsign/[iata_callsign]
    #[tokio::test]
    async fn api_router_callsign_get_iata_midpoint() {
        let state = get_application_state().await;
        let mut known_flightroute = FLIGHTROUTE_MIDPOINT.clone();
        let callsign =
            Callsign::validate(&known_flightroute.callsign_iata.clone().unwrap()).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), callsign.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.flightroute.is_some());
        let response = response.1.response.flightroute.as_ref().unwrap();
        known_flightroute.callsign = known_flightroute.callsign_iata.as_ref().unwrap().to_owned();
        assert_eq!(&known_flightroute, response);

        // check cache
        let cache = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&callsign).to_string(),
        )
        .await;

        assert_eq!(
            response,
            &ResponseFlightRoute::from_model(Some(&cache)).unwrap()
        );
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/random
    #[tokio::test]
    async fn api_router_aircraft_random() {
        let state = get_application_state().await;
        for _ in 0..=20 {
            let response = ApiRoutes::aircraft_random_get(state.clone()).await;
            assert!(response.is_ok());
            let response = response.unwrap();
            validate_response_ok(&response);
            // check cache
            let cache = validate_get_cache::<ModelAircraft>(
                &state.redis,
                &RedisKey::ModeS(
                    &ModeS::validate(&response.1.response.aircraft.as_ref().unwrap().mode_s)
                        .unwrap(),
                )
                .to_string(),
            )
            .await;

            assert_eq!(
                response.1.response.aircraft.as_ref().unwrap(),
                &ResponseAircraft::from(cache)
            );

            let cache = validate_get_cache::<ModelAircraft>(
                &state.redis,
                &RedisKey::Registration(
                    &Registration::validate(
                        &response.1.response.aircraft.as_ref().unwrap().registration,
                    )
                    .unwrap(),
                )
                .to_string(),
            )
            .await;

            assert_eq!(
                response.1.response.aircraft.as_ref().unwrap(),
                &ResponseAircraft::from(cache)
            );
        }
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_404() {
        let state = get_application_state().await;
        let mode_s = ModeS::validate("FFFFFF").unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response =
            ApiRoutes::aircraft_get(state.clone(), aircraft_search, EMPTY_QUERY.clone()).await;
        assert!(response.is_err());

        match response.err().unwrap() {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Aircraft),
            _ => unreachable!(),
        };

        validate_empty_cache(&state.redis, &RedisKey::ModeS(&mode_s).to_string()).await;
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]
    #[tokio::test]
    async fn api_router_aircraft_registration_404() {
        let state = get_application_state().await;
        let registration = Registration::validate("FFFFFF").unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response =
            ApiRoutes::aircraft_get(state.clone(), aircraft_search, EMPTY_QUERY.clone()).await;
        assert!(response.is_err());

        match response.err().unwrap() {
            AppError::UnknownInDb(x) => assert_eq!(x, UnknownAC::Aircraft),
            _ => unreachable!(),
        };

        validate_empty_cache(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_no_photo() {
        let state = get_application_state().await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let mode_s = ModeS::validate(&known_aircraft.mode_s).unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response =
            ApiRoutes::aircraft_get(state.clone(), aircraft_search, EMPTY_QUERY.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        let response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, response);

        // check cache
        let cache = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::ModeS(&mode_s).to_string(),
        )
        .await;

        assert_eq!(response, &ResponseAircraft::from(cache));
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_with_photo() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let mode_s = ModeS::validate(&known_aircraft.mode_s).unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response =
            ApiRoutes::aircraft_get(state.clone(), aircraft_search, EMPTY_QUERY.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        let response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, response);

        // check cache
        let cache = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::ModeS(&mode_s).to_string(),
        )
        .await;

        assert_eq!(response, &ResponseAircraft::from(cache));
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]
    #[tokio::test]
    async fn api_router_aircraft_registration_no_photo() {
        let state = get_application_state().await;
        let known_aircraft = AIRCRAFT_NO_PHOTO.clone();
        let registration = Registration::validate(&known_aircraft.registration).unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response =
            ApiRoutes::aircraft_get(state.clone(), aircraft_search, EMPTY_QUERY.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        let response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, response);

        // check cache
        let cache = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;

        assert_eq!(response, &ResponseAircraft::from(cache));
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]
    #[tokio::test]
    async fn api_router_aircraft_registration_with_photo() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let registration = Registration::validate(&known_aircraft.registration).unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response =
            ApiRoutes::aircraft_get(state.clone(), aircraft_search, EMPTY_QUERY.clone()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        let response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, response);

        // check cache
        let cache = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;

        assert_eq!(response, &ResponseAircraft::from(cache));
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]?callsign=[icao]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_with_photo_flightroute_icao() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let known_flightroute = FLIGHTROUTE.clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert(
            "callsign".to_owned(),
            known_flightroute.callsign_icao.as_ref().unwrap().to_owned(),
        );

        let mode_s = ModeS::validate(&known_aircraft.mode_s).unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_some());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        let flightroute_response = response.1.response.flightroute.as_ref().unwrap();
        assert_eq!(&known_flightroute, flightroute_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::ModeS(&mode_s).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        // check aircraft cache
        let cache_flightroute = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(&known_flightroute.callsign).unwrap())
                .to_string(),
        )
        .await;
        assert_eq!(
            flightroute_response,
            &ResponseFlightRoute::from_model(Some(&cache_flightroute)).unwrap()
        );
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]?callsign=[iata]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_with_photo_flightroute_iata() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let mut known_flightroute = FLIGHTROUTE.clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert(
            "callsign".to_owned(),
            known_flightroute.callsign_iata.as_ref().unwrap().to_owned(),
        );

        let mode_s = ModeS::validate(&known_aircraft.mode_s).unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_some());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        let flightroute_response = response.1.response.flightroute.as_ref().unwrap();
        known_flightroute.callsign = known_flightroute.callsign_iata.as_ref().unwrap().to_owned();
        assert_eq!(&known_flightroute, flightroute_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::ModeS(&mode_s).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        // check aircraft cache
        let cache_flightroute = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(
                &Callsign::validate(&known_flightroute.callsign_iata.unwrap()).unwrap(),
            )
            .to_string(),
        )
        .await;
        assert_eq!(
            flightroute_response,
            &ResponseFlightRoute::from_model(Some(&cache_flightroute)).unwrap()
        );
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]?callsign=[other]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_with_photo_flightroute_other() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let known_flightroute = FLIGHTROUTE_OTHER.clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert("callsign".to_owned(), known_flightroute.callsign.to_owned());

        let mode_s = ModeS::validate(&known_aircraft.mode_s).unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_some());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        let flightroute_response = response.1.response.flightroute.as_ref().unwrap();
        assert_eq!(&known_flightroute, flightroute_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::ModeS(&mode_s).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        // check aircraft cache
        let cache_flightroute = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(&known_flightroute.callsign).unwrap())
                .to_string(),
        )
        .await;
        assert_eq!(
            flightroute_response,
            &ResponseFlightRoute::from_model(Some(&cache_flightroute)).unwrap()
        );
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]?callsign=[unknown_icao]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_with_photo_flightroute_unknown_icao() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert("callsign".to_owned(), UNKNOWN_CALLSIGN_ICAO.to_owned());

        let mode_s = ModeS::validate(&known_aircraft.mode_s).unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_none());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::ModeS(&mode_s).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        validate_empty_cache(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(UNKNOWN_CALLSIGN_ICAO).unwrap()).to_string(),
        )
        .await;
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]?callsign=[unknown_iata]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_with_photo_flightroute_unknown_iata() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert("callsign".to_owned(), UNKNOWN_CALLSIGN_IATA.to_owned());

        let mode_s = ModeS::validate(&known_aircraft.mode_s).unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_none());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::ModeS(&mode_s).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        validate_empty_cache(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(UNKNOWN_CALLSIGN_IATA).unwrap()).to_string(),
        )
        .await;
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[mode_s]?callsign=[unknown_other]
    #[tokio::test]
    async fn api_router_aircraft_mode_s_with_photo_flightroute_unknown_other() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert("callsign".to_owned(), UNKNOWN_CALLSIGN_OTHER.to_owned());

        let mode_s = ModeS::validate(&known_aircraft.mode_s).unwrap();
        let aircraft_search = AircraftSearch::ModeS(mode_s.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_none());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::ModeS(&mode_s).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        validate_empty_cache(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(UNKNOWN_CALLSIGN_OTHER).unwrap()).to_string(),
        )
        .await;
    }

    // FIX ME

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]?callsign=[icao]
    #[tokio::test]
    async fn api_router_aircraft_registration_with_photo_flightroute_icao() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let known_flightroute = FLIGHTROUTE.clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert(
            "callsign".to_owned(),
            known_flightroute.callsign_icao.as_ref().unwrap().to_owned(),
        );

        let registration = Registration::validate(&known_aircraft.registration).unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_some());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        let flightroute_response = response.1.response.flightroute.as_ref().unwrap();
        assert_eq!(&known_flightroute, flightroute_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        // check aircraft cache
        let cache_flightroute = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(&known_flightroute.callsign).unwrap())
                .to_string(),
        )
        .await;
        assert_eq!(
            flightroute_response,
            &ResponseFlightRoute::from_model(Some(&cache_flightroute)).unwrap()
        );
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]?callsign=[iata]
    #[tokio::test]
    async fn api_router_aircraft_registration_with_photo_flightroute_iata() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let mut known_flightroute = FLIGHTROUTE.clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert(
            "callsign".to_owned(),
            known_flightroute.callsign_iata.as_ref().unwrap().to_owned(),
        );

        let registration = Registration::validate(&known_aircraft.registration).unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_some());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        known_flightroute.callsign = known_flightroute.callsign_iata.as_ref().unwrap().to_owned();

        let flightroute_response = response.1.response.flightroute.as_ref().unwrap();
        assert_eq!(&known_flightroute, flightroute_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        // check aircraft cache
        let cache_flightroute = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(
                &Callsign::validate(&known_flightroute.callsign_iata.unwrap()).unwrap(),
            )
            .to_string(),
        )
        .await;
        assert_eq!(
            flightroute_response,
            &ResponseFlightRoute::from_model(Some(&cache_flightroute)).unwrap()
        );
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]?callsign=[other]
    #[tokio::test]
    async fn api_router_aircraft_registration_with_photo_flightroute_other() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();
        let known_flightroute = FLIGHTROUTE_OTHER.clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert("callsign".to_owned(), known_flightroute.callsign.to_owned());

        let registration = Registration::validate(&known_aircraft.registration).unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_some());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        let flightroute_response = response.1.response.flightroute.as_ref().unwrap();
        assert_eq!(&known_flightroute, flightroute_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        // check aircraft cache
        let cache_flightroute = validate_get_cache::<ModelFlightroute>(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(&known_flightroute.callsign).unwrap())
                .to_string(),
        )
        .await;
        assert_eq!(
            flightroute_response,
            &ResponseFlightRoute::from_model(Some(&cache_flightroute)).unwrap()
        );
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]?callsign=[unknown_icao]
    #[tokio::test]
    async fn api_router_aircraft_registration_with_photo_flightroute_unknown_icao() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert("callsign".to_owned(), UNKNOWN_CALLSIGN_ICAO.to_owned());

        let registration = Registration::validate(&known_aircraft.registration).unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_none());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        validate_empty_cache(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(UNKNOWN_CALLSIGN_ICAO).unwrap()).to_string(),
        )
        .await;
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]?callsign=[unknown_iata]
    #[tokio::test]
    async fn api_router_aircraft_registration_with_photo_flightroute_unknown_iata() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert("callsign".to_owned(), UNKNOWN_CALLSIGN_IATA.to_owned());

        let registration = Registration::validate(&known_aircraft.registration).unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_none());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        validate_empty_cache(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(UNKNOWN_CALLSIGN_IATA).unwrap()).to_string(),
        )
        .await;
    }

    /// Aircraft route
    /// /v[API_VERSION]/aircraft/[registration]?callsign=[unknown_other]
    #[tokio::test]
    async fn api_router_aircraft_registration_with_photo_flightroute_unknown_other() {
        let state = get_application_state().await;
        let known_aircraft = gen_aircraft_with_photo(&state.photo_prefixes).clone();

        let mut query = EMPTY_QUERY.clone();
        query.insert("callsign".to_owned(), UNKNOWN_CALLSIGN_OTHER.to_owned());

        let registration = Registration::validate(&known_aircraft.registration).unwrap();
        let aircraft_search = AircraftSearch::Registration(registration.clone());

        let response = ApiRoutes::aircraft_get(state.clone(), aircraft_search, query).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);
        assert!(response.1.response.aircraft.is_some());
        assert!(response.1.response.flightroute.is_none());
        let aircraft_response = response.1.response.aircraft.as_ref().unwrap();
        assert_eq!(&known_aircraft, aircraft_response);

        // check aircraft cache
        let cache_aircraft = validate_get_cache::<ModelAircraft>(
            &state.redis,
            &RedisKey::Registration(&registration).to_string(),
        )
        .await;

        assert_eq!(aircraft_response, &ResponseAircraft::from(cache_aircraft));

        validate_empty_cache(
            &state.redis,
            &RedisKey::Callsign(&Callsign::validate(UNKNOWN_CALLSIGN_OTHER).unwrap()).to_string(),
        )
        .await;
    }

    /// Callsign scraper
    /// /v0/callsign/[icao] scraper
    #[tokio::test]
    /// Insert a new flightroute using the scraper
    async fn api_router_get_callsign_scraper() {
        let state = get_application_state().await;
        remove_scraped_data(&state.postgres).await;
        let path = Callsign::validate(TEST_CALLSIGN).unwrap();
        let response = ApiRoutes::callsign_get(state.clone(), path).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        validate_response_ok(&response);

        let expected = ResponseFlightRoute {
            callsign: S!("ANA460"),
            callsign_icao: Some(S!("ANA460")),
            callsign_iata: Some(S!("NH460")),
            airline: Some(ResponseAirline {
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
                iata_code: Some(S!("OKA")),
                icao_code: S!("ROAH"),
                latitude: 26.1958,
                longitude: 127.646,
                municipality: S!("Naha"),
                name: S!("Naha Airport / JASDF Naha Air Base"),
            },
            midpoint: None,
            destination: Airport {
                country_iso_name: S!("JP"),
                country_name: S!("Japan"),
                elevation: 35,
                iata_code: Some(S!("HND")),
                icao_code: S!("RJTT"),
                latitude: 35.5523,
                longitude: 139.78,
                municipality: S!("Tokyo"),
                name: S!("Tokyo Haneda International Airport"),
            },
        };

        assert!(response.1.response.flightroute.is_some());
        let result = response.1.response.flightroute.clone().unwrap();
        assert_eq!(result, expected);
        remove_scraped_data(&state.postgres).await;
    }
}
