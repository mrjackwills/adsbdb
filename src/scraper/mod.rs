use std::collections::HashMap;
use std::sync::LazyLock;

use reqwest::{Client, Response};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;
use url::Url;

#[cfg(not(test))]
use crate::api::UnknownAC;

use crate::{
    OneTX, RX, Tx,
    api::{AppError, Callsign, ModeS, Validate},
    create_rx_tx,
    db::postgres::{ModelAircraft, ModelFlightroute},
    parse_env::AppEnv,
};

const SCRAPE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[cfg(test)]
use crate::sleep;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct PhotoData {
    #[serde(deserialize_with = "deserialize_image_name")]
    pub image: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
struct PhotoResponse {
    status: u16,
    count: Option<u16>,
    data: Option<[PhotoData; 1]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrapedFlightroute {
    pub callsign_icao: Callsign,
    pub callsign_iata: Callsign,
    pub origin: String,
    pub destination: String,
}

// Check it's a URL, and then convert to just the filename as required
fn deserialize_image_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let input = String::deserialize(deserializer)?;
    Url::parse(&input).map_err(|_| serde::de::Error::custom("unparsable URL"))?;

    let Some(filename) = input.rsplit_once('/').map(|(_, suffix)| suffix) else {
        return Err(serde::de::Error::custom("rplit /"));
    };
    let Some((filename, ext)) = filename.split_once('.') else {
        return Err(serde::de::Error::custom("split ."));
    };
    if !filename.chars().all(|c| c.is_ascii_digit()) {
        return Err(serde::de::Error::custom("char digits"));
    }
    Ok(format!("{filename}.{ext}"))
}

/// Build a reqwest client, with a default timeout, and compression enabled
pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    match Client::builder()
        .connect_timeout(SCRAPE_TIMEOUT)
        .gzip(true)
        .brotli(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{e}");
            Client::new()
        }
    }
});

// TODO if want hashset instead of vec, need to impl hash & eq (?) for ModelFlightroute
pub struct Scraper {
    callsign_requests: HashMap<Callsign, Vec<OneTX<Option<ModelFlightroute>>>>,
    photo_requests: HashMap<ModeS, Vec<OneTX<()>>>,
    flight_scrape_url: String,
    allow_scrape_flightroute: Option<()>,
    photo_url: String,
    allow_scrape_photo: Option<()>,
    postgres: PgPool,
    tx: Tx<MsgScraper>,
}

pub enum MsgScraper {
    CallSign((OneTX<Option<ModelFlightroute>>, Callsign)),
    Photo((OneTX<()>, ModeS)),
    CallSignResult((Callsign, Box<Option<ModelFlightroute>>)),
    PhotoResult(ModeS),
}

impl Scraper {
    /// Scrape for a flightroute, or if currently being scraper, wait for response
    fn msg_callsign(
        &mut self,
        oneshot: OneTX<Option<ModelFlightroute>>,
        callsign: Callsign,
        tx: Tx<MsgScraper>,
    ) {
        if self.allow_scrape_flightroute.is_none() {
            oneshot.send(None);
            return;
        }
        if let Some(requests) = self.callsign_requests.get_mut(&callsign) {
            requests.push(oneshot);
        } else {
            self.callsign_requests
                .insert(callsign.clone(), vec![oneshot]);

            let data = (
                self.postgres.clone(),
                callsign,
                self.flight_scrape_url.clone(),
            );
            tokio::spawn(async move {
                let result = Self::spawn_callsign(data.0, &data.1, data.2).await;
                tx.send(MsgScraper::CallSignResult((data.1, Box::new(result))))
                    .await
                    .ok();
            });
        }
    }

