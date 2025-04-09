use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    api::{AppError, Callsign},
    redis_hash_to_struct,
    scraper::ScrapedFlightroute,
};

use super::{ModelAirline, ModelAirport};

/// Used in transaction of inserting a new scraped flightroute
#[derive(sqlx::FromRow, Debug, Clone, Copy)]
struct Id {
    id: i64,
}

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelFlightroute {
    pub flightroute_id: i64,
    pub callsign: String,
    pub callsign_iata: Option<String>,
    pub callsign_icao: Option<String>,

    pub airline_name: Option<String>,
    pub airline_country_name: Option<String>,
    pub airline_country_iso_name: Option<String>,
    pub airline_callsign: Option<String>,
    pub airline_icao: Option<String>,
    pub airline_iata: Option<String>,

    pub origin_airport_country_iso_name: String,
    pub origin_airport_country_name: String,
    pub origin_airport_elevation: i32,
    // THIS CAN BE NULL?
    pub origin_airport_iata_code: String,
    pub origin_airport_icao_code: String,
    pub origin_airport_latitude: f64,
    pub origin_airport_longitude: f64,
    pub origin_airport_municipality: String,
    pub origin_airport_name: String,

    pub midpoint_airport_country_iso_name: Option<String>,
    pub midpoint_airport_country_name: Option<String>,
    pub midpoint_airport_elevation: Option<i32>,
    pub midpoint_airport_iata_code: Option<String>,
    pub midpoint_airport_icao_code: Option<String>,
    pub midpoint_airport_latitude: Option<f64>,
    pub midpoint_airport_longitude: Option<f64>,
    pub midpoint_airport_municipality: Option<String>,
    pub midpoint_airport_name: Option<String>,

    pub destination_airport_country_iso_name: String,
    pub destination_airport_country_name: String,
    pub destination_airport_elevation: i32,
    pub destination_airport_iata_code: String,
    pub destination_airport_icao_code: String,
    pub destination_airport_latitude: f64,
    pub destination_airport_longitude: f64,
    pub destination_airport_municipality: String,
    pub destination_airport_name: String,
}
redis_hash_to_struct!(ModelFlightroute);

impl ModelFlightroute {
    /// Query a flightroute based on a callsign with is a valid N-Number
    fn get_query_callsign() -> String {
        format!(
            r"
SELECT
    fl.flightroute_id,
    $1 AS callsign,
    NULL AS callsign_iata,
    NULL AS callsign_icao,
    NULL as airline_name,
    NULL as airline_callsign,
    NULL as airline_iata,
    NULL as airline_icao,
    NULL as airline_country_name,
    NULL as airline_country_iso_name,
    {}
WHERE
    fci.callsign = $1
LIMIT
    1",
            Self::get_query_joins()
        )
    }

