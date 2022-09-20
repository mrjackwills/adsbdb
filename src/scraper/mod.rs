use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;
#[cfg(not(test))]
use tracing::error;

use crate::{
    api::AppError,
    db_postgres::{Model, ModelAircraft, ModelAirport, ModelFlightroute},
    parse_env::AppEnv,
};

const ICAO: &str = "\"icao\":";

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct PhotoData {
    #[serde(deserialize_with = "deserialize_url")]
    pub image: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
struct PhotoResponse {
    status: u16,
    count: Option<u16>,
    data: Option<[PhotoData; 1]>,
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Scrapper {
    flight_scrape_url: String,
    photo_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrapedFlightroute {
    pub callsign: String,
    pub origin: String,
    pub destination: String,
}

fn deserialize_url<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let url = String::deserialize(deserializer)?;
    if url.len() > 56 {
        Ok(url[56..].to_owned())
    } else {
        Err(serde::de::Error::custom("invalid_photo_url"))
    }
}

impl Scrapper {
    pub fn new(app_env: &AppEnv) -> Self {
        Self {
            flight_scrape_url: app_env.url_callsign.clone(),
            photo_url: app_env.url_aircraft_photo.clone(),
        }
    }

    // Make sure that input is a valid callsignstring, validitiy is [a-z]{4-8}
    // Should accept str or string as input?
    fn validate_icao(input: &str) -> Option<String> {
        let valid = input.len() == 4
            && input
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='z').contains(&c.to_ascii_lowercase()));
        if valid {
            Some(input.to_uppercase())
        } else {
            None
        }
    }

    /// Search an html file for "icao":", take the next 4 chars, and see if they match the icao spec ([a-z]{4})
    /// Will only return a Option<Vec>, where the Vec has a length of 2
    fn extract_icao_codes(html: &str, callsign: &str) -> Option<ScrapedFlightroute> {
        let output: Vec<_> = html
            .match_indices(ICAO)
            .filter_map(|i| {
                let icao_code = &html.split_at(i.0 + 8).1.chars().take(4).collect::<String>();
                Self::validate_icao(icao_code)
            })
            .collect::<Vec<_>>();
        if output.len() >= 2 {
            Some(ScrapedFlightroute {
                callsign: callsign.to_owned(),
                origin: output[0].clone(),
                destination: output[1].clone(),
            })
        } else {
            None
        }
    }

    /// Return true if BOTH airport_icao_code's are in db
    async fn check_icao_in_db(
        db: &PgPool,
        scraped_flightroute: &ScrapedFlightroute,
    ) -> Result<bool, AppError> {
        let (start, end) = tokio::try_join!(
            ModelAirport::get(db, &scraped_flightroute.origin),
            ModelAirport::get(db, &scraped_flightroute.destination)
        )?;
        Ok(start.is_some() && end.is_some())
    }

    /// Scrape callsign url for whole page html string
    #[cfg(not(test))]
    async fn request_callsign(&self, callsign: &str) -> Result<String, AppError> {
        let url = format!("{}/{}", self.flight_scrape_url, callsign);
        match reqwest::get(url).await {
            Ok(response) => match response.text().await {
                Ok(text) => Ok(text),
                Err(e) => {
                    error!(%e);
                    error!("can't transform into text");
                    Err(AppError::UnknownInDb("callsign"))
                }
            },
            Err(e) => {
                error!(%e);
                error!("can't scrape address");
                Err(AppError::UnknownInDb("callsign"))
            }
        }
    }

    // As above, but just return the test_scrape, instead of hitting a third party site
    #[cfg(test)]
    async fn request_callsign(&self, callsign: &str) -> Result<String, AppError> {
        if callsign == "ANA460" {
            Ok(include_str!("./test_scrape.txt").to_owned())
        } else {
            Ok(String::new())
        }
    }

    /// Request for photo from third party site
    #[cfg(not(test))]
    async fn request_photo(&self, aircraft: &ModelAircraft) -> Option<PhotoResponse> {
        let url = format!("{}ac_thumb.json?m={}&n=1", self.photo_url, aircraft.mode_s);
        match reqwest::get(url).await {
            Ok(response) => match response.json::<PhotoResponse>().await {
                Ok(photo) => {
                    if photo.data.is_some() {
                        Some(photo)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    error!(%e);
                    error!("can't transform into json");
                    None
                }
            },
            Err(e) => {
                error!(%e);
                error!("can't scrape address");
                None
            }
        }
    }

    /// Scrape photo for testings
    /// don't throw error as an internal process, but need to improve logging
    #[cfg(test)]
    async fn request_photo(&self, aircraft: &ModelAircraft) -> Option<PhotoResponse> {
        match aircraft.mode_s.as_str() {
            "393C00" => Some(PhotoResponse {
                status: 200,
                count: Some(1),
                data: Some([PhotoData {
                    image: "001/001/example.jpg".to_owned(),
                }]),
            }),
            _ => None,
        }
    }

    // Attempt to get photol url, and also insert into db
    pub async fn scrape_photo(
        &self,
        db: &PgPool,
        aircraft: &ModelAircraft,
    ) -> Result<(), AppError> {
        if let Some(photo) = self.request_photo(aircraft).await {
            if let Some([data_0, ..]) = photo.data.as_ref() {
                aircraft.insert_photo(db, data_0).await?;
            }
        }
        Ok(())
    }

    /// Scrape third party site for a flightroute, and try to insert into db
    pub async fn scrape_flightroute(
        &self,
        db: &PgPool,
        callsign: &str,
    ) -> Result<Option<ModelFlightroute>, AppError> {
        let mut output = None;
        let html = self.request_callsign(callsign).await?;
        if let Some(scraped_flightroute) = Self::extract_icao_codes(&html, callsign) {
            if Self::check_icao_in_db(db, &scraped_flightroute).await? {
                ModelFlightroute::insert_scraped_flightroute(db, scraped_flightroute).await?;
                output = ModelFlightroute::get(db, callsign).await.unwrap_or(None);
            }
        }
        Ok(output)
    }
}

/// Run tests with
///
/// cargo watch -q -c -w src/ -x 'test scraper_ '
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::api::ModeS;
    use crate::{db_postgres, db_redis};
    use serde::de::value::{Error as ValueError, StringDeserializer};
    use serde::de::IntoDeserializer;

    const TEST_CALLSIGN: &str = "ANA460";
    const TEST_ORIGIN: &str = "ROAH";
    const TEST_DESTINATION: &str = "RJTT";
    const TEST_MODE_S: &str = "393C00";

    async fn setup() -> (AppEnv, PgPool) {
        let app_env = AppEnv::get_env();
        let db = db_postgres::db_pool(&app_env).await.unwrap();
        (app_env, db)
    }

    async fn remove_scraped_data(db: &PgPool) {
        let query = "DELETE FROM flightroute WHERE flightroute_callsign_id = (SELECT flightroute_callsign_id FROM flightroute_callsign WHERE callsign = $1)";
        sqlx::query(query)
            .bind(TEST_CALLSIGN)
            .execute(db)
            .await
            .unwrap();
        let query = "DELETE FROM flightroute_callsign WHERE callsign = $1";
        sqlx::query(query)
            .bind(TEST_CALLSIGN)
            .execute(db)
            .await
            .unwrap();
        let query = r#"
		UPDATE aircraft SET aircraft_photo_id = NULL WHERE aircraft_id = (
			SELECT
				aa.aircraft_id
			FROM
				aircraft aa
			JOIN
				aircraft_mode_s ams
			ON
				aa.aircraft_mode_s_id = ams.aircraft_mode_s_id
			WHERE
				ams.mode_s = $1)"#;

        sqlx::query(query)
            .bind(TEST_MODE_S)
            .execute(db)
            .await
            .unwrap();
        let query = r#"DELETE FROM aircraft_photo WHERE url_photo = $1"#;
        sqlx::query(query)
            .bind("001/001/example.jpg")
            .execute(db)
            .await
            .unwrap();

        let app_env = AppEnv::get_env();
        let mut redis = db_redis::get_connection(&app_env).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<_, ()>(&mut redis)
            .await
            .unwrap();
    }

    #[test]
    fn scraper_deserialize_url() {
        let prefix = "https://www.xxxxxxxxxxxx.xxx/xxxxxx/xxxxxxxx/xxxxxxxxxxx";
        let suffix = "/000/582/582407.jpg";
        let deserializer: StringDeserializer<ValueError> =
            format!("{}{}", prefix, suffix).into_deserializer();
        let result = deserialize_url(deserializer);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/000/582/582407.jpg");

        let prefix = "https://www.xxxxxxxxxxxx.xxx";
        let suffix = "/000/582/582407.jpg";
        let deserializer: StringDeserializer<ValueError> =
            format!("{}{}", prefix, suffix).into_deserializer();
        let result = deserialize_url(deserializer);
        assert!(result.is_err());
    }

    #[test]
    fn scraper_validate_icao_codes() {
        // Too long
        let valid = String::from("AaBb12");
        let result = Scrapper::validate_icao(&valid);
        assert!(result.is_none());

        // Too short
        let valid = String::from("aaa");
        let result = Scrapper::validate_icao(&valid);
        assert!(result.is_none());

        // Invalid char short
        let valid = String::from("AAA*");
        let result = Scrapper::validate_icao(&valid);
        assert!(result.is_none());

        // Valid against known ORIGIN
        let result = Scrapper::validate_icao("roah");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), TEST_ORIGIN);

        // Valid against known DESTINATION
        let result = Scrapper::validate_icao("rjtt");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), TEST_DESTINATION);
    }

    #[tokio::test]
    async fn scraper_check_icao_in_db_true() {
        let setup = setup().await;
        let expected = ScrapedFlightroute {
            callsign: TEST_CALLSIGN.to_owned(),
            origin: TEST_ORIGIN.to_owned(),
            destination: TEST_DESTINATION.to_owned(),
        };
        let result = Scrapper::check_icao_in_db(&setup.1, &expected).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn scraper_check_icao_in_db_false_origin() {
        let setup = setup().await;
        let expected = ScrapedFlightroute {
            callsign: TEST_CALLSIGN.to_owned(),
            origin: "AAAA".to_owned(),
            destination: TEST_DESTINATION.to_owned(),
        };
        let result = Scrapper::check_icao_in_db(&setup.1, &expected).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn scraper_check_icao_in_db_false_destination() {
        let setup = setup().await;
        let expected = ScrapedFlightroute {
            callsign: TEST_CALLSIGN.to_owned(),
            origin: TEST_ORIGIN.to_owned(),
            destination: "AAAA".to_owned(),
        };
        let result = Scrapper::check_icao_in_db(&setup.1, &expected).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn scraper_extract_icao_codes() {
        let html_string = include_str!("./test_scrape.txt");
        let result = Scrapper::extract_icao_codes(html_string, TEST_CALLSIGN);

        let expected = ScrapedFlightroute {
            callsign: TEST_CALLSIGN.to_owned(),
            origin: TEST_ORIGIN.to_owned(),
            destination: TEST_DESTINATION.to_owned(),
        };

        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    /// in test mode, live site is actually just include_str()
    async fn scraper_use_live_site() {
        let setup = setup().await;
        let scraper = Scrapper::new(&setup.0);
        let result = scraper.request_callsign(TEST_CALLSIGN).await;

        assert!(result.is_ok());

        let result = Scrapper::extract_icao_codes(&result.unwrap(), TEST_CALLSIGN);
        let expected = ScrapedFlightroute {
            callsign: TEST_CALLSIGN.to_owned(),
            origin: TEST_ORIGIN.to_owned(),
            destination: TEST_DESTINATION.to_owned(),
        };

        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    /// in test mode, live site is actually just include_str()
    async fn scraper_scraper_for_route_insert() {
        let setup = setup().await;
        let scraper = Scrapper::new(&setup.0);
        let result = scraper.scrape_flightroute(&setup.1, TEST_CALLSIGN).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();

        let expected = ModelFlightroute {
            flightroute_id: 0,
            callsign: "ANA460".to_owned(),
            origin_airport_country_iso_name: "JP".to_owned(),
            origin_airport_country_name: "Japan".to_owned(),
            origin_airport_elevation: 12,
            origin_airport_iata_code: "OKA".to_owned(),
            origin_airport_icao_code: "ROAH".to_owned(),
            origin_airport_latitude: 26.195_801,
            origin_airport_longitude: 127.646_004,
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
            destination_airport_latitude: 35.552_299,
            destination_airport_longitude: 139.779_999,
            destination_airport_municipality: "Tokyo".to_owned(),
            destination_airport_name: "Tokyo Haneda International Airport".to_owned(),
        };

        // assert_eq!(expected, result);

        assert_eq!(result.callsign, expected.callsign);
        assert_eq!(
            result.origin_airport_country_iso_name,
            expected.origin_airport_country_iso_name
        );
        assert_eq!(
            result.origin_airport_country_name,
            expected.origin_airport_country_name
        );
        assert_eq!(
            result.origin_airport_elevation,
            expected.origin_airport_elevation
        );
        assert_eq!(
            result.origin_airport_iata_code,
            expected.origin_airport_iata_code
        );
        assert_eq!(
            result.origin_airport_icao_code,
            expected.origin_airport_icao_code
        );
        assert_eq!(
            result.origin_airport_latitude,
            expected.origin_airport_latitude
        );
        assert_eq!(
            result.origin_airport_longitude,
            expected.origin_airport_longitude
        );
        assert_eq!(
            result.origin_airport_municipality,
            expected.origin_airport_municipality
        );
        assert_eq!(result.origin_airport_name, expected.origin_airport_name);
        assert_eq!(
            result.midpoint_airport_country_iso_name,
            expected.midpoint_airport_country_iso_name
        );
        assert_eq!(
            result.midpoint_airport_country_name,
            expected.midpoint_airport_country_name
        );
        assert_eq!(
            result.midpoint_airport_elevation,
            expected.midpoint_airport_elevation
        );
        assert_eq!(
            result.midpoint_airport_iata_code,
            expected.midpoint_airport_iata_code
        );
        assert_eq!(
            result.midpoint_airport_icao_code,
            expected.midpoint_airport_icao_code
        );
        assert_eq!(
            result.midpoint_airport_latitude,
            expected.midpoint_airport_latitude
        );
        assert_eq!(
            result.midpoint_airport_longitude,
            expected.midpoint_airport_longitude
        );
        assert_eq!(
            result.midpoint_airport_municipality,
            expected.midpoint_airport_municipality
        );
        assert_eq!(result.midpoint_airport_name, expected.midpoint_airport_name);
        assert_eq!(
            result.destination_airport_country_iso_name,
            expected.destination_airport_country_iso_name
        );
        assert_eq!(
            result.destination_airport_country_name,
            expected.destination_airport_country_name
        );
        assert_eq!(
            result.destination_airport_elevation,
            expected.destination_airport_elevation
        );
        assert_eq!(
            result.destination_airport_iata_code,
            expected.destination_airport_iata_code
        );
        assert_eq!(
            result.destination_airport_icao_code,
            expected.destination_airport_icao_code
        );
        assert_eq!(
            result.destination_airport_latitude,
            expected.destination_airport_latitude
        );
        assert_eq!(
            result.destination_airport_longitude,
            expected.destination_airport_longitude
        );
        assert_eq!(
            result.destination_airport_municipality,
            expected.destination_airport_municipality
        );
        assert_eq!(
            result.destination_airport_name,
            expected.destination_airport_name
        );

        remove_scraped_data(&setup.1).await;
    }

    #[tokio::test]
    async fn scraper_get_photo() {
        let setup = setup().await;
        let scraper = Scrapper::new(&setup.0);

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "393C00".to_owned(),
            n_number: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        // let mode_s = ModeS::new("393C00".to_owned()).unwrap();
        let result = scraper.request_photo(&test_aircraft).await;
        assert!(result.is_some());
        let expected = PhotoResponse {
            status: 200,
            count: Some(1),
            data: Some([PhotoData {
                image: "001/001/example.jpg".to_owned(),
            }]),
        };
        assert_eq!(result.unwrap(), expected);

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "AAAAAA".to_owned(),
            n_number: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        let result = scraper.request_photo(&test_aircraft).await;
        assert!(result.is_none());

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "AAAAAB".to_owned(),
            n_number: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        let result = scraper.request_photo(&test_aircraft).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn scraper_get_photo_insert() {
        let setup = setup().await;
        let scraper = Scrapper::new(&setup.0);

        let mode_s = ModeS::new(TEST_MODE_S.to_owned()).unwrap();

        let test_aircraft = ModelAircraft::get(&setup.1, &mode_s, &setup.0.url_photo_prefix)
            .await
            .unwrap()
            .unwrap();

        let result = scraper.scrape_photo(&setup.1, &test_aircraft).await;
        assert!(result.is_ok());

        let result = ModelAircraft::get(&setup.1, &mode_s, &setup.0.url_photo_prefix).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();

        assert_eq!(result.aircraft_type, test_aircraft.aircraft_type);
        assert_eq!(result.icao_type, test_aircraft.icao_type);
        assert_eq!(result.manufacturer, test_aircraft.manufacturer);
        assert_eq!(result.mode_s, test_aircraft.mode_s);
        assert_eq!(result.n_number, test_aircraft.n_number);
        assert_eq!(result.registered_owner, test_aircraft.registered_owner);
        assert_eq!(
            result.registered_owner_country_iso_name,
            test_aircraft.registered_owner_country_iso_name
        );
        assert_eq!(
            result.registered_owner_country_name,
            test_aircraft.registered_owner_country_name
        );
        assert_eq!(
            result.registered_owner_operator_flag_code,
            test_aircraft.registered_owner_operator_flag_code
        );
        assert_eq!(
            result.url_photo,
            Some(format!("{}001/001/example.jpg", setup.0.url_photo_prefix)),
        );
        assert_eq!(
            result.url_photo_thumbnail,
            Some(format!(
                "{}thumbnails/001/001/example.jpg",
                setup.0.url_photo_prefix
            )),
        );

        remove_scraped_data(&setup.1).await;
    }
}