    /// Scrape for a photo, or if currently being scraper, wait for response
    fn msg_photo(&mut self, oneshot: OneTX<()>, mode_s: ModeS, tx: Tx<MsgScraper>) {
        if self.allow_scrape_photo.is_none() {
            oneshot.send(());
            return;
        }

        if let Some(requests) = self.photo_requests.get_mut(&mode_s) {
            requests.push(oneshot);
        } else {
            self.photo_requests.insert(mode_s.clone(), vec![oneshot]);
            let data = (self.postgres.clone(), mode_s, self.photo_url.clone());
            tokio::spawn(async move {
                Self::spawn_photo(data.0, &data.1, data.2).await;
                tx.send(MsgScraper::PhotoResult(data.1)).await.ok();
            });
        }
    }

    pub async fn listen(&mut self, rx: RX<MsgScraper>) {
        while let Ok(msg) = rx.recv().await {
            match msg {
                MsgScraper::CallSign((oneshot, callsign)) => {
                    self.msg_callsign(oneshot, callsign, self.tx.clone());
                }
                MsgScraper::Photo((oneshot, mode_s)) => {
                    self.msg_photo(oneshot, mode_s, self.tx.clone());
                }
                MsgScraper::CallSignResult((callsign, result)) => {
                    if let Some(requests) = self.callsign_requests.remove(&callsign) {
                        let result = *result;
                        for oneshot in requests {
                            oneshot.send(result.clone());
                        }
                    }
                }
                MsgScraper::PhotoResult(mode_s) => {
                    if let Some(requests) = self.photo_requests.remove(&mode_s) {
                        for oneshot in requests {
                            oneshot.send(());
                        }
                    }
                }
            }
        }
    }

    /// Build a new scraper, and spawn in a tokio thread, return a Sender to be inserted into ApplicationState
    pub fn start(app_env: &AppEnv, postgres: &PgPool) -> Tx<MsgScraper> {
        let (tx, rx) = create_rx_tx(8192);
        let mut scraper = Self {
            flight_scrape_url: app_env.url_callsign.clone(),
            photo_url: app_env.url_aircraft_photo.clone(),
            allow_scrape_flightroute: app_env.allow_scrape_flightroute,
            allow_scrape_photo: app_env.allow_scrape_photo,
            callsign_requests: HashMap::new(),
            photo_requests: HashMap::new(),
            postgres: postgres.clone(),
            tx: tx.clone(),
        };
        tokio::spawn(async move {
            scraper.listen(rx).await;
        });
        tx
    }

    /// Use static client, send a get request to the url given
    async fn get_request(url: String) -> Result<Response, AppError> {
        Ok(CLIENT.get(url).timeout(SCRAPE_TIMEOUT).send().await?)
    }

    /// Try to extract the ICAO callsign, IATA callsign, and ICAO origin/destination airports
    /// Return None is origin and destination are the same
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

        let origin = html
            .split_once(r".setTargeting('origin', '")
            .and_then(|i| i.1.split_once('\''))
            .map(|i| i.0.to_owned());

        let destination = html
            .split_once(r".setTargeting('destination', '")
            .and_then(|i| i.1.split_once('\''))
            .map(|i| i.0.to_owned());

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

    /// As above, but just return the test_scrape, instead of hitting a third party site
    #[cfg(test)]
    async fn request_callsign(callsign: &Callsign, _url: String) -> Result<String, AppError> {
        use crate::S;
        sleep!(500);
        if callsign.to_string() == "ANA460" {
            Ok(include_str!("./test_scrape.txt").to_owned())
        } else {
            Ok(S!())
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::cognitive_complexity)]
    async fn request_callsign(callsign: &Callsign, url: String) -> Result<String, AppError> {
        match Self::get_request(format!("{url}/{callsign}")).await {
            Ok(response) => match response.text().await {
                Ok(text) => Ok(text),
                Err(e) => {
                    tracing::error!("{e:?}");
                    tracing::error!("can't transform callsign into text");
                    Err(AppError::UnknownInDb(UnknownAC::Callsign))
                }
            },
            Err(e) => {
                tracing::error!("{e:?}");
                tracing::error!("can't scrape callsign address");
                Err(AppError::UnknownInDb(UnknownAC::Callsign))
            }
        }
    }

