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
    db_postgres::{ModelAircraft, ModelAirport, ModelFlightroute},
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
/// But it's better than nothing, the limits are the current max length  for each +10%
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

// At the moment this is only for mode_s, where the aircraft GET endpoint can search by registration as well
pub async fn aircraft_patch(
    State(state): State<ApplicationState>,
    mode_s: ModeS,
    IncomingJson(body): IncomingJson<ResponseAircraft>,
) -> Result<StatusCode, AppError> {
    let Some(known) = ModelAircraft::get(
        &state.postgres,
        &super::AircraftSearch::ModeS(mode_s),
        &state.url_prefix,
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
    if ResponseAircraft::from(known.clone()) == body {
        return Err(AppError::Body(S!("no change")));
    }

    // At the moment, don't allow update if the photo_url's or mode_s has been changed
    if known.url_photo != body.url_photo
        || known.url_photo_thumbnail != body.url_photo_thumbnail
        || known.mode_s != body.mode_s
    {
        return Err(AppError::Body(S!("immutable value changed")));
    }

    known.update(state.postgres, &body).await?;

    // Delete cache
    state
        .redis
        .del::<(), String>(format!("mode_s::{}", known.mode_s))
        .await?;
    state
        .redis
        .del::<(), String>(format!("registration::{}", known.registration))
        .await?;
    state
        .redis
        .del::<(), String>(format!("registration::{}", body.registration))
        .await?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
#[allow(clippy::pedantic, clippy::unwrap_used)]
pub mod tests {
    use std::collections::HashMap;

    use super::*;

    use crate::S;
    use crate::api::get_api_version;
    use crate::api::serve;
    use crate::db_postgres;
    use crate::db_redis;
    use crate::parse_env;
    use crate::parse_env::AppEnv;
    use crate::sleep;

    use fred::{
        interfaces::ClientLike,
        prelude::{HashesInterface, Pool},
    };
    use reqwest::{Client, StatusCode};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use sqlx::PgPool;
    use tokio::task::JoinHandle;

    pub struct TestSetup {
        pub _handle: Option<JoinHandle<()>>,
        pub app_env: AppEnv,
        pub postgres: PgPool,
        pub redis: Pool,
    }

    // Get basic api params, also flushes all redis keys
    pub async fn test_setup() -> TestSetup {
        let app_env = parse_env::AppEnv::get_env();
        let postgres = db_postgres::get_pool(&app_env).await.unwrap();
        let redis = db_redis::get_pool(&app_env).await.unwrap();
        redis.flushall::<()>(true).await.unwrap();
        TestSetup {
            _handle: None,
            app_env,
            postgres,
            redis,
        }
    }

    const AIRCRAFT: &str = "8880E1";
    const CALLSIGN: &str = "CFE37E";

    fn callsign_url() -> String {
        format!(
            "http://127.0.0.1:8282{}/callsign/{CALLSIGN}",
            get_api_version(),
        )
    }

    fn aircraft_url() -> String {
        format!(
            "http://127.0.0.1:8282{}/aircraft/{AIRCRAFT}",
            get_api_version()
        )
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
        let handle = tokio::spawn(async move {
            serve(spawn_env, postgres, redis).await.unwrap();
        });
        // just sleep to make sure the server is running - 1ms is enough
        sleep!(1);
        TestSetup {
            _handle: Some(handle),
            app_env,
            postgres: setup.postgres,
            redis: setup.redis,
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestResponse {
        response: Value,
    }

    //
    // Aircraft Tests
    //

    /// Test based on this pre-known aircraft
    fn gen_aircraft() -> ResponseAircraft {
        ResponseAircraft {
            aircraft_type: S!("787 9"),
            icao_type: S!("B789"),
            manufacturer: S!("Boeing"),
            mode_s: S!("8880E1"),
            registration: S!("VN-A863"),
            registered_owner_country_iso_name: S!("VN"),
            registered_owner_country_name: S!("Vietnam"),
            registered_owner_operator_flag_code: Some(S!("HVN")),
            registered_owner: S!("Vietnam Airlines"),
            url_photo: Some(S!(
                "https://airport-data.com/images/aircraft/001/675/001675893.jpg"
            )),
            url_photo_thumbnail: Some(S!(
                "https://airport-data.com/images/aircraft/thumbnails/001/675/001675893.jpg"
            )),
        }
    }

    /// Reset aircraft details back to original
    async fn reset_aircraft(client: &Client) {
        let aircraft = gen_aircraft();
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&aircraft)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    /// env.update is None, 405 response
    async fn http_mod_patch_aircraft_no_update() {
        start_server(None).await;
        let client = reqwest::Client::new();
        let resp = client.patch(aircraft_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    /// No auth header, return 401
    async fn http_mod_patch_aircraft_no_header() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client.patch(aircraft_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    /// invalid auth header, return 401
    async fn http_mod_patch_aircraft_invalid_header() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "invalid_header")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    /// No body, return 401
    async fn http_mod_patch_aircraft_no_body() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    /// Unknown aircraft, return 404
    async fn http_mod_patch_aircraft_unknown() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(format!(
                "http://127.0.0.1:8282{}/aircraft/101010",
                get_api_version(),
            ))
            .header("authorization", "password123")
            .json(&gen_aircraft())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "unknown aircraft"
        );
    }

    #[tokio::test]
    /// Invalid body, return 401
    async fn http_mod_patch_aircraft_invalid_body() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&HashMap::from([("thing", "other")]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    /// No change to data, return 400
    async fn http_mod_patch_aircraft_no_change() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&gen_aircraft())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body no change"
        );
    }

    #[tokio::test]
    /// Changes to either photo url's or mode_s will result in a 400 error
    async fn http_mod_patch_aircraft_invalid_body_url_mode_s() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();

        // Change of mode_s
        let mut body = gen_aircraft();
        body.mode_s = S!("AAAAAA");
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body immutable value changed"
        );

        // change of url_photo
        let mut body = gen_aircraft();
        body.url_photo = Some(S!("/any/url/here"));
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body immutable value changed"
        );

        let mut body = gen_aircraft();
        body.url_photo = None;
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body immutable value changed"
        );

        // change of url_photo_thumbnail
        let mut body = gen_aircraft();
        body.url_photo_thumbnail = Some(S!("/any/url/here"));
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body immutable value changed"
        );

        let mut body = gen_aircraft();
        body.url_photo_thumbnail = None;
        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body immutable value changed"
        );
    }

    #[tokio::test]
    /// Unknown country returns a 400 error
    async fn http_mod_patch_aircraft_invalid_body_country() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();

        let mut body = gen_aircraft();
        body.registered_owner_country_name = S!("Unknown");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body unknown country"
        );

        let mut body = gen_aircraft();
        body.registered_owner_country_iso_name = S!("XX");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body unknown country"
        );
    }
    #[tokio::test]
    /// New values rejected if too long
    async fn http_mod_patch_aircraft_invalid_lengths() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();

        async fn test(client: &Client, body: ResponseAircraft) {
            let resp = client
                .patch(aircraft_url())
                .header("authorization", "password123")
                .json(&body)
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                resp.json::<TestResponse>().await.unwrap().response,
                "invalid body value too long"
            );
        }

        let mut body = gen_aircraft();
        body.aircraft_type = S!("X").repeat(66);
        test(&client, body).await;

        let mut body = gen_aircraft();
        body.icao_type = S!("X").repeat(7);
        test(&client, body).await;

        let mut body = gen_aircraft();
        body.registration = S!("X").repeat(15);
        test(&client, body).await;

        let mut body = gen_aircraft();
        body.manufacturer = S!("X").repeat(81);
        test(&client, body).await;

        let mut body = gen_aircraft();
        body.registered_owner = S!("X").repeat(122);
        test(&client, body).await;

        let mut body = gen_aircraft();
        body.registered_owner_operator_flag_code = Some(S!("X").repeat(6));
        test(&client, body).await;
    }

    #[tokio::test]
    /// Unknown registration prefix
    async fn http_mod_patch_aircraft_invalid_registration() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();

        let mut body = gen_aircraft();
        body.registration = S!("XXXXXXX");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body unknown registration prefix"
        );

        let mut body = gen_aircraft();
        body.registration = S!("G1");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body unknown registration prefix"
        );
    }

    #[tokio::test]
    /// Valid request & body, but env.update is None, 405 response
    async fn http_mod_patch_aircraft_env_not_set() {
        start_server(None).await;
        let client = reqwest::Client::new();

        let mut body = gen_aircraft();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("JP");
        body.registered_owner_country_name = S!("Japan");
        body.registered_owner_operator_flag_code = Some(S!("XXX"));
        body.registration = S!("JAXXX");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    /// All aircraft details updated, when flag_code is null, cache cleared
    async fn http_mod_patch_aircraft_null_flag() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();

        let mut body = gen_aircraft();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("JP");
        body.registered_owner_country_name = S!("Japan");
        body.registered_owner_operator_flag_code = None;
        body.registration = S!("JAXXX");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = client.get(aircraft_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap();

        assert_eq!(result.response["aircraft"]["type"], "XXX");
        assert_eq!(result.response["aircraft"]["icao_type"], "XXX");
        assert_eq!(result.response["aircraft"]["manufacturer"], "XXX");
        assert_eq!(result.response["aircraft"]["registered_owner"], "XXX");
        assert_eq!(
            result.response["aircraft"]["registered_owner_country_iso_name"],
            "JP"
        );
        assert_eq!(
            result.response["aircraft"]["registered_owner_country_name"],
            "Japan"
        );
        assert_eq!(
            result.response["aircraft"]["registered_owner_operator_flag_code"],
            Value::Null
        );
        assert_eq!(result.response["aircraft"]["registration"], "JAXXX");

        reset_aircraft(&client).await;
    }

    #[tokio::test]
    /// All aircraft details updated, cache cleared
    async fn http_mod_patch_aircraft_ok() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();

        let mut body = gen_aircraft();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("JP");
        body.registered_owner_country_name = S!("Japan");
        body.registered_owner_operator_flag_code = Some(S!("XXX"));
        body.registration = S!("JAXXX");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = client.get(aircraft_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap();

        assert_eq!(result.response["aircraft"]["type"], "XXX");
        assert_eq!(result.response["aircraft"]["icao_type"], "XXX");
        assert_eq!(result.response["aircraft"]["manufacturer"], "XXX");
        assert_eq!(result.response["aircraft"]["registered_owner"], "XXX");
        assert_eq!(
            result.response["aircraft"]["registered_owner_country_iso_name"],
            "JP"
        );
        assert_eq!(
            result.response["aircraft"]["registered_owner_country_name"],
            "Japan"
        );
        assert_eq!(
            result.response["aircraft"]["registered_owner_operator_flag_code"],
            "XXX"
        );
        assert_eq!(result.response["aircraft"]["registration"], "JAXXX");

        reset_aircraft(&client).await;
    }

    #[tokio::test]
    /// All aircraft details updated, cache cleared, when using a country that has multiple registration prefixes
    async fn http_mod_patch_aircraft_ok_ireland() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();

        let mut body = gen_aircraft();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("IE");
        body.registered_owner_country_name = S!("Ireland");
        body.registered_owner_operator_flag_code = Some(S!("XXX"));
        body.registration = S!("EJXXX");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = client.get(aircraft_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap();

        assert_eq!(result.response["aircraft"]["type"], "XXX");
        assert_eq!(result.response["aircraft"]["icao_type"], "XXX");
        assert_eq!(result.response["aircraft"]["manufacturer"], "XXX");
        assert_eq!(result.response["aircraft"]["registered_owner"], "XXX");
        assert_eq!(
            result.response["aircraft"]["registered_owner_country_iso_name"],
            "IE"
        );
        assert_eq!(
            result.response["aircraft"]["registered_owner_country_name"],
            "Ireland"
        );
        assert_eq!(
            result.response["aircraft"]["registered_owner_operator_flag_code"],
            "XXX"
        );
        assert_eq!(result.response["aircraft"]["registration"], "EJXXX");

        reset_aircraft(&client).await;

        let mut body = gen_aircraft();
        body.aircraft_type = S!("XXX");
        body.icao_type = S!("XXX");
        body.manufacturer = S!("XXX");
        body.registered_owner = S!("XXX");
        body.registered_owner_country_iso_name = S!("IE");
        body.registered_owner_country_name = S!("Ireland");
        body.registered_owner_operator_flag_code = Some(S!("XXX"));
        body.registration = S!("EIXXX");

        let resp = client
            .patch(aircraft_url())
            .header("authorization", "password123")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = client.get(aircraft_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result = resp.json::<TestResponse>().await.unwrap();

        assert_eq!(result.response["aircraft"]["type"], "XXX");
        assert_eq!(result.response["aircraft"]["icao_type"], "XXX");
        assert_eq!(result.response["aircraft"]["manufacturer"], "XXX");
        assert_eq!(result.response["aircraft"]["registered_owner"], "XXX");
        assert_eq!(
            result.response["aircraft"]["registered_owner_country_iso_name"],
            "IE"
        );
        assert_eq!(
            result.response["aircraft"]["registered_owner_country_name"],
            "Ireland"
        );
        assert_eq!(
            result.response["aircraft"]["registered_owner_operator_flag_code"],
            "XXX"
        );
        assert_eq!(result.response["aircraft"]["registration"], "EIXXX");

        reset_aircraft(&client).await;
    }

    //
    // Callsign Tests
    //

    fn assert_london(result: &Value) {
        assert_eq!(result["country_name"], "United Kingdom");
        assert_eq!(result["elevation"], 19);
        assert_eq!(result["country_iso_name"], "GB");
        assert_eq!(result["iata_code"], "LCY");
        assert_eq!(result["icao_code"], "EGLC");
        assert_eq!(result["latitude"], 51.505299);
        assert_eq!(result["longitude"], 0.055278);
        assert_eq!(result["municipality"], "London");
        assert_eq!(result["name"], "London City Airport");
    }

    fn assert_dublin(result: &Value) {
        assert_eq!(result["country_iso_name"], "IE");
        assert_eq!(result["country_name"], "Ireland");
        assert_eq!(result["elevation"], 242);
        assert_eq!(result["iata_code"], "DUB");
        assert_eq!(result["icao_code"], "EIDW");
        assert_eq!(result["latitude"], 53.421299);
        assert_eq!(result["longitude"], -6.27007);
        assert_eq!(result["municipality"], "Dublin");
        assert_eq!(result["name"], "Dublin Airport");
    }

    fn assert_frankfurt(result: &Value) {
        assert_eq!(result["country_iso_name"], "DE");
        assert_eq!(result["country_name"], "Germany");
        assert_eq!(result["elevation"], 364);
        assert_eq!(result["iata_code"], "FRA");
        assert_eq!(result["icao_code"], "EDDF");
        assert_eq!(result["latitude"], 50.033333);
        assert_eq!(result["longitude"], 8.570556);
        assert_eq!(result["municipality"], "Frankfurt am Main");
        assert_eq!(result["name"], "Frankfurt am Main Airport");
    }

    fn assert_indianapolis(result: &Value) {
        assert_eq!(result["country_iso_name"], "US");
        assert_eq!(result["country_name"], "United States");
        assert_eq!(result["elevation"], 797);
        assert_eq!(result["iata_code"], "IND");
        assert_eq!(result["icao_code"], "KIND");
        assert_eq!(result["latitude"], 39.7173);
        assert_eq!(result["longitude"], -86.294403);
        assert_eq!(result["municipality"], "Indianapolis");
        assert_eq!(result["name"], "Indianapolis International Airport");
    }

    fn assert_original_callsign(result: &Value) {
        assert_eq!(result["callsign"], CALLSIGN);
        assert_eq!(result["callsign_icao"], CALLSIGN);
        assert_eq!(result["callsign_iata"], "CJ37E".to_uppercase());
        assert_london(&result["origin"]);
        assert!(result.get("midpoint").is_none());
        assert_dublin(&result["destination"]);
    }

    fn assert_updated_callsign_destination(result: &Value) {
        assert_eq!(result["callsign"], CALLSIGN);
        assert_eq!(result["callsign_icao"], CALLSIGN);
        assert_eq!(result["callsign_iata"], "CJ37E".to_uppercase());
        assert_london(&result["origin"]);
        assert!(result.get("midpoint").is_none());
        assert_frankfurt(&result["destination"]);
    }

    fn assert_updated_callsign_origin(result: &Value) {
        assert_eq!(result["callsign"], CALLSIGN);
        assert_eq!(result["callsign_icao"], CALLSIGN);
        assert_eq!(result["callsign_iata"], "CJ37E".to_uppercase());
        assert_frankfurt(&result["origin"]);
        assert!(result.get("midpoint").is_none());
        assert_dublin(&result["destination"]);
    }

    fn assert_updated_callsign_origin_and_destination(result: &Value) {
        assert_eq!(result["callsign"], CALLSIGN);
        assert_eq!(result["callsign_icao"], CALLSIGN);
        assert_eq!(result["callsign_iata"], "CJ37E".to_uppercase());
        assert_indianapolis(&result["origin"]);
        assert!(result.get("midpoint").is_none());
        assert_frankfurt(&result["destination"]);
    }

    /// Reset callsign back to original
    async fn reset_callsign(client: &Client) {
        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "EGLC"),
                ("destination", "EIDW"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    /// env.update is None, 405 response
    async fn http_mod_patch_icao_callsign_no_update() {
        start_server(None).await;
        let client = reqwest::Client::new();
        let resp = client.patch(callsign_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    /// No auth header, return 401
    async fn http_mod_patch_icao_callsign_no_header() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client.patch(callsign_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    /// Invalid auth header, return 401
    async fn http_mod_patch_icao_callsign_invalid_header() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(callsign_url())
            .header("authorization", "invalid_header")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    /// No body, return 401
    async fn http_mod_patch_icao_callsign_no_body() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    /// Invalid body, return 401
    async fn http_mod_patch_icao_callsign_invalid_body() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([("thing", "other")]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    /// Unknown callsign, return 404
    async fn http_mod_patch_icao_callsign_unknown() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(format!(
                "http://127.0.0.1:8282{}/callsign/ZZ0909",
                get_api_version(),
            ))
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "EGLC"),
                ("destination", "EIDW"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "unknown callsign"
        );
    }

    #[tokio::test]
    /// Invalid origin, return 404
    async fn http_mod_patch_icao_callsign_invalid_body_origin() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "DHAM"),
                ("destination", "EGLL"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "unknown airport: DHAM"
        );
    }

    #[tokio::test]
    /// Invalid destination, return 404
    async fn http_mod_patch_icao_callsign_invalid_body_destination() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "EHAM"),
                ("destination", "DGLL"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "unknown airport: DGLL"
        );
    }

    #[tokio::test]
    /// No change in origin & destination, return 401
    async fn http_mod_patch_icao_callsign_invalid_body_no_change() {
        start_server(Some(())).await;
        let client = reqwest::Client::new();
        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "EGLC"),
                ("destination", "EIDW"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            resp.json::<TestResponse>().await.unwrap().response,
            "invalid body no change"
        );
    }

    #[tokio::test]
    /// Valid request & body, but env.update is None, 405 response
    async fn http_mod_patch_icao_callsign_env_not_set() {
        start_server(None).await;
        let client = reqwest::Client::new();

        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "EDDF"),
                ("destination", "EIDW"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    /// Valid flightroute origin change, cache removed
    async fn http_mod_patch_icao_callsign_update_origin() {
        let setup = start_server(Some(())).await;
        let client = reqwest::Client::new();

        let resp = client.get(callsign_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp.json::<TestResponse>().await.unwrap().response;
        assert_original_callsign(resp.get("flightroute").unwrap());
        let original_cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(original_cache_icao.is_some());

        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "EDDF"),
                ("destination", "EIDW"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(cache_icao.is_none());

        let resp = client.get(callsign_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp.json::<TestResponse>().await.unwrap().response;

        assert_updated_callsign_origin(resp.get("flightroute").unwrap());
        let updated_cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(updated_cache_icao.is_some());
        assert_ne!(original_cache_icao, updated_cache_icao);

        reset_callsign(&client).await;
    }

    #[tokio::test]
    /// Valid flightroute destination change, cache removed
    async fn http_mod_patch_icao_callsign_update_destination() {
        let setup = start_server(Some(())).await;
        let client = reqwest::Client::new();

        let resp = client.get(callsign_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp.json::<TestResponse>().await.unwrap().response;
        assert_original_callsign(resp.get("flightroute").unwrap());
        let original_cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(original_cache_icao.is_some());

        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "EGLC"),
                ("destination", "EDDF"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(cache_icao.is_none());

        let resp = client.get(callsign_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp.json::<TestResponse>().await.unwrap().response;

        assert_updated_callsign_destination(resp.get("flightroute").unwrap());
        let updated_cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(updated_cache_icao.is_some());
        assert_ne!(original_cache_icao, updated_cache_icao);

        reset_callsign(&client).await;
    }

    #[tokio::test]
    /// Valid flightroute origin & destination change, cache removed
    async fn http_mod_patch_icao_callsign_update_origin_and_destination() {
        let setup = start_server(Some(())).await;
        let client = reqwest::Client::new();

        let resp = client.get(callsign_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp.json::<TestResponse>().await.unwrap().response;
        assert_original_callsign(resp.get("flightroute").unwrap());
        let original_cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(original_cache_icao.is_some());

        let resp = client
            .patch(callsign_url())
            .header("authorization", "password123")
            .json(&HashMap::from([
                ("origin", "KIND"),
                ("destination", "EDDF"),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(cache_icao.is_none());

        let resp = client.get(callsign_url()).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp.json::<TestResponse>().await.unwrap().response;

        assert_updated_callsign_origin_and_destination(resp.get("flightroute").unwrap());
        let updated_cache_icao = setup
            .redis
            .hget::<Option<String>, String, &str>(format!("callsign::{CALLSIGN}"), "data")
            .await
            .unwrap();
        assert!(updated_cache_icao.is_some());
        assert_ne!(original_cache_icao, updated_cache_icao);

        reset_callsign(&client).await;
    }
}
