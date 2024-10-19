use std::{collections::HashMap, env};
use thiserror::Error;

type EnvHashMap = HashMap<String, String>;

#[derive(Debug, Error)]
enum EnvError {
    #[error("missing env: '{0}'")]
    NotFound(String),
}

#[derive(Debug, Clone)]
pub struct AppEnv {
    pub allow_scrape_flightroute: Option<()>,
    pub allow_scrape_photo: Option<()>,
    pub api_host: String,
    pub api_port: u16,
    pub location_logs: String,
    pub log_level: tracing::Level,
    pub pg_database: String,
    pub pg_host: String,
    pub pg_pass: String,
    pub pg_port: u16,
    pub pg_user: String,
    pub redis_database: u16,
    pub redis_host: String,
    pub redis_password: String,
    pub redis_port: u16,
    pub url_aircraft_photo: String,
    pub url_callsign: String,
    pub url_photo_prefix: String,
}

impl AppEnv {
    /// Parse "true" or "false" to bool, else false
    fn parse_boolean(key: &str, map: &EnvHashMap) -> bool {
        map.get(key).map_or(false, |value| value == "true")
    }

    /// Parse debug and/or trace into tracing level
    fn parse_log(map: &EnvHashMap) -> tracing::Level {
        if Self::parse_boolean("LOG_TRACE", map) {
            tracing::Level::TRACE
        } else if Self::parse_boolean("LOG_DEBUG", map) {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        }
    }

    /// Parse string to u32, else return 1
    fn parse_number(key: &str, map: &EnvHashMap) -> Result<u16, EnvError> {
        let default = 1;
        map.get(key).map_or_else(
            || Err(EnvError::NotFound(key.into())),
            |data| data.parse::<u16>().map_or(Ok(default), Ok),
        )
    }

    fn parse_string(key: &str, map: &EnvHashMap) -> Result<String, EnvError> {
        map.get(key).map_or_else(
            || Err(EnvError::NotFound(key.into())),
            |value| Ok(value.into()),
        )
    }

    fn parse_allow_scrape(key: &str, map: &EnvHashMap) -> Result<Option<()>, EnvError> {
        map.get(key).map_or_else(
            || Err(EnvError::NotFound(key.into())),
            |value| {
                if value.to_uppercase() == "TRUE" {
                    Ok(Some(()))
                } else {
                    Ok(None)
                }
            },
        )
    }

    /// Load, and parse .env file, return AppEnv
    fn generate() -> Result<Self, EnvError> {
        let env_map = env::vars()
            .map(|i| (i.0, i.1))
            .collect::<HashMap<String, String>>();

        Ok(Self {
            api_host: Self::parse_string("API_HOST", &env_map)?,
            api_port: Self::parse_number("API_PORT", &env_map)?,
            location_logs: Self::parse_string("LOCATION_LOGS", &env_map)?,
            log_level: Self::parse_log(&env_map),
            pg_database: Self::parse_string("PG_DATABASE", &env_map)?,
            pg_host: Self::parse_string("PG_HOST", &env_map)?,
            pg_pass: Self::parse_string("PG_PASS", &env_map)?,
            pg_port: Self::parse_number("PG_PORT", &env_map)?,
            pg_user: Self::parse_string("PG_USER", &env_map)?,
            redis_database: Self::parse_number("REDIS_DATABASE", &env_map)?,
            redis_host: Self::parse_string("REDIS_HOST", &env_map)?,
            redis_password: Self::parse_string("REDIS_PASSWORD", &env_map)?,
            redis_port: Self::parse_number("REDIS_PORT", &env_map)?,
            allow_scrape_flightroute: Self::parse_allow_scrape("SCRAPE_FLIGHTROUTE", &env_map)?,
            allow_scrape_photo: Self::parse_allow_scrape("SCRAPE_PHOTO", &env_map)?,
            url_aircraft_photo: Self::parse_string("URL_AIRCRAFT_PHOTO", &env_map)?,
            url_photo_prefix: Self::parse_string("URL_PHOTO_PREFIX", &env_map)?,
            url_callsign: Self::parse_string("URL_CALLSIGN", &env_map)?,
        })
    }

    pub fn get_env() -> Self {
        let local_env = ".env";
        let app_env = "/app_env/.api.env";

        let env_path = if std::fs::metadata(app_env).is_ok() {
            app_env
        } else if std::fs::metadata(local_env).is_ok() {
            local_env
        } else {
            println!("\n\x1b[31mUnable to load env file\x1b[0m\n");
            std::process::exit(1);
        };

        dotenvy::from_path(env_path).ok();
        match Self::generate() {
            Ok(s) => s,
            Err(e) => {
                println!("\n\x1b[31m{e}\x1b[0m\n");
                std::process::exit(1);
            }
        }
    }
}

