use std::{collections::HashMap, sync::Arc};

use reqwest::{Client, Response};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;
use tokio::sync::{
    broadcast::{Receiver, Sender},
    Mutex,
};

use crate::{
    api::{AppError, Callsign, Validate},
    db_postgres::{ModelAircraft, ModelFlightroute},
    parse_env::AppEnv,
};

#[cfg(not(test))]
use crate::api::UnknownAC;
#[cfg(not(test))]
use tracing::error;

#[cfg(test)]
use crate::sleep;

const SCRAPE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
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

/// Use this to keep track of currently scraping Aircraft & callsigns
/// is stored globally in an Arc Mutex
#[derive(Debug)]
pub struct ScraperThreadMap {
    photo: HashMap<ModelAircraft, Sender<()>>,
    flightroute: HashMap<Callsign, Sender<Option<ModelFlightroute>>>,
}

impl ScraperThreadMap {
    pub fn new() -> Self {
        Self {
            photo: HashMap::new(),
            flightroute: HashMap::new(),
        }
    }

    /// If the HashMap as a Sender in it, return a Subscriber to that sender
    fn get_flight_rx(&self, callsign: &Callsign) -> Option<Receiver<Option<ModelFlightroute>>> {
        self.flightroute
            .get(callsign)
            .map(tokio::sync::broadcast::Sender::subscribe)
    }

    /// Insert a sender into the HashMap
    fn insert_flight_tx(&mut self, callsign: &Callsign, tx: &Sender<Option<ModelFlightroute>>) {
        self.flightroute.insert(callsign.to_owned(), tx.to_owned());
    }

    /// Remove the callsign/sender from HashMap
    fn remove_flight(&mut self, callsign: &Callsign) {
        self.flightroute.remove(callsign);
    }

    /// If the HashMap as a Sender in it, return a Subscriber to that sender
    fn get_photo_rx(&self, aircraft: &ModelAircraft) -> Option<Receiver<()>> {
        self.photo
            .get(aircraft)
            .map(tokio::sync::broadcast::Sender::subscribe)
    }

    /// Insert a sender into the HashMap
    fn insert_photo_tx(&mut self, aircraft: &ModelAircraft, tx: &Sender<()>) {
        self.photo.insert(aircraft.to_owned(), tx.to_owned());
    }

    /// Remove the aircraft/sender from HashMap
    fn remove_photo(&mut self, aircraft: &ModelAircraft) {
        self.photo.remove(aircraft);
    }
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
            .connect_timeout(SCRAPE_TIMEOUT)
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

        let origin = output.first().map(std::borrow::ToOwned::to_owned);
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

    /// Scrape callsign url for whole page html string
    #[cfg(not(test))]
    async fn request_callsign(&self, callsign: &Callsign) -> Result<String, AppError> {
        match Self::client_get(format!("{}/{callsign}", self.flight_scrape_url)).await {
            Ok(response) => match response.text().await {
                Ok(text) => Ok(text),
                Err(e) => {
                    error!("{e:?}");
                    error!("can't transform callsign into text");
                    Err(AppError::UnknownInDb(UnknownAC::Callsign))
                }
            },
            Err(e) => {
                error!("{e:?}");
                error!("can't scrape callsign address");
                Err(AppError::UnknownInDb(UnknownAC::Callsign))
            }
        }
    }