    // Main body for the flightroute query with all the joins
    const fn get_query_joins() -> &'static str {
        r"
    co_o.country_name AS origin_airport_country_name,
    co_o.country_iso_name AS origin_airport_country_iso_name,
    am_o.municipality AS origin_airport_municipality,
    aic_o.icao_code AS origin_airport_icao_code,
    aia_o.iata_code AS origin_airport_iata_code,
    an_o.name AS origin_airport_name,
    ae_o.elevation AS origin_airport_elevation,
    ala_o.latitude AS origin_airport_latitude,
    alo_o.longitude AS origin_airport_longitude,
    co_m.country_name AS midpoint_airport_country_name,
    co_m.country_iso_name AS midpoint_airport_country_iso_name,
    am_m.municipality AS midpoint_airport_municipality,
    aic_m.icao_code AS midpoint_airport_icao_code,
    aia_m.iata_code AS midpoint_airport_iata_code,
    an_m.name AS midpoint_airport_name,
    ae_m.elevation AS midpoint_airport_elevation,
    ala_m.latitude AS midpoint_airport_latitude,
    alo_m.longitude AS midpoint_airport_longitude,
    co_d.country_name AS destination_airport_country_name,
    co_d.country_iso_name AS destination_airport_country_iso_name,
    am_d.municipality AS destination_airport_municipality,
    aic_d.icao_code AS destination_airport_icao_code,
    aia_d.iata_code AS destination_airport_iata_code,
    an_d.name AS destination_airport_name,
    ae_d.elevation AS destination_airport_elevation,
    ala_d.latitude AS destination_airport_latitude,
    alo_d.longitude AS destination_airport_longitude
FROM
    flightroute fl
    LEFT JOIN flightroute_callsign flc USING(flightroute_callsign_id)
    LEFT JOIN airline ai USING(airline_id)
    LEFT JOIN flightroute_callsign_inner fci ON fci.flightroute_callsign_inner_id = flc.callsign_id
    LEFT JOIN airport apo ON apo.airport_id = fl.airport_origin_id
    LEFT JOIN country co_o ON co_o.country_id = apo.country_id
    LEFT JOIN airport_municipality am_o ON am_o.airport_municipality_id = apo.airport_municipality_id
    LEFT JOIN airport_icao_code aic_o ON aic_o.airport_icao_code_id = apo.airport_icao_code_id
    LEFT JOIN airport_iata_code aia_o ON aia_o.airport_iata_code_id = apo.airport_iata_code_id
    LEFT JOIN airport_name an_o ON an_o.airport_name_id = apo.airport_name_id
    LEFT JOIN airport_elevation ae_o ON ae_o.airport_elevation_id = apo.airport_elevation_id
    LEFT JOIN airport_latitude ala_o ON ala_o.airport_latitude_id = apo.airport_latitude_id
    LEFT JOIN airport_longitude alo_o ON alo_o.airport_longitude_id = apo.airport_longitude_id
    LEFT JOIN airport apm ON apm.airport_id = fl.airport_midpoint_id
    LEFT JOIN country co_m ON co_m.country_id = apm.country_id
    LEFT JOIN airport_municipality am_m ON am_m.airport_municipality_id = apm.airport_municipality_id
    LEFT JOIN airport_icao_code aic_m ON aic_m.airport_icao_code_id = apm.airport_icao_code_id
    LEFT JOIN airport_iata_code aia_m ON aia_m.airport_iata_code_id = apm.airport_iata_code_id
    LEFT JOIN airport_name an_m ON an_m.airport_name_id = apm.airport_name_id
    LEFT JOIN airport_elevation ae_m ON ae_m.airport_elevation_id = apm.airport_elevation_id
    LEFT JOIN airport_latitude ala_m ON ala_m.airport_latitude_id = apm.airport_latitude_id
    LEFT JOIN airport_longitude alo_m ON alo_m.airport_longitude_id = apm.airport_longitude_id
    LEFT JOIN airport apd ON apd.airport_id = fl.airport_destination_id
    LEFT JOIN country co_d ON co_d.country_id = apd.country_id
    LEFT JOIN airport_municipality am_d ON am_d.airport_municipality_id = apd.airport_municipality_id
    LEFT JOIN airport_icao_code aic_d ON aic_d.airport_icao_code_id = apd.airport_icao_code_id
    LEFT JOIN airport_iata_code aia_d ON aia_d.airport_iata_code_id = apd.airport_iata_code_id
    LEFT JOIN airport_name an_d ON an_d.airport_name_id = apd.airport_name_id
    LEFT JOIN airport_elevation ae_d ON ae_d.airport_elevation_id = apd.airport_elevation_id
    LEFT JOIN airport_latitude ala_d ON ala_d.airport_latitude_id = apd.airport_latitude_id
    LEFT JOIN airport_longitude alo_d ON alo_d.airport_longitude_id = apd.airport_longitude_id
"
    }

    /// Start of IATA and ICAO query
    const fn get_query_selects() -> &'static str {
        r"
