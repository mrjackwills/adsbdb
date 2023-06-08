use reqwest::{Client, Response};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;

#[cfg(not(test))]
use tracing::error;

use crate::{
    api::{AppError, Callsign, Validate},
    db_postgres::{ModelAircraft, ModelFlightroute},
    parse_env::AppEnv,
};

#[cfg(not(test))]
use crate::api::UnknownAC;

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
pub struct Scraper {
    flight_scrape_url: String,
    allow_scrape_flightroute: Option<()>,
    photo_url: String,
    allow_scrape_photo: Option<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrapedFlightroute {
    pub callsign_icao: Callsign,
    pub callsign_iata: Callsign,
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

impl Scraper {
    pub fn new(app_env: &AppEnv) -> Self {
        Self {
            flight_scrape_url: app_env.url_callsign.clone(),
            photo_url: app_env.url_aircraft_photo.clone(),
            allow_scrape_flightroute: app_env.allow_scrape_flightroute,
            allow_scrape_photo: app_env.allow_scrape_photo,
        }
    }

    /// Build a reqwest client, with a default timeout, and compression enabled
    /// Then send a get request to the url given
    #[allow(dead_code)]
    async fn client_get(url: String) -> Result<Response, AppError> {
        Ok(Client::builder()
            .connect_timeout(std::time::Duration::from_millis(10000))
            .gzip(true)
            .brotli(true)
            .build()?
            .get(url)
            .send()
            .await?)
    }

    /// Check that a given airport ICAO is valid, and return as uppercase
    fn validate_airport(input: &str) -> Option<String> {
        if (3..=4).contains(&input.chars().count())
            && input.chars().all(|i| i.is_ascii_alphabetic())
        {
            Some(input.to_uppercase())
        } else {
            None
        }
    }

    /// Try to extract the ICAO callsign, IATA callsign, and ICAO origin/destination airports
    fn extract_flightroute(html: &str) -> Option<ScrapedFlightroute> {
        let title_callsigns = html
            .split_once("<title>")
            .unwrap_or_default()
            .1
            .split_once("</title>")
            .unwrap_or_default()
            .0
            .split_once(')')
            .unwrap_or_default()
            .0
            .replace('(', "");
        let title_callsigns = title_callsigns.split_whitespace().collect::<Vec<_>>();

        let icao_callsign =
            Callsign::validate(title_callsigns.get(1).unwrap_or(&"")).map_or(None, |x| match x {
                Callsign::Icao(_) => Some(x),
                _ => None,
            });

        let iata_callsign =
            Callsign::validate(title_callsigns.first().unwrap_or(&"")).map_or(None, |x| match x {
                Callsign::Iata(_) => Some(x),
                _ => None,
            });

        let output = html
            .match_indices(ICAO)
            .filter_map(|i| {
                let icao_code = &html.split_at(i.0 + 8).1.chars().take(4).collect::<String>();
                Self::validate_airport(icao_code)
            })
            .collect::<Vec<_>>();

        let origin = output.get(0).map(std::borrow::ToOwned::to_owned);
        let destination = output.get(1).map(std::borrow::ToOwned::to_owned);

        if let (Some(callsign_icao), Some(callsign_iata), Some(origin), Some(destination)) =
            (icao_callsign, iata_callsign, origin, destination)
        {
            Some(ScrapedFlightroute {
                callsign_icao,
                callsign_iata,
                origin,
                destination,
            })
        } else {
            None
        }
    }

    // This is now done in the insert_transction
    /// Return true if BOTH airport_icao_code's are in db
    // async fn check_icao_in_db(db: &PgPool, scraped_flightroute: &ScrapedFlightroute) -> bool {
    //     let (start, end) = tokio::join!(
    //         ModelAirport::get(db, &scraped_flightroute.origin),
    //         ModelAirport::get(db, &scraped_flightroute.destination)
    //     );
    //     start.map_or(false, |f| f.is_some()) && end.map_or(false, |f| f.is_some())
    // }

    /// Scrape callsign url for whole page html string
    #[cfg(not(test))]
    async fn request_callsign(&self, callsign: &Callsign) -> Result<String, AppError> {
        match Self::client_get(format!("{}/{callsign}", self.flight_scrape_url)).await {
            Ok(response) => match response.text().await {
                Ok(text) => Ok(text),
                Err(e) => {
                    error!("{e:?}");
                    error!("can't transform into text");
                    Err(AppError::UnknownInDb(UnknownAC::Callsign))
                }
            },
            Err(e) => {
                error!("{e:?}");
                error!("can't scrape address");
                Err(AppError::UnknownInDb(UnknownAC::Callsign))
            }
        }
    }

    // As above, but just return the test_scrape, instead of hitting a third party site
    #[cfg(test)]
    #[allow(clippy::unused_async)]
    async fn request_callsign(&self, callsign: &Callsign) -> Result<String, AppError> {
        if callsign.to_string() == "ANA460" {
            Ok(include_str!("./test_scrape.txt").to_owned())
        } else {
            Ok(String::new())
        }
    }

    /// Request for photo from third party site
    #[cfg(not(test))]
    async fn request_photo(&self, aircraft: &ModelAircraft) -> Option<PhotoResponse> {
        match Self::client_get(format!(
            "{}ac_thumb.json?m={}&n=1",
            self.photo_url, aircraft.mode_s
        ))
        .await
        {
            Ok(response) => match response.json::<PhotoResponse>().await {
                Ok(photo) => {
                    if photo.data.is_some() {
                        Some(photo)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    error!("{e:?}");
                    error!("can't transform into json");
                    None
                }
            },
            Err(e) => {
                error!("{e:?}");
                error!("can't scrape address");
                None
            }
        }
    }

    /// Scrape photo for testings
    /// don't throw error as an internal process, but need to improve logging
    #[cfg(test)]
    #[allow(clippy::unused_async)]
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

    /// Attempt to get photol url, and also insert into db
    pub async fn scrape_photo(
        &self,
        db: &PgPool,
        aircraft: &ModelAircraft,
    ) -> Result<(), AppError> {
        if self.allow_scrape_photo.is_some() {
            if let Some(photo) = self.request_photo(aircraft).await {
                if let Some([data_0, ..]) = photo.data.as_ref() {
                    aircraft.insert_photo(db, data_0).await?;
                }
            }
        }
        Ok(())
    }

    /// Scrape third party site for a flightroute, and try to insert into db
    pub async fn scrape_flightroute(
        &self,
        postgres: &PgPool,
        callsign: &Callsign,
    ) -> Result<Option<ModelFlightroute>, AppError> {
        let mut output = None;
        if self.allow_scrape_flightroute.is_some() {
            if let Ok(html) = self.request_callsign(callsign).await {
                if let Some(scraped_flightroute) = Self::extract_flightroute(&html) {
                    output =
                        ModelFlightroute::insert_scraped_flightroute(postgres, scraped_flightroute)
                            .await?;
                }
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
    use crate::api::{AircraftSearch, ModeS, Registration, Validate};
    use crate::{db_postgres, db_redis};
    use serde::de::value::{Error as ValueError, StringDeserializer};
    use serde::de::IntoDeserializer;

    const TEST_CALLSIGN: &str = "ANA460";
    const TEST_ORIGIN: &str = "ROAH";
    const TEST_DESTINATION: &str = "RJTT";
    const TEST_MODE_S: &str = "393C00";
    const TEST_REGISTRATION: &str = "F-GPAA";

    async fn setup() -> (AppEnv, PgPool) {
        let app_env = AppEnv::get_env();
        let db = db_postgres::db_pool(&app_env).await.unwrap();
        (app_env, db)
    }

    async fn remove_scraped_data(db: &PgPool) {
        let query = r#"
        UPDATE aircraft SET aircraft_photo_id = NULL WHERE aircraft_photo_id = (
            SELECT
                ap.aircraft_photo_id
            FROM
                aircraft_photo ap
            WHERE
                ap.url_photo = '001/001/example.jpg'
        )"#;

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
            format!("{prefix}{suffix}").into_deserializer();
        let result = deserialize_url(deserializer);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/000/582/582407.jpg");

        let prefix = "https://www.xxxxxxxxxxxx.xxx";
        let suffix = "/000/582/582407.jpg";
        let deserializer: StringDeserializer<ValueError> =
            format!("{prefix}{suffix}").into_deserializer();
        let result = deserialize_url(deserializer);
        assert!(result.is_err());
    }

    #[test]
    fn scraper_validate_icao_codes() {
        // Too long
        let valid = String::from("AaBb12");
        let result = Scraper::validate_airport(&valid);
        assert!(result.is_none());

        // Too short
        let valid = String::from("aa");
        let result = Scraper::validate_airport(&valid);
        assert!(result.is_none());

        // Invalid char short
        let valid = String::from("AAA*");
        let result = Scraper::validate_airport(&valid);
        assert!(result.is_none());

        // Valid against known ORIGIN
        let result = Scraper::validate_airport("roah");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), TEST_ORIGIN);

        // Valid against known DESTINATION
        let result = Scraper::validate_airport("rjtt");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), TEST_DESTINATION);
    }

    // #[tokio::test]
    // async fn scraper_check_icao_in_db_true() {
    //     let setup = setup().await;

    //     let expected = ScrapedFlightroute {
    //         callsign_icao: Callsign::Icao(("ANA".to_owned(), "666".to_owned())),
    //         callsign_iata: Callsign::Iata(("NH".to_owned(), "460".to_owned())),
    //         origin: TEST_ORIGIN.to_owned(),
    //         destination: TEST_DESTINATION.to_owned(),
    //     };
    //     let result = Scraper::check_icao_in_db(&setup.1, &expected).await;
    //     assert!(result);
    // }

    // #[tokio::test]
    // async fn scraper_check_icao_in_db_false_origin() {
    //     let setup = setup().await;

    //     let expected = ScrapedFlightroute {
    //         callsign_icao: Callsign::Icao(("ANA".to_owned(), "666".to_owned())),
    //         callsign_iata: Callsign::Iata(("NH".to_owned(), "460".to_owned())),
    //         origin: "AAAA".to_owned(),
    //         destination: TEST_DESTINATION.to_owned(),
    //     };
    //     let result = Scraper::check_icao_in_db(&setup.1, &expected).await;
    //     assert!(!result);
    // }

    // #[tokio::test]
    // async fn scraper_check_icao_in_db_false_destination() {
    //     let setup = setup().await;

    //     let expected = ScrapedFlightroute {
    //         callsign_icao: Callsign::Icao(("ANA".to_owned(), "666".to_owned())),
    //         callsign_iata: Callsign::Iata(("NH".to_owned(), "460".to_owned())),
    //         origin: TEST_ORIGIN.to_owned(),
    //         destination: "AAAA".to_owned(),
    //     };
    //     let result = Scraper::check_icao_in_db(&setup.1, &expected).await;
    //     assert!(!result);
    // }

    #[test]
    fn scraper_extract_flightroute() {
        let html_string = include_str!("./test_scrape.txt");
        let result = Scraper::extract_flightroute(html_string);

        let expected = ScrapedFlightroute {
            callsign_icao: Callsign::Icao(("ANA".to_owned(), "460".to_owned())),
            callsign_iata: Callsign::Iata(("NH".to_owned(), "460".to_owned())),
            origin: TEST_ORIGIN.to_owned(),
            destination: TEST_DESTINATION.to_owned(),
        };

        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    // WARNING - this will test against a live, third party, website
    async fn scraper_extract_flightroute_live() {
        unimplemented!("`scraper_extract_flightroute_live` test currently disabled");

        //     let setup = setup().await;

        //     let url = format!("{}/{TEST_CALLSIGN}", setup.0.url_callsign);
        //     let html = Scraper::client_get(url)
        //         .await
        //         .unwrap()
        //         .text()
        //         .await
        //         .unwrap();

        //     let result = Scraper::extract_flightroute(&html);
        //     let expected = ScrapedFlightroute {
        //         callsign_icao: Callsign::Icao(("ANA".to_owned(), "460".to_owned())),
        //         callsign_iata: Callsign::Iata(("NH".to_owned(), "460".to_owned())),
        //         origin: TEST_ORIGIN.to_owned(),
        //         destination: TEST_DESTINATION.to_owned(),
        //     };

        //     assert!(result.is_some());
        //     assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    /// in test mode, live site is actually just include_str()
    async fn scraper_scrape_for_route_insert() {
        let callsign = Callsign::validate(TEST_CALLSIGN).unwrap();
        let setup = setup().await;
        let scraper = Scraper::new(&setup.0);
        let result = scraper.scrape_flightroute(&setup.1, &callsign).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();

        let expected = ModelFlightroute {
            flightroute_id: result.flightroute_id,
            callsign: "ANA460".to_owned(),
            callsign_iata: Some("NH460".to_owned()),
            callsign_icao: Some("ANA460".to_owned()),
            airline_name: Some("All Nippon Airways".to_owned()),
            airline_country_name: Some("Japan".to_owned()),
            airline_country_iso_name: Some("JP".to_owned()),
            airline_callsign: Some("ALL NIPPON".to_owned()),
            airline_iata: Some("NH".to_owned()),
            airline_icao: Some("ANA".to_owned()),
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
        assert_eq!(result, expected);
    }

    #[tokio::test]
    /// if callsign_scrape is none, doesn't scrape
    async fn scraper_scrape_for_route_null() {
        let callsign = Callsign::validate(TEST_CALLSIGN).unwrap();
        let mut setup = setup().await;
        setup.0.allow_scrape_flightroute = None;
        let scraper = Scraper::new(&setup.0);
        let result = scraper.scrape_flightroute(&setup.1, &callsign).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn scraper_get_photo() {
        let setup = setup().await;
        let scraper = Scraper::new(&setup.0);

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "393C00".to_owned(),
            registration: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

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
            registration: "N429AW".to_owned(),
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
            registration: "N429AW".to_owned(),
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
    async fn scraper_get_photo_null() {
        let mut setup = setup().await;
        setup.0.allow_scrape_photo = None;
        let scraper = Scraper::new(&setup.0);

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "393C00".to_owned(),
            registration: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: "AWI".to_owned(),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        let result = scraper.scrape_photo(&setup.1, &test_aircraft).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn scraper_get_photo_insert_by_mode_s() {
        let setup = setup().await;
        let scraper = Scraper::new(&setup.0);

        let mode_s = ModeS::validate(TEST_MODE_S).unwrap();

        let aircraft_search = AircraftSearch::ModeS(mode_s);
        let test_aircraft =
            ModelAircraft::get(&setup.1, &aircraft_search, &setup.0.url_photo_prefix)
                .await
                .unwrap()
                .unwrap();

        let result = scraper.scrape_photo(&setup.1, &test_aircraft).await;
        // let result = result.unwrap();
        assert!(result.is_ok());

        let result =
            ModelAircraft::get(&setup.1, &aircraft_search, &setup.0.url_photo_prefix).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();

        assert_eq!(result.aircraft_type, test_aircraft.aircraft_type);
        assert_eq!(result.icao_type, test_aircraft.icao_type);
        assert_eq!(result.manufacturer, test_aircraft.manufacturer);
        assert_eq!(result.mode_s, test_aircraft.mode_s);
        assert_eq!(result.registration, test_aircraft.registration);
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

    #[tokio::test]
    async fn scraper_get_photo_insert_by_registration() {
        let setup = setup().await;
        let scraper = Scraper::new(&setup.0);

        let registration = Registration::validate(TEST_REGISTRATION).unwrap();

        let aircraft_search = AircraftSearch::Registration(registration);
        let test_aircraft =
            ModelAircraft::get(&setup.1, &aircraft_search, &setup.0.url_photo_prefix)
                .await
                .unwrap()
                .unwrap();

        let result = scraper.scrape_photo(&setup.1, &test_aircraft).await;
        assert!(result.is_ok());

        let result =
            ModelAircraft::get(&setup.1, &aircraft_search, &setup.0.url_photo_prefix).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();

        assert_eq!(result.aircraft_type, test_aircraft.aircraft_type);
        assert_eq!(result.icao_type, test_aircraft.icao_type);
        assert_eq!(result.manufacturer, test_aircraft.manufacturer);
        assert_eq!(result.mode_s, test_aircraft.mode_s);
        assert_eq!(result.registration, test_aircraft.registration);
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