    /// This is spawned in a tokio thread, scrapes the flightroute, inserts into postgres, and returns modelflightroute
    #[allow(clippy::cognitive_complexity)]
    async fn spawn_callsign(
        postgres: PgPool,
        callsign: &Callsign,
        url: String,
    ) -> Option<ModelFlightroute> {
        let Ok(html) = Self::request_callsign(callsign, url).await else {
            tracing::error!("{callsign}: request error");
            return None;
        };
        let scraped_flightroute = Self::extract_flightroute(&html)?;

        if scraped_flightroute.origin == scraped_flightroute.destination {
            tracing::error!(
                "{callsign}: airport clash: origin: {o}, destination: {d}",
                o = scraped_flightroute.origin,
                d = scraped_flightroute.destination
            );
            return None;
        }
        match ModelFlightroute::insert_scraped_flightroute(&postgres, &scraped_flightroute).await {
            Ok(flightroute) => flightroute,
            Err(e) => {
                tracing::error!("{}", e);
                None
            }
        }
    }

    /// This is spawned in a tokio thread, scrapes the photo, and inserts into postgres
    async fn spawn_photo(postgres: PgPool, mode_s: &ModeS, url: String) {
        let Some(photo) = Self::request_photo(mode_s, url).await else {
            return;
        };
        if let Some([data_0, ..]) = photo.data
            && let Err(e) = ModelAircraft::insert_photo(&postgres, data_0, mode_s).await
        {
            tracing::error!("{e}");
        }
    }

    /// Scrape photo for testings
    /// don't throw error as an internal process, but need to improve logging
    #[cfg(test)]
    async fn request_photo(mode_s: &ModeS, _url: String) -> Option<PhotoResponse> {
        use crate::S;

        sleep!(500);
        match mode_s.to_string().as_str() {
            "393C00" => Some(PhotoResponse {
                status: 200,
                count: Some(1),
                data: Some([PhotoData {
                    image: S!("987654321.jpg"),
                }]),
            }),
            _ => None,
        }
    }

    /// Request for photo from third party site
    #[cfg(not(test))]
    #[allow(clippy::cognitive_complexity)]
    async fn request_photo(mode_s: &ModeS, url: String) -> Option<PhotoResponse> {
        match Self::get_request(format!("{url}{mode_s}&n=1")).await {
            Ok(response) => match response.json::<PhotoResponse>().await {
                Ok(photo) => {
                    if photo.data.is_some() {
                        Some(photo)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    tracing::error!("{e:?}");
                    tracing::error!("can't transform photo into json");
                    None
                }
            },
            Err(e) => {
                tracing::error!("{e:?}");
                tracing::error!("can't scrape photo address");
                None
            }
        }
    }
}

/// Run tests with
///
/// cargo watch -q -c -w src/ -x 'test scraper_ '
#[cfg(test)]
#[allow(clippy::pedantic, clippy::unwrap_used)]
pub mod tests {
    use super::*;
    use crate::api::{AircraftSearch, ModeS, PhotoPrefixes, Validate};
    use crate::{S, create_oneshot, db};
    use fred::interfaces::ClientLike;
    use serde::de::IntoDeserializer;
    use serde::de::value::{Error as ValueError, StringDeserializer};

    pub const EXAMPLE_PHOTO_NAME: &str = "987654321.jpg";
    pub const TEST_CALLSIGN: &str = "ANA460";
    const TEST_ORIGIN: &str = "ROAH";
    const TEST_DESTINATION: &str = "RJTT";

    async fn test_setup() -> (AppEnv, PgPool) {
        let app_env = AppEnv::get_env();
        let db = db::postgres::get_pool(&app_env).await.unwrap();
        (app_env, db)
    }