SELECT
    fl.flightroute_id,
    concat($1, $2) as callsign,
    concat(
        ai.iata_prefix,
        (
            SELECT
                callsign
            FROM
                flightroute_callsign_inner
            WHERE
                flightroute_callsign_inner_id = iata_prefix_id
        )
    ) AS callsign_iata,
    concat(
        ai.icao_prefix,
        (
            SELECT
                callsign
            FROM
                flightroute_callsign_inner
            WHERE
                flightroute_callsign_inner_id = icao_prefix_id
        )
    ) AS callsign_icao,
    (
        SELECT
            country_iso_name
        FROM
            COUNTRY
        where
            country_id = ai.country_id
    ) as airline_country_iso_name,
    (
        SELECT
            country_name
        FROM
            COUNTRY
        where
            country_id = ai.country_id
    ) as airline_country_name,
    ai.airline_callsign,
    ai.airline_name,
    ai.iata_prefix AS airline_iata,
    ai.icao_prefix AS airline_icao,"
    }

    /// Query a flightroute based on a callsign with is a valid IATA callsign, will choose airline_id which has highest number of entries in flightroute, for when IATA collide
    fn get_query_iata() -> String {
        format!(
            r"
{}
{}
WHERE
    flc.airline_id = (
        SELECT
            ai.airline_id
        FROM
            flightroute_callsign flc
            LEFT JOIN airline ai ON flc.airline_id = ai.airline_id
        WHERE
            ai.iata_prefix = $1
        GROUP BY
            ai.airline_id
        ORDER BY
            COUNT(*)
        LIMIT
            1
    )
    AND flc.iata_prefix_id = (
        SELECT
            flightroute_callsign_inner_id
        FROM
            flightroute_callsign_inner
        WHERE
            callsign = $2
    )",
            Self::get_query_selects(),
            Self::get_query_joins()
        )
    }

    /// Query for flightroute based on ICAO callsign
    fn get_query_icao() -> String {
        format!(
            r"
{}
{}
WHERE
    flc.airline_id = (
        SELECT
            airline_id
        FROM
            airline
        WHERE
            icao_prefix = $1
    )
    AND flc.icao_prefix_id = (
        SELECT
            flightroute_callsign_inner_id
        FROM
            flightroute_callsign_inner
        WHERE
            callsign = $2
    )",
            Self::get_query_selects(),
            Self::get_query_joins()
        )
    }

    /// Query for a fully joined Option<ModelFlightRoute>
    /// Don't return result, as issues with nulls in the database, that I can't be bothered to deal with at the moment
    pub async fn get(db: &PgPool, callsign: &Callsign) -> Option<Self> {
        let query = match callsign {
            Callsign::Iata(_) => Self::get_query_iata(),
            Callsign::Icao(_) => Self::get_query_icao(),
            Callsign::Other(_) => Self::get_query_callsign(),
        };

        match callsign {
            Callsign::Other(callsign) => sqlx::query_as::<_, Self>(&query)
                .bind(callsign)
                .fetch_optional(db)
                .await
                .unwrap_or(None),
            Callsign::Iata(x) | Callsign::Icao(x) => {
                match sqlx::query_as::<_, Self>(&query)
                    .bind(&x.0)
                    .bind(&x.1)
                    .fetch_optional(db)
                    .await
                {
                    Ok(flightroute) => {
                        if let Some(flightroute) = flightroute {
                            Some(flightroute)
                        } else {
                            sqlx::query_as::<_, Self>(&Self::get_query_callsign())
                                .bind(format!("{}{}", x.0, x.1))
                                .fetch_optional(db)
                                .await
                                .unwrap_or(None)
                        }
                    }
                    _ => None,
                }
            }
        }
    }

    /// Transaction to insert, and return, a new flightroute,
    pub async fn insert_scraped_flightroute(
        db: &PgPool,
        scraped_flightroute: &ScrapedFlightroute,
    ) -> Result<Option<Self>, AppError> {
        if let Some(airline_id) =
            ModelAirline::get_by_icao_callsign(db, &scraped_flightroute.callsign_icao).await?
        {
            let origin = ModelAirport::get(db, &scraped_flightroute.origin).await?;
            let destination = ModelAirport::get(db, &scraped_flightroute.destination).await?;
            if let (Some(origin), Some(destination)) = (origin, destination) {
                let mut transaction = db.begin().await?;
                sqlx::query!(
                    "
INSERT INTO
    flightroute_callsign_inner(callsign)
VALUES
    ($1)
ON CONFLICT
    (callsign)
DO NOTHING",
                    scraped_flightroute.callsign_icao.get_suffix()
                )
                .execute(&mut *transaction)
                .await?;

                sqlx::query!(
                    "
INSERT INTO
    flightroute_callsign_inner(callsign)
VALUES
    ($1)
ON CONFLICT
    (callsign)
DO NOTHING",
                    scraped_flightroute.callsign_iata.get_suffix()
                )
                .execute(&mut *transaction)
                .await?;

                let icao_prefix = sqlx::query_as!(
                    Id,
                    "
SELECT
    flightroute_callsign_inner_id AS id
FROM
    flightroute_callsign_inner
WHERE
    callsign = $1",
                    scraped_flightroute.callsign_icao.get_suffix()
                )
                .fetch_one(&mut *transaction)
                .await?;

                let iata_prefix = sqlx::query_as!(
                    Id,
                    "
SELECT
    flightroute_callsign_inner_id AS id
FROM
    flightroute_callsign_inner
WHERE
    callsign = $1",
                    scraped_flightroute.callsign_iata.get_suffix()
                )
                .fetch_one(&mut *transaction)
                .await?;

                let flighroute_callsign_id = sqlx::query_as!(
                    Id,
                    "
INSERT INTO
    flightroute_callsign(airline_id, iata_prefix_id, icao_prefix_id)
VALUES
    ($1, $2, $3)
RETURNING
    flightroute_callsign_id AS id",
                    airline_id.airline_id,
                    iata_prefix.id,
                    icao_prefix.id
                )
                .fetch_one(&mut *transaction)
                .await?;
                sqlx::query!(
                    "
INSERT INTO
    flightroute (
        airport_origin_id,
        airport_destination_id,
        flightroute_callsign_id
    )
VALUES
    ($1, $2, $3)",
                    origin.airport_id,
                    destination.airport_id,
                    flighroute_callsign_id.id,
                )
                .execute(&mut *transaction)
                .await?;
                transaction.commit().await?;
            }
        }
        Ok(Self::get(db, &scraped_flightroute.callsign_icao).await)
    }

    pub async fn update(
        &self,
        postgres: &PgPool,
        origin: ModelAirport,
        destination: ModelAirport,
    ) -> Result<(), AppError> {
        sqlx::query!("UPDATE flightroute SET airport_origin_id = $1, airport_destination_id = $2 WHERE flightroute_id = $3",
		origin.airport_id, destination.airport_id, self.flightroute_id)
		.execute(postgres)
		.await?;

        Ok(())
    }
}

