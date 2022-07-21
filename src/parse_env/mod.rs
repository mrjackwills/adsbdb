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
    pub api_host: String,
    pub api_port: u16,
    pub debug: bool,
    pub location_logs: String,
    pub domain: String,
    pub pg_database: String,
    pub pg_host: String,
    pub pg_pass: String,
    pub pg_port: u16,
    pub pg_user: String,
    pub redis_database: u16,
    pub redis_host: String,
    pub redis_password: String,
    pub redis_port: u16,
    pub trace: bool,
    pub url_aircraft_photo: String,
    pub url_photo_prefix: String,
    pub url_callsign: String,
    pub www_domain: String,
}

impl AppEnv {
    /// Parse "true" or "false" to bool, else false
    fn parse_boolean(key: &str, map: &EnvHashMap) -> bool {
        match map.get(key) {
            Some(value) => value == "true",
            None => false,
        }
    }

    fn parse_string(key: &str, map: &EnvHashMap) -> Result<String, EnvError> {
        match map.get(key) {
            Some(value) => Ok(value.into()),
            None => Err(EnvError::NotFound(key.into())),
        }
    }

    /// Parse string to u32, else return 1
    fn parse_number(key: &str, map: &EnvHashMap) -> Result<u16, EnvError> {
        let default = 1;
        if let Some(data) = map.get(key) {
            match data.parse::<u16>() {
                Ok(d) => Ok(d),
                Err(_) => Ok(default),
            }
        } else {
            Err(EnvError::NotFound(key.into()))
        }
    }

    /// Load, and parse .env file, return AppEnv
    fn generate() -> Result<Self, EnvError> {
        let env_map = env::vars()
            .into_iter()
            .map(|i| (i.0, i.1))
            .collect::<HashMap<String, String>>();

        Ok(Self {
            api_host: Self::parse_string("API_HOST", &env_map)?,
            api_port: Self::parse_number("API_PORT", &env_map)?,
            debug: Self::parse_boolean("DEBUG", &env_map),
            domain: Self::parse_string("API_HOST", &env_map)?,
            location_logs: Self::parse_string("LOCATION_LOGS", &env_map)?,
            pg_database: Self::parse_string("PG_DATABASE", &env_map)?,
            pg_host: Self::parse_string("PG_HOST", &env_map)?,
            pg_pass: Self::parse_string("PG_PASS", &env_map)?,
            pg_port: Self::parse_number("PG_PORT", &env_map)?,
            pg_user: Self::parse_string("PG_USER", &env_map)?,
            redis_database: Self::parse_number("REDIS_DATABASE", &env_map)?,
            redis_host: Self::parse_string("REDIS_HOST", &env_map)?,
            redis_password: Self::parse_string("REDIS_PASSWORD", &env_map)?,
            redis_port: Self::parse_number("REDIS_PORT", &env_map)?,
            trace: Self::parse_boolean("TRACE", &env_map),
            url_aircraft_photo: Self::parse_string("URL_AIRCRAFT_PHOTO", &env_map)?,
            url_photo_prefix: Self::parse_string("URL_PHOTO_PREFIX", &env_map)?,
            url_callsign: Self::parse_string("URL_CALLSIGN", &env_map)?,
            www_domain: Self::parse_string("WWW_DOMAIN", &env_map)?,
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
            panic!("Unable to load env file")
        };

        dotenv::from_path(env_path).ok();
        match Self::generate() {
            Ok(s) => s,
            Err(e) => {
                println!("\n\x1b[31m{}\x1b[0m\n", e);
                std::process::exit(1);
            }
        }
    }
}

/// Run tests with
///
/// cargo watch -q -c -w src/ -x 'test env_ -- --nocapture'
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_missing_env() {
        let mut map = HashMap::new();
        map.insert("not_fish".to_owned(), "not_fish".to_owned());
        // ACTION
        let result = AppEnv::parse_string("fish", &map);

        // CHECK
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "missing env: 'fish'");
    }

    #[test]
    fn env_parse_string_valid() {
        // FIXTURES
        let mut map = HashMap::new();
        map.insert("RANDOM_STRING".to_owned(), "123".to_owned());

        // ACTION
        let result = AppEnv::parse_string("RANDOM_STRING", &map).unwrap();

        // CHECK
        assert_eq!(result, "123");

        // FIXTURES
        let mut map = HashMap::new();
        map.insert("RANDOM_STRING".to_owned(), "hello_world".to_owned());

        // ACTION
        let result = AppEnv::parse_string("RANDOM_STRING", &map).unwrap();

        // CHECK
        assert_eq!(result, "hello_world");
    }

    #[test]
    fn env_parse_boolean_ok() {
        // FIXTURES
        let mut map = HashMap::new();
        map.insert("valid_true".to_owned(), "true".to_owned());
        map.insert("valid_false".to_owned(), "false".to_owned());
        map.insert("invalid_but_false".to_owned(), "as".to_owned());

        // ACTION
        let result01 = AppEnv::parse_boolean("valid_true", &map);
        let result02 = AppEnv::parse_boolean("valid_false", &map);
        let result03 = AppEnv::parse_boolean("invalid_but_false", &map);
        let result04 = AppEnv::parse_boolean("missing", &map);

        // CHECK
        assert!(result01);
        assert!(!result02);
        assert!(!result03);
        assert!(!result04);
    }

    #[test]
    fn env_return_appenv() {
        dotenv::dotenv().ok();

        // ACTION
        let result = AppEnv::generate();

        assert!(result.is_ok());
    }

    #[test]
    fn env_parse_number_ok() {
        // FIXTURES
        let mut map = HashMap::new();
        map.insert("U16_TEST".to_owned(), "88".to_owned());

        // ACTION
        let result = AppEnv::parse_number("U16_TEST", &map).unwrap();

        // CHECK
        assert_eq!(result, 88);
    }

    #[test]
    fn env_parse_number_is_err() {
        // FIXTURES
        let map = HashMap::new();

        // ACTION
        let result = AppEnv::parse_number("U16_TEST", &map);

        assert!(result.is_err());
        match result.unwrap_err() {
            EnvError::NotFound(value) => assert_eq!(value, "U16_TEST"),
        }
    }
}