    pub async fn remove_scraped_data(db: &PgPool) {
        let callsign = Callsign::validate(TEST_CALLSIGN).unwrap();
        if let Some(flightroute) = ModelFlightroute::get(db, &callsign).await {
            sqlx::query!(
                "DELETE FROM flightroute WHERE flightroute_id = $1",
                flightroute.flightroute_id.get()
            )
            .execute(db)
            .await
            .unwrap();
        }
        let query = r#"
        UPDATE aircraft SET aircraft_photo_id = NULL WHERE aircraft_photo_id = (
            SELECT
                ap.aircraft_photo_id
            FROM
                aircraft_photo ap
            WHERE
                ap.url_photo = $1
        )"#;

        sqlx::query(query)
            .bind(EXAMPLE_PHOTO_NAME)
            .execute(db)
            .await
            .unwrap();
        let query = r#"DELETE FROM aircraft_photo WHERE url_photo = $1"#;
        sqlx::query(query)
            .bind(EXAMPLE_PHOTO_NAME)
            .execute(db)
            .await
            .unwrap();

        let app_env = AppEnv::get_env();
        let redis = db::redis::get_pool(&app_env).await.unwrap();
        redis.flushall::<()>(true).await.unwrap();
    }

    #[test]
    fn scraper_deserialize_url() {
        let url = "https://example.com/foo/bar/another/001/924/001924443.jpg".to_owned();
        let deserializer: StringDeserializer<ValueError> = url.into_deserializer();
        let result = deserialize_image_name(deserializer);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "001924443.jpg");

        let url = "https://example.com/foo/bar/001/924/001924443jpg".to_owned();
        let deserializer: StringDeserializer<ValueError> = url.into_deserializer();
        let result = deserialize_image_name(deserializer);

        assert!(result.is_err());
    }