/// Run tests with
//
// cargo watch -q -c -w src/ -x 'test model_flightroute '
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{S, api::Validate, db_postgres, parse_env::AppEnv};

    async fn setup() -> (AppEnv, PgPool) {
        let app_env = AppEnv::get_env();
        let db = db_postgres::get_pool(&app_env).await.unwrap();
        (app_env, db)
    }

    async fn remove_flightroute(db: &PgPool, scraped_flightroute: &ScrapedFlightroute) {
        let flightroute = ModelFlightroute::get(db, &scraped_flightroute.callsign_iata)
            .await
            .unwrap();

        sqlx::query!(
            "DELETE FROM flightroute WHERE flightroute_id = $1",
            flightroute.flightroute_id
        )
        .execute(db)
        .await
        .unwrap();
    }

    use super::*;
    #[tokio::test]
    async fn model_flightroute_scraped_flightroute_transaction() {
        let setup = setup().await;

        let scraped_flightroute = ScrapedFlightroute {
            callsign_icao: Callsign::Icao((S!("ANA"), S!("000"))),
            callsign_iata: Callsign::Iata((S!("NH"), S!("000"))),
            origin: S!("ROAH"),
            destination: S!("RJTT"),
        };

        let result = ModelFlightroute::get(&setup.1, &scraped_flightroute.callsign_icao).await;
        assert!(result.is_none());

        ModelFlightroute::insert_scraped_flightroute(&setup.1, &scraped_flightroute)
            .await
            .unwrap();

        let result = ModelFlightroute::get(&setup.1, &scraped_flightroute.callsign_icao).await;

        assert!(result.is_some());

        let result = result.unwrap();

        let expected = ModelFlightroute {
            flightroute_id: result.flightroute_id,
            callsign: S!("ANA000"),
            callsign_iata: Some(S!("NH000")),
            callsign_icao: Some(S!("ANA000")),
            airline_name: Some(S!("All Nippon Airways")),
            airline_country_name: Some(S!("Japan")),
            airline_country_iso_name: Some(S!("JP")),
            airline_callsign: Some(S!("ALL NIPPON")),
            airline_iata: Some(S!("NH")),
            airline_icao: Some(S!("ANA")),
            origin_airport_country_iso_name: S!("JP"),
            origin_airport_country_name: S!("Japan"),
            origin_airport_elevation: 12,
            origin_airport_iata_code: S!("OKA"),
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
            destination_airport_iata_code: S!("HND"),
            destination_airport_icao_code: S!("RJTT"),
            destination_airport_latitude: 35.552_299,
            destination_airport_longitude: 139.779_999,
            destination_airport_municipality: S!("Tokyo"),
            destination_airport_name: S!("Tokyo Haneda International Airport"),
        };

        assert_eq!(result, expected);
        remove_flightroute(&setup.1, &scraped_flightroute).await;
    }

    const ICAO_CALLSIGNS: [(&str, &str); 122] = [
        ("AAL177", "AA177"),
        ("AAL2251", "AA2251"),
        ("AAL2781", "AA2781"),
        ("ACA1330", "AC1330"),
        ("ACA430", "AC430"),
        ("AFR90", "AF90"),
        ("AMC306", "KM306"),
        ("AMU882", "NX882"),
        ("ANE8435", "YW8435"),
        ("ANZ65", "NZ65"),
        ("ATN4312", "8C4312"),
        ("BAW382", "BA382"),
        ("BAW84PX", "BA84PX"),
        ("CAY608", "KX608"),
        ("CFG1TL", "DE1TL"),
        ("CPA238", "CX238"),
        ("CSN3378", "CZ3378"),
        ("CYP467", "CY467"),
        ("DAL1516", "DL1516"),
        ("DAL642", "DL642"),
        ("DAL9925", "DL9925"),
        ("DHK38", "D038"),
        ("EDV4789", "9E4789"),
        ("EDV4862", "9E4862"),
        ("EDW78", "WK78"),
        ("EIN1C6", "EI1C6"),
        ("ELY16", "LY16"),
        ("ELY25", "LY25"),
        ("EVA6606", "BR6606"),
        ("EWG465", "EW465"),
        ("EXS47SR", "LS47SR"),
        ("EXS878", "LS878"),
        ("EZS2186", "DS2186"),
        ("EZY2146", "U22146"),
        ("EZY2582", "U22582"),
        ("EZY278Z", "U2278Z"),
        ("EZY34TZ", "U234TZ"),
        ("EZY47UG", "U247UG"),
        ("FFT1093", "F91093"),
        ("FFT329", "F9329"),
        ("FFT8511", "F98511"),
        ("FIN7052", "AY7052"),
        ("IBE841", "IB841"),
        ("ISR49", "6H49"),
        ("JAF21", "JF21"),
        ("JBU2003", "B62003"),
        ("JST712", "JQ712"),
        ("JZA595", "QK595"),
        ("KLM168", "KL168"),
        ("MXY139", "MX139"),
        ("NKS1617", "NK1617"),
        ("OHY6645", "8Q6645"),
        ("OMA101", "WY101"),
        ("PAC97", "PO97"),
        ("PIA790", "PK790"),
        ("QFA578", "QF578"),
        ("RCH230", "MC230"),
        ("RXA4519", "ZL4519"),
        ("RYR2AN", "FR2AN"),
        ("RYR308D", "FR308D"),
        ("RYR4YX", "FR4YX"),
        ("RYR6TA", "FR6TA"),
        ("RYR91MQ", "FR91MQ"),
        ("RYR98BK", "FR98BK"),
        ("SAS2002", "SK2002"),
        ("SAS2168", "SK2168"),
        ("SKW2900", "OO2900"),
        ("SKW4125", "OO4125"),
        ("SKW4742", "OO4742"),
        ("SVA2400", "SV2400"),
        ("SWA1082", "WN1082"),
        ("SWA9032", "WN9032"),
        ("SWA970", "WN970"),
        ("TAI567", "TA567"),
        ("TAP72", "TP72"),
        ("TAP948N", "TP948N"),
        ("TAY59P", "3V59P"),
        ("THY3467", "TK3467"),
        ("TOM188", "BY188"),
        ("TOM4541", "BY4541"),
        ("UAE7DK", "EK7DK"),
        ("UAE978", "EK978"),
        ("UAL904", "UA904"),
        ("VLG3323", "VY3323"),
        ("VLG9GN", "VY9GN"),
        ("WZZ5323", "W65323"),
        ("AAR522", "OZ522"),
        ("AFR6730", "AF6730"),
        ("AFR73PV", "AF73PV"),
        ("BAW199", "BA199"),
        ("BAW33", "BA33"),
        ("BAW73V", "BA73V"),
        ("BCS2885", "QY2885"),
        ("CES552", "MU552"),
        ("CSN674", "CZ674"),
        ("ELY314", "LY314"),
        ("EZY98PG", "U298PG"),
        ("OMA102", "WY102"),
        ("RYR21VT", "FR21VT"),
        ("RYR26", "FR26"),
        ("RYR5KJ", "FR5KJ"),
        ("RYR86RP", "FR86RP"),
        ("RYR8LE", "FR8LE"),
        ("WZZ48HY", "W648HY"),
        ("EWG21CF", "EW21CF"),
        ("RYR42GA", "FR42GA"),
        ("QTR29A", "QR29A"),
        ("SKW3669", "OO3669"),
        ("RYR3SN", "FR3SN"),
        ("TVF79RR", "TO79RR"),
        ("FFT4762", "F94762"),
        ("PGT1158", "H91158"),
        ("KLM47U", "KL47U"),
        ("AUA73", "OS73"),
        ("HVN595", "VN595"),
        ("DAL1734", "DL1734"),
        ("SAS41C", "SK41C"),
        ("CSN658", "CZ658"),
        ("DAL445", "DL445"),
        ("THY8FH", "TK8FH"),
        ("SAS2305", "SK2305"),
        ("AFR96QH", "AF96QH"),
    ];

    #[tokio::test]
    /// Just check that a sample of flightroutes can be found, with both the ICAO and IATA callsign
    async fn flightroute_get() {
        let setup = setup().await;

        for callsign in ICAO_CALLSIGNS {
            let flightroute_icao =
                ModelFlightroute::get(&setup.1, &Callsign::validate(callsign.0).unwrap()).await;
            let flightroute_iata =
                ModelFlightroute::get(&setup.1, &Callsign::validate(callsign.1).unwrap()).await;
            assert!(flightroute_icao.is_some());
            assert!(flightroute_iata.is_some());

            let mut flightroute_icao = flightroute_icao.unwrap();
            let mut flightroute_iata = flightroute_iata.unwrap();

            // Need to ignore the callsign, else they won't match
            flightroute_icao.callsign = S!("ignore");
            flightroute_iata.callsign = S!("ignore");
            assert_eq!(flightroute_icao, flightroute_iata);
        }
    }
}
