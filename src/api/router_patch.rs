use axum::{
    extract::{
        FromRequest, State,
        rejection::{JsonDataError, JsonRejection},
    },
    http::{HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use fred::prelude::KeysInterface;
use reqwest::StatusCode;
use serde::Deserialize;

use std::error::Error;

use crate::{
    S,
    api::UnknownAC,
    argon::ArgonHash,
    db::postgres::{ModelAircraft, ModelAirport, ModelFlightroute},
};

use super::{AppError, ApplicationState, Callsign, ModeS, response::ResponseAircraft};

/// Verify the Authorization header against the app_env.argon_hash
pub async fn auth_header(
    State(argon_hash): State<ArgonHash>,
    headers: HeaderMap,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    if let Some(auth_header) = headers.get("Authorization") {
        match argon_hash
            .verify_password(auth_header.to_str().unwrap_or_default())
            .await
        {
            Ok(valid) => {
                if valid {
                    return Ok(next.run(req).await);
                }
            }
            Err(e) => tracing::error!("{e}"),
        }
    }
    Err(AppError::Authorization)
}

/// attempt to downcast `err` into a `T` and if that fails recursively try and
/// downcast `err`'s source
fn find_error_source<'a, T>(err: &'a (dyn Error + 'static)) -> Option<&'a T>
where
    T: Error + 'static,
{
    err.downcast_ref::<T>().map_or_else(
        || err.source().and_then(|source| find_error_source(source)),
        Some,
    )
}

/// attempt to extract the inner `serde_json::Error`, if that succeeds we can
/// provide a more specific error
// see https://docs.rs/axum/latest/axum/extract/index.html#accessing-inner-errors
fn extract_serde_error<E>(e: E) -> AppError
where
    E: Error + 'static,
{
    if let Some(err) = find_error_source::<JsonDataError>(&e) {
        let text = err.body_text();
        if text.contains("missing field") {
            return AppError::Body(S!(text
                .split_once("missing field `")
                .map_or("", |f| f.1)
                .split_once('`')
                .map_or("", |f| f.0.trim())));
        } else if text.contains("unknown field") {
            return AppError::Body(S!("invalid input"));
        } else if text.contains("at line") {
            return AppError::Body(S!(text
                .split_once("at line")
                .map_or("", |f| f.0)
                .split_once(':')
                .map_or("", |f| f.1)
                .split_once(':')
                .map_or("", |f| f.1.trim())));
        }
    }
    AppError::Internal(S!("downcast error"))
}

pub struct IncomingJson<T>(pub T);

/// Implement custom error handing for JSON extraction on incoming JSON
/// Either return valid json (meeting a struct spec listed below), or return an ApiError
/// Then each route handler, can use `IncomingJson(body): IncomingJson<T>`, to extract T into param body
impl<S, T> FromRequest<S> for IncomingJson<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(
        req: Request<axum::body::Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => match rejection {
                JsonRejection::JsonDataError(e) => Err(extract_serde_error(e)),
                JsonRejection::JsonSyntaxError(_) => Err(AppError::Body(S!("JSON syntax"))),
                JsonRejection::MissingJsonContentType(e) => {
                    tracing::trace!("{e:?}");
                    Err(AppError::Body(S!("\"application/json\" header")))
                }
                JsonRejection::BytesRejection(e) => {
                    tracing::trace!("{e:?}");
                    tracing::trace!("BytesRejection");
                    Err(AppError::Body(S!("Bytes Rejected")))
                }
                _ => Err(AppError::Body(S!("IncomingJson from_request error"))),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdatedCallsign {
    origin: String,
    destination: String,
}

/// Just some simple to checks to make sure that the new aircraft body is valid
/// Just basic checks, could still result in invalid data being inputted
/// But it's better than nothing, the limits are the current max length for each +10%
fn check_aircraft_body_length(body: &ResponseAircraft) -> Result<(), ()> {
    let counter = |x: &str, limit: usize| x.chars().count() > limit;
    if counter(&body.aircraft_type, 65)
        || counter(&body.icao_type, 6)
        || counter(&body.registration, 14)
        || counter(&body.manufacturer, 80)
        || counter(&body.registered_owner, 121)
        || counter(
            &body
                .registered_owner_operator_flag_code
                .clone()
                .unwrap_or_default(),
            5,
        )
    {
        Err(())
    } else {
        Ok(())
    }
}

/// Return a flightroute detail from a callsign input
pub async fn callsign_patch(
    State(state): State<ApplicationState>,
    callsign: Callsign,
    IncomingJson(body): IncomingJson<UpdatedCallsign>,
) -> Result<StatusCode, AppError> {
    let Some(flightroute) = ModelFlightroute::get(&state.postgres, &callsign).await else {
        return Err(AppError::UnknownInDb(UnknownAC::Callsign));
    };
    let Some(origin) = ModelAirport::get(&state.postgres, &body.origin).await? else {
        return Err(AppError::UnknownInDb(UnknownAC::Airport(S!(body.origin))));
    };
    let Some(destination) = ModelAirport::get(&state.postgres, &body.destination).await? else {
        return Err(AppError::UnknownInDb(UnknownAC::Airport(S!(
            body.destination
        ))));
    };

    if body.origin == flightroute.origin_airport_icao_code
        && body.destination == flightroute.destination_airport_icao_code
    {
        return Err(AppError::Body(S!("no change")));
    }
    flightroute
        .update(&state.postgres, origin, destination)
        .await?;

    if let Some(iata) = flightroute.callsign_iata.as_ref() {
        state
            .redis
            .del::<(), String>(format!("callsign::{iata}"))
            .await?;
    }

    if let Some(icao) = flightroute.callsign_icao.as_ref() {
        state
            .redis
            .del::<(), String>(format!("callsign::{icao}"))
            .await?;
    }
    Ok(StatusCode::OK)
}

/// Check if a known aircraft, and a updated body, match
/// Returns true if they are equal
fn model_body_equal(c: &ModelAircraft, b: &ResponseAircraft) -> bool {
    b.aircraft_type == c.aircraft_type
        && b.icao_type == c.icao_type
        && b.manufacturer == c.manufacturer
        && b.mode_s == c.mode_s.to_string().as_str()
        && b.registration == c.registration.to_string().as_str()
        && b.registered_owner_country_iso_name == c.registered_owner_country_iso_name
        && b.registered_owner_country_name == c.registered_owner_country_name
        && b.registered_owner_operator_flag_code == c.registered_owner_operator_flag_code
        && b.registered_owner == c.registered_owner
        && b.url_photo == c.url_photo
        && b.url_photo_thumbnail == c.url_photo_thumbnail
}

// At the moment this is only for mode_s, where the aircraft GET endpoint can search by registration as well
pub async fn aircraft_patch(
    State(state): State<ApplicationState>,
    mode_s: ModeS,
    IncomingJson(body): IncomingJson<ResponseAircraft>,
) -> Result<StatusCode, AppError> {
    let Some(current_aircraft) = ModelAircraft::get(
        &state.postgres,
        &super::AircraftSearch::ModeS(mode_s),
        &state.photo_prefixes,
    )
    .await?
    else {
        return Err(AppError::UnknownInDb(UnknownAC::Aircraft));
    };

    // Simple check to make sure the values aren't excessively large
    // Could also check to validity of registration/owner/type etc, but there's a lot of factors in that
    if check_aircraft_body_length(&body).is_err() {
        return Err(AppError::Body(S!("value too long")));
    }

    // This isn't elegant, but just check if thers any difference between the current aircraft in DB, and the new aircraft in the body
    if model_body_equal(&current_aircraft, &body) {
        return Err(AppError::Body(S!("no change")));
    }

    // At the moment, don't allow update if the photo_url's or mode_s has been changed
    if current_aircraft.url_photo != body.url_photo
        || current_aircraft.url_photo_thumbnail != body.url_photo_thumbnail
        || current_aircraft.mode_s.to_string() != body.mode_s
    {
        return Err(AppError::Body(S!("immutable value changed")));
    }

    current_aircraft.update(state.postgres, &body).await?;

    // Delete caches
    tokio::try_join!(
        state
            .redis
            .del::<(), String>(format!("mode_s::{}", current_aircraft.mode_s)),
        state
            .redis
            .del::<(), String>(format!("registration::{}", current_aircraft.registration)),
        state
            .redis
            .del::<(), String>(format!("registration::{}", body.registration))
    )?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
#[allow(clippy::pedantic, clippy::unwrap_used)]
pub mod tests {
    use std::collections::HashMap;
    use std::sync::LazyLock;

    use super::*;

    use crate::S;
    use crate::api::{
        API_VERSION, Registration, Validate,
        response::{AircraftAndRoute, Airport, ResponseFlightRoute, ResponseJson},
        serve,
        tests::{CLIENT, test_setup},
        tests::{FLIGHTROUTE, UNKNOWN_CALLSIGN_ICAO, get_response_200, random_hex},
    };
    use crate::db::redis::{HASH_FIELD, RedisKey};
    use crate::parse_env::AppEnv;
    use crate::scraper::Scraper;
    use crate::sleep;
    use crate::start_incoming_requests;

    use fred::prelude::{HashesInterface, Pool};
    use reqwest::StatusCode;
    use serde::Serialize;
    use sqlx::PgPool;
    use tokio::task::JoinHandle;

    pub struct TestSetup {
        pub _handle: Option<JoinHandle<()>>,
        pub _app_env: AppEnv,
        pub _postgres: PgPool,
        pub redis: Pool,
    }

    async fn send_patch_request<T: Serialize>(
        path: &str,
        headers: Option<(&str, &str)>,
        body: Option<T>,
    ) -> reqwest::Response {
        let mut request = CLIENT.patch(format!(
            "http://127.0.0.1:8282{}/{path}",
            API_VERSION.as_str()
        ));

        if let Some(headers) = headers {
            request = request.header(headers.0, headers.1);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await;
        assert!(response.is_ok());
        response.unwrap()
    }

    /// Start the server, if allow_update is some, then set the environmental variable to allow PATCH update requests
    async fn start_server(allow_update: Option<()>) -> TestSetup {
        let setup = test_setup().await;
        let mut app_env = setup.app_env.clone();

        if allow_update.is_none() {
            app_env.allow_update = None;
        }
        let spawn_env = app_env.clone();

        let postgres = setup.postgres.clone();

        let redis = setup.redis.clone();

        let tx_scraper = Scraper::start(&app_env, &postgres);
        let tx_stats = start_incoming_requests(&postgres, &redis).await.unwrap();

        let handle = tokio::spawn(async move {
            serve(spawn_env, postgres, redis, tx_scraper, tx_stats)
                .await
                .unwrap();
        });
        // just sleep to make sure the server is running - 1ms is enough
        sleep!(100);
        TestSetup {
            _handle: Some(handle),
            _app_env: app_env,
            _postgres: setup.postgres,
            redis: setup.redis,
        }
    }

    /// Test based on this pre-known aircraft
    fn gen_aircraft() -> ResponseAircraft {
        ResponseAircraft {
            aircraft_type: S!("737NG 8U3/W"),
            icao_type: S!("B738"),
            manufacturer: S!("Boeing"),
            mode_s: S!("8A043A"),
            registration: S!("PK-GNE"),
            registered_owner_country_iso_name: S!("ID"),
            registered_owner_country_name: S!("Indonesia"),
            registered_owner_operator_flag_code: Some(S!("GIA")),
            registered_owner: S!("Garuda Indonesia"),
            url_photo: None,
            url_photo_thumbnail: None,
        }
    }

    const AIRCRAFT: &str = "8A043A";
    // const CALLSIGN: &str = "CFE37E";

    static DEFAULT_AIRCRAFT_PATH: LazyLock<String> =
        LazyLock::new(|| format!("aircraft/{AIRCRAFT}"));

    static DEFAULT_CALLSIGN_PATH: LazyLock<String> =
        LazyLock::new(|| format!("callsign/{}", FLIGHTROUTE.callsign));

    /// Test based on this pre-known aircraft
    static DEFAULT_AIRCRAFT_BODY: LazyLock<ResponseAircraft> = LazyLock::new(|| ResponseAircraft {
        aircraft_type: S!("737NG 8U3/W"),
        icao_type: S!("B738"),
        manufacturer: S!("Boeing"),
        mode_s: S!("8A043A"),
        registration: S!("PK-GNE"),
        registered_owner_country_iso_name: S!("ID"),
        registered_owner_country_name: S!("Indonesia"),
        registered_owner_operator_flag_code: Some(S!("GIA")),
        registered_owner: S!("Garuda Indonesia"),
        url_photo: None,
        url_photo_thumbnail: None,
    });

    const VALID_AUTH_HEADER: (&str, &str) = ("Authorization", "password123");

    async fn cache_aircraft_exists(redis: &Pool) -> bool {
        redis
            .hexists(
                RedisKey::ModeS(&ModeS::validate(AIRCRAFT).unwrap()).to_string(),
                HASH_FIELD,
            )
            .await
            .unwrap()
            && redis
                .hexists(
                    RedisKey::Registration(
                        &Registration::validate(&DEFAULT_AIRCRAFT_BODY.registration).unwrap(),
                    )
                    .to_string(),
                    HASH_FIELD,
                )
                .await
                .unwrap()
    }

    async fn cache_callsign_exists(redis: &Pool) -> bool {
        let flightroute = FLIGHTROUTE.clone();
        redis
            .hexists(
                RedisKey::Callsign(&Callsign::validate(&flightroute.callsign).unwrap()).to_string(),
                HASH_FIELD,
            )
            .await
            .unwrap()
            && redis
                .hexists(
                    RedisKey::Callsign(
                        &Callsign::validate(&flightroute.callsign_iata.unwrap()).unwrap(),
                    )
                    .to_string(),
                    HASH_FIELD,
                )
                .await
                .unwrap()
    }

    /// Return the aircraft details from redis
    async fn get_aircraft_cache(redis: &Pool) -> ModelAircraft {
        redis
            .hget(
                RedisKey::ModeS(&ModeS::validate(AIRCRAFT).unwrap()).to_string(),
                HASH_FIELD,
            )
            .await
            .unwrap()
    }

    /// Return the aircraft details from redis
    async fn get_callsign_cache(redis: &Pool) -> ModelFlightroute {
        redis
            .hget(
                RedisKey::Callsign(&Callsign::validate(&FLIGHTROUTE.callsign).unwrap()).to_string(),
                HASH_FIELD,
            )
            .await
            .unwrap()
    }

    /// Reset aircraft details back to original
    async fn reset_aircraft() {
        send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(VALID_AUTH_HEADER),
            Some(DEFAULT_AIRCRAFT_BODY.clone()),
        )
        .await;
    }

    /// Reset callsign back to original
    async fn reset_callsign() {
        send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([
                ("origin", FLIGHTROUTE.origin.icao_code.as_str()),
                ("destination", FLIGHTROUTE.destination.icao_code.as_str()),
            ])),
        )
        .await;
    }

    async fn invalid_aircraft_body_test(body: ResponseAircraft, msg: &str) {
        let response =
            send_patch_request(&DEFAULT_AIRCRAFT_PATH, Some(VALID_AUTH_HEADER), Some(body)).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            msg
        );
    }

    async fn invalid_callsign_body_test(body: HashMap<&str, &str>, msg: &str) {
        let response =
            send_patch_request(&DEFAULT_CALLSIGN_PATH, Some(VALID_AUTH_HEADER), Some(body)).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            msg
        );
    }
    // REFACTOR THESE
    fn assert_dublin(airport: &Airport) {
        assert_eq!(airport.country_iso_name, "IE");
        assert_eq!(airport.country_name, "Ireland");
        assert_eq!(airport.elevation, 242);
        assert_eq!(airport.iata_code, Some(S!("DUB")));
        assert_eq!(airport.icao_code, "EIDW");
        assert_eq!(airport.latitude, 53.421299);
        assert_eq!(airport.longitude, -6.27007);
        assert_eq!(airport.municipality, "Dublin");
        assert_eq!(airport.name, "Dublin Airport");
    }

    fn assert_san_jose(airport: &Airport) {
        assert_eq!(airport.country_iso_name, S!("CR"));
        assert_eq!(airport.country_name, S!("Costa Rica"));
        assert_eq!(airport.elevation, 3021);
        assert_eq!(airport.iata_code, Some(S!("SJO")));
        assert_eq!(airport.icao_code, S!("MROC"));
        assert_eq!(airport.latitude, 9.99386);
        assert_eq!(airport.longitude, -84.208801);
        assert_eq!(airport.municipality, S!("San José (Alajuela)"));
        assert_eq!(airport.name, S!("Juan Santamaría International Airport"));
    }

    fn assert_montreal(airport: &Airport) {
        assert_eq!(airport.country_iso_name, "CA");
        assert_eq!(airport.country_name, "Canada");
        assert_eq!(airport.elevation, 118);
        assert_eq!(airport.iata_code, Some(S!("YUL")));
        assert_eq!(airport.icao_code, "CYUL");
        assert_eq!(airport.latitude, 45.4706001282);
        assert_eq!(airport.longitude, -73.7407989502);
        assert_eq!(airport.municipality, "Montréal");
        assert_eq!(
            airport.name,
            "Montreal / Pierre Elliott Trudeau International Airport"
        );
    }

    //
    // Aircraft Tests
    //

    #[tokio::test]
    /// env.update is None, 405 response
    async fn http_mod_patch_aircraft_no_update() {
        start_server(None).await;

        let response = send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(VALID_AUTH_HEADER),
            Some(DEFAULT_AIRCRAFT_BODY.clone()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    /// No auth header, return 401
    async fn http_mod_patch_aircraft_no_header() {
        start_server(Some(())).await;

        let response = send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            None,
            Some(DEFAULT_AIRCRAFT_BODY.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    /// invalid auth header, return 401
    async fn http_mod_patch_aircraft_invalid_header() {
        start_server(Some(())).await;

        let response = send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(("Authorization", "authorization")),
            Some(DEFAULT_AIRCRAFT_BODY.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    /// No body, return 400
    async fn http_mod_patch_aircraft_no_body() {
        start_server(Some(())).await;

        let response =
            send_patch_request::<()>(&DEFAULT_AIRCRAFT_PATH, Some(VALID_AUTH_HEADER), None).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            "invalid body \"application/json\" header"
        );
    }

    #[tokio::test]
    /// Unknown aircraft, return 404
    async fn http_mod_patch_aircraft_unknown() {
        start_server(Some(())).await;

        let response = send_patch_request(
            "aircraft/101010",
            Some(VALID_AUTH_HEADER),
            Some(DEFAULT_AIRCRAFT_BODY.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            "unknown aircraft"
        );
    }

    #[tokio::test]
    /// Invalid body, return 401
    async fn http_mod_patch_aircraft_invalid_body() {
        start_server(Some(())).await;

        let response = send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(VALID_AUTH_HEADER),
            Some(&HashMap::from([("thing", "other")])),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            "invalid body type"
        );
    }

    #[tokio::test]
    /// No change to data, return 400
    async fn http_mod_patch_aircraft_no_change() {
        start_server(Some(())).await;

        invalid_aircraft_body_test(DEFAULT_AIRCRAFT_BODY.clone(), "invalid body no change").await;
    }

    #[tokio::test]
    /// Changes to either photo url's or mode_s will result in a 400 error
    async fn http_mod_patch_aircraft_invalid_body_url_mode_s() {
        start_server(Some(())).await;
        let msg = "invalid body immutable value changed";

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.mode_s = S!("AAAAAA");
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.url_photo = Some(random_hex(10));
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.url_photo_thumbnail = Some(random_hex(10));
        invalid_aircraft_body_test(body, msg).await;
    }

    #[tokio::test]
    /// Unknown country returns a 400 error
    async fn http_mod_patch_aircraft_invalid_body_country() {
        start_server(Some(())).await;

        let msg = "invalid body unknown country";

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.registered_owner_country_name = S!("AAAAAA");
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.registered_owner_country_iso_name = S!("XX");
        invalid_aircraft_body_test(body, msg).await;
    }

    #[tokio::test]
    /// New values rejected if too long
    async fn http_mod_patch_aircraft_invalid_lengths() {
        start_server(Some(())).await;

        let msg = "invalid body value too long";

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.aircraft_type = random_hex(66);
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.icao_type = random_hex(7);
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.registration = random_hex(15);
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.manufacturer = random_hex(81);
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.registered_owner = random_hex(122);
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.registered_owner_operator_flag_code = Some(random_hex(6));
        invalid_aircraft_body_test(body, msg).await;
    }

    #[tokio::test]
    /// Unknown registration prefix
    async fn http_mod_patch_aircraft_invalid_registration() {
        start_server(Some(())).await;

        let msg = "invalid body unknown registration prefix";

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.registration = S!("XXXXXXX");
        invalid_aircraft_body_test(body, msg).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.registration = S!("G1");
        invalid_aircraft_body_test(body, msg).await;
    }

    #[tokio::test]
    /// Valid request & body, but env.update is None, 405 response
    async fn http_mod_patch_aircraft_env_not_set() {
        start_server(None).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("JP");
        body.registered_owner_country_name = S!("Japan");
        body.registered_owner_operator_flag_code = Some(S!("XXX"));
        body.registration = S!("JAXXX");

        let response =
            send_patch_request(&DEFAULT_AIRCRAFT_PATH, Some(VALID_AUTH_HEADER), Some(body)).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert!(response.text().await.unwrap().is_empty());
    }

    #[tokio::test]
    /// All aircraft details updated
    async fn http_mod_patch_aircraft_null_flag() {
        let state = start_server(Some(())).await;

        get_response_200::<AircraftAndRoute>(&DEFAULT_AIRCRAFT_PATH).await;
        let pre_cache = get_aircraft_cache(&state.redis).await;

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("JP");
        body.registered_owner_country_name = S!("Japan");
        body.registered_owner_operator_flag_code = None;
        body.registration = S!("JAXXX");

        let response = send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(VALID_AUTH_HEADER),
            Some(body.clone()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);

        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_AIRCRAFT_PATH).await;

        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());

        assert_eq!(response.aircraft.unwrap(), body);

        let post_cache = get_aircraft_cache(&state.redis).await;
        assert_ne!(pre_cache, post_cache);

        reset_aircraft().await;
    }

    #[tokio::test]
    /// All aircraft details updated, cache cleared
    async fn http_mod_patch_aircraft_ok() {
        let state = start_server(Some(())).await;

        get_response_200::<AircraftAndRoute>(&DEFAULT_AIRCRAFT_PATH).await;
        get_response_200::<AircraftAndRoute>(&format!(
            "aircraft/{}",
            DEFAULT_AIRCRAFT_BODY.registration
        ))
        .await;
        let pre_cache = get_aircraft_cache(&state.redis).await;

        assert!(cache_aircraft_exists(&state.redis).await);

        let mut body = DEFAULT_AIRCRAFT_BODY.clone();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("JP");
        body.registered_owner_country_name = S!("Japan");
        body.registered_owner_operator_flag_code = None;
        body.registration = S!("JAXXX");

        let response = send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(VALID_AUTH_HEADER),
            Some(body.clone()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!cache_aircraft_exists(&state.redis).await);

        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_AIRCRAFT_PATH).await;

        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());

        assert_eq!(response.aircraft.unwrap(), body);

        let post_cache = get_aircraft_cache(&state.redis).await;
        assert_ne!(pre_cache, post_cache);

        reset_aircraft().await;
    }

    #[tokio::test]
    /// All aircraft details updated, when using a country that has multiple registration prefixes
    async fn http_mod_patch_aircraft_ok_ireland() {
        start_server(Some(())).await;

        let mut body = gen_aircraft();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("IE");
        body.registered_owner_country_name = S!("Ireland");
        body.registered_owner_operator_flag_code = Some(S!("XXX"));
        body.registration = S!("EJXXX");

        let response = send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(VALID_AUTH_HEADER),
            Some(body.clone()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_AIRCRAFT_PATH).await;

        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());

        assert_eq!(response.aircraft.unwrap(), body);

        reset_aircraft().await;

        let mut body = gen_aircraft();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("IE");
        body.registered_owner_country_name = S!("Ireland");
        body.registered_owner_operator_flag_code = Some(S!("XXX"));
        body.registration = S!("EIXXX");

        let response = send_patch_request(
            &DEFAULT_AIRCRAFT_PATH,
            Some(VALID_AUTH_HEADER),
            Some(body.clone()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_AIRCRAFT_PATH).await;

        assert!(response.aircraft.is_some());
        assert!(response.flightroute.is_none());

        assert_eq!(response.aircraft.unwrap(), body);

        reset_aircraft().await;
    }

    //
    // Callsign Tests
    //
    #[tokio::test]
    /// env.update is None, 405 response
    async fn http_mod_patch_icao_callsign_no_update() {
        start_server(None).await;

        let response = send_patch_request::<()>(&DEFAULT_CALLSIGN_PATH, None, None).await;

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    /// No auth header, return 401
    async fn http_mod_patch_icao_callsign_no_header() {
        start_server(Some(())).await;

        let response = send_patch_request::<()>(&DEFAULT_CALLSIGN_PATH, None, None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    /// Invalid auth header, return 401
    async fn http_mod_patch_icao_callsign_invalid_header() {
        start_server(Some(())).await;
        let flightroute = FLIGHTROUTE.clone();

        let response = send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(("Authorization", "authorization")),
            Some(HashMap::from([
                ("origin", flightroute.origin.icao_code),
                ("destination", flightroute.destination.icao_code),
            ])),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    /// No body, return 400
    async fn http_mod_patch_icao_callsign_no_body() {
        start_server(Some(())).await;

        let response =
            send_patch_request::<()>(&DEFAULT_CALLSIGN_PATH, Some(VALID_AUTH_HEADER), None).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            "invalid body \"application/json\" header"
        );
    }

    #[tokio::test]
    /// Invalid body, return 401
    async fn http_mod_patch_icao_callsign_invalid_body() {
        start_server(Some(())).await;

        let body = HashMap::from([("EIDW", "EIDW"), ("destination", "EIDW")]);
        invalid_callsign_body_test(body, "invalid body origin").await;

        let body = HashMap::from([("origin", "EIDW"), ("EIDW", "EIDW")]);
        invalid_callsign_body_test(body, "invalid body destination").await
    }

    #[tokio::test]
    /// Unknown callsign, return 404
    async fn http_mod_patch_icao_callsign_unknown() {
        start_server(Some(())).await;

        let response = send_patch_request(
            &format!("callsign/{UNKNOWN_CALLSIGN_ICAO}"),
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([("origin", "EGLC"), ("destination", "EIDW")])),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            "unknown callsign"
        );
    }

    #[tokio::test]
    /// Invalid origin, return 404
    async fn http_mod_patch_icao_callsign_invalid_body_origin() {
        start_server(Some(())).await;

        let response = send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([("origin", "DHAM"), ("destination", "EGLL")])),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            "unknown airport: DHAM"
        );
    }

    #[tokio::test]
    /// Invalid destination, return 404
    async fn http_mod_patch_icao_callsign_invalid_body_destination() {
        start_server(Some(())).await;

        let response = send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([("origin", "EGLL"), ("destination", "DGLL")])),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            "unknown airport: DGLL"
        );
    }

    #[tokio::test]
    /// No change in origin & destination, return 401
    async fn http_mod_patch_icao_callsign_invalid_body_no_change() {
        start_server(Some(())).await;
        let flightroute = FLIGHTROUTE.clone();

        let response = send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([
                ("origin", flightroute.origin.icao_code),
                ("destination", flightroute.destination.icao_code),
            ])),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response
                .json::<ResponseJson<String>>()
                .await
                .unwrap()
                .response,
            "invalid body no change"
        );
    }

    #[tokio::test]
    /// Valid request & body, but env.update is None, 405 response
    async fn http_mod_patch_icao_callsign_env_not_set() {
        start_server(None).await;

        let flightroute = FLIGHTROUTE.clone();
        let response = send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([
                ("origin", flightroute.origin.icao_code),
                ("destination", flightroute.destination.icao_code),
            ])),
        )
        .await;

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert!(response.text().await.unwrap().is_empty(),);
    }

    #[tokio::test]
    /// Valid flightroute origin change, cache removed
    async fn http_mod_patch_icao_callsign_update_origin() {
        let setup = start_server(Some(())).await;
        reset_callsign().await;

        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_CALLSIGN_PATH).await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let response = get_response_200::<AircraftAndRoute>(&format!(
            "callsign/{}",
            FLIGHTROUTE.callsign_iata.as_ref().unwrap()
        ))
        .await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let pre_cache = get_callsign_cache(&setup.redis).await;

        // Montreal, Dublin
        let response = send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([
                ("origin", "EIDW"),
                ("destination", FLIGHTROUTE.origin.icao_code.as_str()),
            ])),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        assert!(!cache_callsign_exists(&setup.redis).await);

        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_CALLSIGN_PATH).await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let flightroute = response.flightroute.unwrap();
        assert_montreal(&flightroute.destination);
        assert_dublin(&flightroute.origin);

        let post_cache = get_callsign_cache(&setup.redis).await;

        assert_ne!(pre_cache, post_cache);

        assert_eq!(
            ResponseFlightRoute::from_model(Some(&post_cache)).unwrap(),
            flightroute
        );
        reset_callsign().await;
    }

    #[tokio::test]
    /// Valid flightroute destination change, cache removed
    async fn http_mod_patch_icao_callsign_update_destination() {
        let setup = start_server(Some(())).await;
        reset_callsign().await;

        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_CALLSIGN_PATH).await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let response = get_response_200::<AircraftAndRoute>(&format!(
            "callsign/{}",
            FLIGHTROUTE.callsign_iata.as_ref().unwrap()
        ))
        .await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let pre_cache = get_callsign_cache(&setup.redis).await;

        // Montreal, Dublin
        let response = send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([
                ("origin", FLIGHTROUTE.origin.icao_code.as_str()),
                ("destination", "EIDW"),
            ])),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        assert!(!cache_callsign_exists(&setup.redis).await);

        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_CALLSIGN_PATH).await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let flightroute = response.flightroute.unwrap();
        assert_montreal(&flightroute.origin);
        assert_dublin(&flightroute.destination);

        let post_cache = get_callsign_cache(&setup.redis).await;

        assert_ne!(pre_cache, post_cache);

        assert_eq!(
            ResponseFlightRoute::from_model(Some(&post_cache)).unwrap(),
            flightroute
        );
        reset_callsign().await;
    }

    #[tokio::test]
    /// Valid flightroute origin & destination change, cache removed
    async fn http_mod_patch_icao_callsign_update_origin_destination() {
        let setup = start_server(Some(())).await;
        reset_callsign().await;

        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_CALLSIGN_PATH).await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let response = get_response_200::<AircraftAndRoute>(&format!(
            "callsign/{}",
            FLIGHTROUTE.callsign_iata.as_ref().unwrap()
        ))
        .await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let pre_cache = get_callsign_cache(&setup.redis).await;

        let response = send_patch_request(
            &DEFAULT_CALLSIGN_PATH,
            Some(VALID_AUTH_HEADER),
            Some(HashMap::from([
                ("origin", FLIGHTROUTE.destination.icao_code.as_str()),
                ("destination", FLIGHTROUTE.origin.icao_code.as_str()),
            ])),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        assert!(!cache_callsign_exists(&setup.redis).await);

        let response = get_response_200::<AircraftAndRoute>(&DEFAULT_CALLSIGN_PATH).await;
        assert!(response.aircraft.is_none());
        assert!(response.flightroute.is_some());

        let flightroute = response.flightroute.unwrap();
        assert_san_jose(&flightroute.origin);
        assert_montreal(&flightroute.destination);

        let post_cache = get_callsign_cache(&setup.redis).await;

        assert_ne!(pre_cache, post_cache);

        assert_eq!(
            ResponseFlightRoute::from_model(Some(&post_cache)).unwrap(),
            flightroute
        );
        reset_callsign().await;
    }
}