/// Run tests with
///
/// cargo watch -q -c -w src/ -x 'test env_ -- --nocapture'
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::S;

    use super::*;

    #[test]
    fn env_missing_env() {
        let mut map = HashMap::new();
        map.insert(S!("not_fish"), S!("not_fish"));

        let result = AppEnv::parse_string("fish", &map);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "missing env: 'fish'");
    }

    #[test]
    fn env_parse_string_valid() {
        let mut map = HashMap::new();
        map.insert(S!("RANDOM_STRING"), S!("123"));

        let result = AppEnv::parse_string("RANDOM_STRING", &map).unwrap();

        assert_eq!(result, "123");

        let mut map = HashMap::new();
        map.insert(S!("RANDOM_STRING"), S!("hello_world"));

        let result = AppEnv::parse_string("RANDOM_STRING", &map).unwrap();

        assert_eq!(result, "hello_world");
    }

    #[test]
    fn env_parse_scrape_allow() {
        let mut map = HashMap::new();
        map.insert(S!("SCRAPE_PHOTO"), S!("true"));
        map.insert(S!("SCRAPE_FLIGHTROUTE"), S!("true"));

        let result01 = AppEnv::parse_allow_scrape("SCRAPE_PHOTO", &map);
        let result02 = AppEnv::parse_allow_scrape("SCRAPE_FLIGHTROUTE", &map);

        assert!(result01.is_ok());
        assert!(result01.unwrap().is_some());
        assert!(result02.is_ok());
        assert!(result02.unwrap().is_some());

        let mut map = HashMap::new();
        map.insert(S!("SCRAPE_PHOTO"), S!("false"));
        map.insert(S!("SCRAPE_FLIGHTROUTE"), S!("false"));

        let result01 = AppEnv::parse_allow_scrape("SCRAPE_PHOTO", &map);
        let result02 = AppEnv::parse_allow_scrape("SCRAPE_FLIGHTROUTE", &map);

        assert!(result01.is_ok());
        assert!(result01.unwrap().is_none());
        assert!(result02.is_ok());
        assert!(result02.unwrap().is_none());

        let mut map = HashMap::new();
        map.insert(S!("SCRAPE_PHOTO"), S!("tru"));
        map.insert(S!("SCRAPE_FLIGHTROUTE"), S!("tre"));

        let result01 = AppEnv::parse_allow_scrape("SCRAPE_PHOTO", &map);
        let result02 = AppEnv::parse_allow_scrape("SCRAPE_FLIGHTROUTE", &map);

        assert!(result01.is_ok());
        assert!(result01.unwrap().is_none());
        assert!(result02.is_ok());
        assert!(result02.unwrap().is_none());
    }

    #[test]
    fn env_parse_boolean_ok() {
        let mut map = HashMap::new();
        map.insert(S!("valid_true"), S!("true"));
        map.insert(S!("valid_false"), S!("false"));
        map.insert(S!("invalid_but_false"), S!("as"));

        let result01 = AppEnv::parse_boolean("valid_true", &map);
        let result02 = AppEnv::parse_boolean("valid_false", &map);
        let result03 = AppEnv::parse_boolean("invalid_but_false", &map);
        let result04 = AppEnv::parse_boolean("missing", &map);

        assert!(result01);
        assert!(!result02);
        assert!(!result03);
        assert!(!result04);
    }

    #[test]
    fn env_return_appenv() {
        dotenvy::dotenv().ok();

        let result = AppEnv::generate();

        assert!(result.is_ok());
    }

    #[test]
    fn env_parse_log_valid() {
        let map = HashMap::from([(S!("RANDOM_STRING"), S!("123"))]);

        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::INFO);

        let map = HashMap::from([(S!("LOG_DEBUG"), S!("false"))]);

        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::INFO);

        let map = HashMap::from([(S!("LOG_TRACE"), S!("false"))]);

        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::INFO);

        let map = HashMap::from([
            (S!("LOG_DEBUG"), S!("false")),
            (S!("LOG_TRACE"), S!("false")),
        ]);

        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::INFO);

        let map = HashMap::from([
            (S!("LOG_DEBUG"), S!("true")),
            (S!("LOG_TRACE"), S!("false")),
        ]);

        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::DEBUG);

        let map = HashMap::from([(S!("LOG_DEBUG"), S!("true")), (S!("LOG_TRACE"), S!("true"))]);

        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::TRACE);

        let map = HashMap::from([
            (S!("LOG_DEBUG"), S!("false")),
            (S!("LOG_TRACE"), S!("true")),
        ]);

        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::TRACE);
    }

    #[test]
    fn env_parse_number_ok() {
        let mut map = HashMap::new();
        map.insert(S!("U16_TEST"), S!("88"));

        let result = AppEnv::parse_number("U16_TEST", &map).unwrap();

        assert_eq!(result, 88);
    }

    #[test]
    fn env_parse_number_is_err() {
        let map = HashMap::new();

        let result = AppEnv::parse_number("U16_TEST", &map);

        assert!(result.is_err());
        match result.unwrap_err() {
            EnvError::NotFound(value) => assert_eq!(value, "U16_TEST"),
        }
    }
}