    /// As above, but just return the test_scrape, instead of hitting a third party site
    #[cfg(test)]
    async fn request_callsign(&self, callsign: &Callsign) -> Result<String, AppError> {
        // artificial sleep, so can make sure things are in the hashmap
        sleep!(500);
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
                    error!("can't transform photo into json");
                    None
                }
            },
            Err(e) => {
                error!("{e:?}");
                error!("can't scrape photo address");
                None
            }
        }
    }

    /// Scrape photo for testings
    /// don't throw error as an internal process, but need to improve logging
    #[cfg(test)]
    async fn request_photo(&self, aircraft: &ModelAircraft) -> Option<PhotoResponse> {
        sleep!(500);
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
    /// Inserts the aircraft into a shared HashMap, so that if there are multiple requests for the same aircraft, it'll only scrape once
    /// Could also place the scrape into it's own tokio thread, but probably not worth it
    pub async fn scrape_photo(
        &self,
        db: &PgPool,
        aircraft: &ModelAircraft,
        scraper_threads: &Arc<Mutex<ScraperThreadMap>>,
    ) {
        if self.allow_scrape_photo.is_some() {
            let threads = scraper_threads.lock().await.get_photo_rx(aircraft);
            if let Some(mut rx) = threads {
                rx.recv().await.ok();
            } else {
                let (tx, _) = tokio::sync::broadcast::channel(1);
                scraper_threads.lock().await.insert_photo_tx(aircraft, &tx);
                if let Ok(Some(photo)) =
                    tokio::time::timeout(SCRAPE_TIMEOUT, self.request_photo(aircraft)).await
                {
                    if let Some([data_0, ..]) = photo.data.as_ref() {
                        if let Err(e) = aircraft.insert_photo(db, data_0).await {
                            tracing::error!("{}", e);
                        }
                    }
                }
                scraper_threads.lock().await.remove_photo(aircraft);
                tx.send(()).ok();
            }
        }
    }

    /// Scrape third party site for a flightroute, and try to insert into db
    /// Inserts the callsign into a shared HashMap, so that if there are multiple requests for the same callsign, it'll only scrape once
    /// Could also place the scrape into it's own tokio thread, but probably not worth it
    pub async fn scrape_flightroute(
        &self,
        postgres: &PgPool,
        callsign: &Callsign,
        scraper_threads: &Arc<Mutex<ScraperThreadMap>>,
    ) -> Result<Option<ModelFlightroute>, AppError> {
        let mut output = None;
        if self.allow_scrape_flightroute.is_some() {
            let thread = scraper_threads.lock().await.get_flight_rx(callsign);
            if let Some(mut rx) = thread {
                if let Ok(x) = rx.recv().await {
                    output = x;
                }
            } else {
                let (tx, _) = tokio::sync::broadcast::channel(1);
                scraper_threads.lock().await.insert_flight_tx(callsign, &tx);

                if let Ok(Ok(html)) =
                    tokio::time::timeout(SCRAPE_TIMEOUT, self.request_callsign(callsign)).await
                {
                    if let Some(scraped_flightroute) = Self::extract_flightroute(&html) {
                        match ModelFlightroute::insert_scraped_flightroute(
                            postgres,
                            &scraped_flightroute,
                        )
                        .await
                        {
                            Ok(flightroute) => output = flightroute,
                            Err(e) => tracing::error!("{}", e),
                        }
                    }
                }
                scraper_threads.lock().await.remove_flight(callsign);
                tx.send(output.clone()).ok();
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
    use crate::{db_postgres, db_redis, sleep};
    use fred::interfaces::ClientLike;
    use serde::de::value::{Error as ValueError, StringDeserializer};
    use serde::de::IntoDeserializer;

    const TEST_CALLSIGN: &str = "ANA460";
    const TEST_ORIGIN: &str = "ROAH";
    const TEST_DESTINATION: &str = "RJTT";
    const TEST_MODE_S: &str = "393C00";
    const TEST_REGISTRATION: &str = "F-GPAA";

    async fn test_setup() -> (AppEnv, PgPool) {
        let app_env = AppEnv::get_env();
        let db = db_postgres::get_pool(&app_env).await.unwrap();
        (app_env, db)
    }

    fn test_threads() -> Arc<Mutex<ScraperThreadMap>> {
        Arc::new(Mutex::new(ScraperThreadMap::new()))
    }

    async fn remove_scraped_data(db: &PgPool) {
        let callsign = Callsign::validate(TEST_CALLSIGN).unwrap();
        if let Some(flightroute) = ModelFlightroute::get(db, &callsign).await {
            sqlx::query!(
                "DELETE FROM flightroute WHERE flightroute_id = $1",
                flightroute.flightroute_id
            )
            .execute(db)
            .await
            .unwrap();

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
            let redis = db_redis::get_pool(&app_env).await.unwrap();
            redis.flushall::<()>(true).await.unwrap();
        }
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
    #[allow(unreachable_code)]
    // WARNING - this will test against a live, third party, website
    async fn scraper_extract_flightroute_live() {
        unimplemented!("`scraper_extract_flightroute_live` test currently disabled");

        let setup = test_setup().await;

        let url = format!("{}/{TEST_CALLSIGN}", setup.0.url_callsign);
        let html = Scraper::client_get(url)
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        let result = Scraper::extract_flightroute(&html);
        let expected = ScrapedFlightroute {
            callsign_icao: Callsign::Icao(("ANA".to_owned(), "460".to_owned())),
            callsign_iata: Callsign::Iata(("NH".to_owned(), "460".to_owned())),
            origin: TEST_ORIGIN.to_owned(),
            destination: TEST_DESTINATION.to_owned(),
        };

        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
        remove_scraped_data(&setup.1).await;
    }

    #[tokio::test]
    /// in test mode, live site is actually just include_str()
    async fn scraper_scrape_for_route_insert() {
        let callsign = Callsign::validate(TEST_CALLSIGN).unwrap();
        let setup = test_setup().await;
        remove_scraped_data(&setup.1).await;
        let scraper = Scraper::new(&setup.0);
        let threads = test_threads();

        let result = scraper
            .scrape_flightroute(&setup.1, &callsign, &threads)
            .await;

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
        remove_scraped_data(&setup.1).await;
    }

    #[tokio::test]
    /// Multiple request - each in own thread - results in the ScraperThreadMap being correctly populated
    async fn scraper_scrape_for_route_insert_threaded() {
        let callsign = Callsign::validate(TEST_CALLSIGN).unwrap();
        let setup = test_setup().await;
        remove_scraped_data(&setup.1).await;
        let thread_map = test_threads();

        let mut spawned_threads = vec![];

        for _ in 0..=3 {
            let callsign = callsign.clone();
            let pg = setup.1.clone();
            let threads = Arc::clone(&thread_map);
            let scraper = Scraper::new(&setup.0);
            spawned_threads.push(tokio::spawn(async move {
                scraper
                    .scrape_flightroute(&pg, &callsign, &threads)
                    .await
                    .ok();
            }))
        }

        sleep!(100);
        assert_eq!(thread_map.lock().await.flightroute.len(), 1);

        for i in spawned_threads {
            i.await.ok();
        }
        assert_eq!(thread_map.lock().await.flightroute.len(), 0);
        remove_scraped_data(&setup.1).await;
    }

    #[tokio::test]
    /// if callsign_scrape is none, doesn't scrape
    async fn scraper_scrape_for_route_null() {
        let callsign = Callsign::validate(TEST_CALLSIGN).unwrap();
        let mut setup = test_setup().await;
        let threads = test_threads();
        setup.0.allow_scrape_flightroute = None;
        let scraper = Scraper::new(&setup.0);
        let result = scraper
            .scrape_flightroute(&setup.1, &callsign, &threads)
            .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn scraper_get_photo() {
        let setup = test_setup().await;
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
            registered_owner_operator_flag_code: Some("AWI".to_owned()),
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
            registered_owner_operator_flag_code: Some("AWI".to_owned()),
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
            registered_owner_operator_flag_code: Some("AWI".to_owned()),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        let result = scraper.request_photo(&test_aircraft).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn scraper_get_photo_null() {
        let mut setup = test_setup().await;
        setup.0.allow_scrape_photo = None;
        let scraper = Scraper::new(&setup.0);
        let threads = test_threads();

        let test_aircraft = ModelAircraft {
            aircraft_id: 8415,
            aircraft_type: "CRJ 200LR".to_owned(),
            icao_type: "CRJ2".to_owned(),
            manufacturer: "Bombardier".to_owned(),
            mode_s: "393C00".to_owned(),
            registration: "N429AW".to_owned(),
            registered_owner_country_iso_name: "US".to_owned(),
            registered_owner_country_name: "United States".to_owned(),
            registered_owner_operator_flag_code: Some("AWI".to_owned()),
            registered_owner: "United Express".to_owned(),
            url_photo: None,
            url_photo_thumbnail: None,
        };

        scraper
            .scrape_photo(&setup.1, &test_aircraft, &threads)
            .await;
        // check for photo
        // assert!(result.is_ok());
    }

    #[tokio::test]
    async fn scraper_get_photo_insert_by_mode_s() {
        let setup = test_setup().await;
        let scraper = Scraper::new(&setup.0);
        let threads = test_threads();

        let mode_s = ModeS::validate(TEST_MODE_S).unwrap();

        let aircraft_search = AircraftSearch::ModeS(mode_s);
        let test_aircraft =
            ModelAircraft::get(&setup.1, &aircraft_search, &setup.0.url_photo_prefix)
                .await
                .unwrap()
                .unwrap();

        scraper
            .scrape_photo(&setup.1, &test_aircraft, &threads)
            .await;
        // let result = result.unwrap();
        // assert!(result.is_ok());
        // check for photo in db

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
    async fn scraper_get_photo_insert_by_mode_s_threaded() {
        let setup = test_setup().await;
        let thread_map = test_threads();

        let mode_s = ModeS::validate(TEST_MODE_S).unwrap();

        let aircraft_search = AircraftSearch::ModeS(mode_s);
        let test_aircraft =
            ModelAircraft::get(&setup.1, &aircraft_search, &setup.0.url_photo_prefix)
                .await
                .unwrap()
                .unwrap();

        let mut spawned_threads = vec![];

        for _ in 0..=3 {
            let test_aircraft = test_aircraft.clone();
            let pg = setup.1.clone();
            let threads = Arc::clone(&thread_map);
            let scraper = Scraper::new(&setup.0);
            spawned_threads.push(tokio::spawn(async move {
                scraper.scrape_photo(&pg, &test_aircraft, &threads).await;
            }))
        }

        sleep!(10);
        assert_eq!(thread_map.lock().await.photo.len(), 1);
        for i in spawned_threads {
            i.await.ok();
        }

        assert_eq!(thread_map.lock().await.photo.len(), 0);

        let result =
            ModelAircraft::get(&setup.1, &aircraft_search, &setup.0.url_photo_prefix).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());

        remove_scraped_data(&setup.1).await;
    }

    #[tokio::test]
    async fn scraper_get_photo_insert_by_registration() {
        let setup = test_setup().await;
        let scraper = Scraper::new(&setup.0);
        let threads = test_threads();

        let registration = Registration::validate(TEST_REGISTRATION).unwrap();

        let aircraft_search = AircraftSearch::Registration(registration);
        let test_aircraft =
            ModelAircraft::get(&setup.1, &aircraft_search, &setup.0.url_photo_prefix)
                .await
                .unwrap()
                .unwrap();

        scraper
            .scrape_photo(&setup.1, &test_aircraft, &threads)
            .await;

        // check for photo in db

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