    #[test]
    fn scraper_extract_flightroute() {
        let html_string = include_str!("./test_scrape.txt");
        let result = Scraper::extract_flightroute(html_string);

        let expected = ScrapedFlightroute {
            callsign_icao: Callsign::Icao((S!("ANA"), S!("460"))),
            callsign_iata: Callsign::Iata((S!("NH"), S!("460"))),
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
        let setup = test_setup().await;
        remove_scraped_data(&setup.1).await;
        unimplemented!("`scraper_extract_flightroute_live` test currently disabled");

        let url = format!("{}/{TEST_CALLSIGN}", setup.0.url_callsign);
        let html = Scraper::get_request(url)
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        let result = Scraper::extract_flightroute(&html);
        let expected = ScrapedFlightroute {
            callsign_icao: Callsign::Icao((S!("ANA"), S!("460"))),
            callsign_iata: Callsign::Iata((S!("NH"), S!("460"))),
            origin: S!(TEST_ORIGIN),
            destination: S!(TEST_DESTINATION),
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
        let sender = Scraper::start(&setup.0, &setup.1);
        let (s, r) = create_oneshot();
        sender.send(MsgScraper::CallSign((s, callsign))).await.ok();

        let result = r.await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let result = result.unwrap();

        let expected = ModelFlightroute {
            flightroute_id: result.flightroute_id,
            callsign: S!("ANA460"),
            callsign_iata: Some(S!("NH460")),
            callsign_icao: Some(S!("ANA460")),
            airline_name: Some(S!("All Nippon Airways")),
            airline_country_name: Some(S!("Japan")),
            airline_country_iso_name: Some(S!("JP")),
            airline_callsign: Some(S!("ALL NIPPON")),
            airline_iata: Some(S!("NH")),
            airline_icao: Some(S!("ANA")),
            origin_airport_country_iso_name: S!("JP"),
            origin_airport_country_name: S!("Japan"),
            origin_airport_elevation: 12,
            origin_airport_iata_code: Some(S!("OKA")),
            origin_airport_icao_code: S!("ROAH"),
            origin_airport_latitude: 26.195_801,
            origin_airport_longitude: 127.646_004,
            origin_airport_municipality: S!("Naha"),
            origin_airport_name: S!("Naha Airport / JASDF Naha Air Base"),
            midpoint_airport_country_iso_name: None,
            midpoint_airport_country_name: None,
            midpoint_airport_elevation: None,
            midpoint_airport_iata_code: None,
            midpoint_airport_icao_code: None,
            midpoint_airport_latitude: None,
            midpoint_airport_longitude: None,
            midpoint_airport_municipality: None,
            midpoint_airport_name: None,
            destination_airport_country_iso_name: S!("JP"),
            destination_airport_country_name: S!("Japan"),
            destination_airport_elevation: 35,
            destination_airport_iata_code: Some(S!("HND")),
            destination_airport_icao_code: S!("RJTT"),
            destination_airport_latitude: 35.552_299,
            destination_airport_longitude: 139.779_999,
            destination_airport_municipality: S!("Tokyo"),
            destination_airport_name: S!("Tokyo Haneda International Airport"),
        };
        assert_eq!(result, expected);
        remove_scraped_data(&setup.1).await;
    }

    #[tokio::test]
    /// if callsign_scrape is none, doesn't scrape
    async fn scraper_scrape_for_route_null() {
        let callsign = Callsign::validate(TEST_CALLSIGN).unwrap();
        let mut setup = test_setup().await;
        remove_scraped_data(&setup.1).await;
        setup.0.allow_scrape_flightroute = None;
        let sender = Scraper::start(&setup.0, &setup.1);

        let (s, r) = create_oneshot();
        sender.send(MsgScraper::CallSign((s, callsign))).await.ok();

        let result = r.await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    /// TODO need to test photo is some and thumnaibail is none
    async fn scraper_get_photo() {
        let setup = test_setup().await;
        remove_scraped_data(&setup.1).await;
        let sender = Scraper::start(&setup.0, &setup.1);

        let mode_s = ModeS::from(S!("393C00"));

        let photo_prefix = PhotoPrefixes::from(&setup.0);

        let result = ModelAircraft::get(
            &setup.1,
            &AircraftSearch::ModeS(mode_s.clone()),
            &photo_prefix,
        )
        .await
        .unwrap()
        .unwrap();

        assert!(result.url_photo.is_none());
        assert!(result.url_photo_thumbnail.is_none());

        let (s, r) = create_oneshot();
        sender
            .send(MsgScraper::Photo((s, mode_s.clone())))
            .await
            .unwrap();
        let result = r.await;
        assert!(result.is_ok());

        let result = ModelAircraft::get(
            &setup.1,
            &AircraftSearch::ModeS(mode_s.clone()),
            &photo_prefix,
        )
        .await
        .unwrap()
        .unwrap();

        println!("{result:#?}");
        assert!(result.url_photo.is_some());
        let url_photo = result.url_photo.unwrap();
        assert!(url_photo.ends_with("987654321.jpg"));
        assert!(!url_photo.ends_with("/thumbnails/987/654/987654321.jpg"));
        assert!(result.url_photo_thumbnail.is_some());
        let url_photo_thumbnail = result.url_photo_thumbnail.unwrap();
        assert!(url_photo_thumbnail.ends_with("/thumbnails/987/654/987654321.jpg"));
        remove_scraped_data(&setup.1).await;
    }

    #[tokio::test]
    async fn scraper_get_photo_null() {
        let mut setup = test_setup().await;
        setup.0.allow_scrape_photo = None;
        let sender = Scraper::start(&setup.0, &setup.1);

        let mode_s = ModeS::from(S!("393C00"));
        let photo_prefix = PhotoPrefixes::from(&setup.0);

        let result = ModelAircraft::get(
            &setup.1,
            &AircraftSearch::ModeS(mode_s.clone()),
            &photo_prefix,
        )
        .await
        .unwrap()
        .unwrap();

        assert!(result.url_photo.is_none());
        assert!(result.url_photo_thumbnail.is_none());

        let (s, r) = create_oneshot();
        sender
            .send(MsgScraper::Photo((s, mode_s.clone())))
            .await
            .unwrap();
        let result = r.await;
        assert!(result.is_ok());

        let result = ModelAircraft::get(
            &setup.1,
            &AircraftSearch::ModeS(mode_s.clone()),
            &photo_prefix,
        )
        .await
        .unwrap()
        .unwrap();

        assert!(result.url_photo.is_none());
        assert!(result.url_photo_thumbnail.is_none());
        remove_scraped_data(&setup.1).await;
    }
}
