use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};

use crate::{
    api::{AppError, Callsign},
    scraper::ScrapedFlightroute,
};

use super::ModelAirline;

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
    // THIS CAN BE NULL!
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

impl ModelFlightroute {
    /// Query for a fully joined Option<ModelFlightRoute>
    /// Don't return result, as issues with nulls in the database, that I can't be bothered to deal with at the moment
    async fn _get(db: &mut Transaction<'_, Postgres>, callsign: &Callsign) -> Option<Self> {
        let query = match callsign {
            Callsign::Iata(_) => Self::get_query_iata(),
            Callsign::Icao(_) => Self::get_query_icao(),
            Callsign::Other(_) => Self::get_query_callsign(),
        };

        match callsign {
            Callsign::Other(callsign) => sqlx::query_as::<_, Self>(query)
                .bind(callsign)
                .fetch_optional(&mut *db)
                .await
                .unwrap_or(None),
            Callsign::Iata(x) | Callsign::Icao(x) => {
                if let Ok(flightroute) = sqlx::query_as::<_, Self>(query)
                    .bind(&x.0)
                    .bind(&x.1)
                    .fetch_optional(&mut *db)
                    .await
                {
                    if let Some(flightroute) = flightroute {
                        Some(flightroute)
                    } else {
                        sqlx::query_as::<_, Self>(Self::get_query_callsign())
                            .bind(format!("{}{}", x.0, x.1))
                            .fetch_optional(&mut *db)
                            .await
                            .unwrap_or(None)
                    }
                } else {
                    None
                }
            }
        }
    }

    pub async fn get(db: &PgPool, callsign: &Callsign) -> Result<Option<Self>, AppError> {
        let mut transaction = db.begin().await?;
        let output = Self::_get(&mut transaction, callsign).await;
        transaction.commit().await?;
        Ok(output)
    }

    /// Query a flightroute based on a callsign with is a valid N-Number
    const fn get_query_callsign() -> &'static str {
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

    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_longitude,
        
    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_longitude,
            
    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_longitude
FROM
    flightroute fl
LEFT JOIN flightroute_callsign flc USING (flightroute_callsign_id)
LEFT JOIN 
    flightroute_callsign_inner fci
ON
    fci.flightroute_callsign_inner_id = flc.callsign_id
LEFT JOIN airport apo ON apo.airport_id = fl.airport_origin_id
LEFT JOIN airport apm ON apm.airport_id = fl.airport_midpoint_id
LEFT JOIN airport apd ON apd.airport_id = fl.airport_destination_id

WHERE fci.callsign = $1"
    }

    /// Query a flightroute based on a callsign with is a valid ICAO callsign
    const fn get_query_icao() -> &'static str {
        r"
SELECT
    fl.flightroute_id,
    concat($1,$2) as callsign,
    concat(ai.iata_prefix, (SELECT callsign FROM flightroute_callsign_inner WHERE flightroute_callsign_inner_id = iata_prefix_id)) AS callsign_iata,
    concat(ai.icao_prefix, (SELECT callsign FROM flightroute_callsign_inner WHERE flightroute_callsign_inner_id = icao_prefix_id)) AS callsign_icao,
    
    (SELECT country_iso_name FROM COUNTRY where country_id = ai.country_id) as airline_country_iso_name,
    (SELECT country_name FROM COUNTRY where country_id = ai.country_id) as airline_country_name,
    ai.airline_callsign,
    ai.airline_name,
    ai.iata_prefix AS airline_iata,
    ai.icao_prefix AS airline_icao,
    
    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_longitude,
          
    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_longitude,
              
    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_longitude
FROM
    flightroute fl
LEFT JOIN flightroute_callsign flc USING (flightroute_callsign_id)
LEFT JOIN 
    flightroute_callsign_inner fci
ON
    fci.flightroute_callsign_inner_id = flc.callsign_id
LEFT JOIN airline ai USING (airline_id)
LEFT JOIN airport apo ON apo.airport_id = fl.airport_origin_id
LEFT JOIN airport apm ON apm.airport_id = fl.airport_midpoint_id
LEFT JOIN airport apd ON apd.airport_id = fl.airport_destination_id
WHERE 
    flc.airline_id = (SELECT airline_id FROM airline WHERE icao_prefix = $1)
AND
    flc.icao_prefix_id = (SELECT flightroute_callsign_inner_id FROM flightroute_callsign_inner WHERE callsign = $2)"
    }

    /// EXPLAIN ANALYZE seems to think that using JOINS, instead of subqueries, is slower?
    const fn _get_query_icao_joins() -> &'static str {
        r"
SELECT
    fl.flightroute_id,
    concat($1, $2) as callsign,
    concat(ai.iata_prefix, (SELECT callsign FROM flightroute_callsign_inner WHERE flightroute_callsign_inner_id = iata_prefix_id)) AS callsign_iata,
    concat(ai.icao_prefix, (SELECT callsign FROM flightroute_callsign_inner WHERE flightroute_callsign_inner_id = icao_prefix_id)) AS callsign_icao,
    (SELECT country_iso_name FROM COUNTRY where country_id = ai.country_id) as airline_country_iso_name,
    (SELECT country_name FROM COUNTRY where country_id = ai.country_id) as airline_country_name,
    ai.airline_callsign,
    ai.airline_name,
    ai.iata_prefix AS airline_iata,
    ai.icao_prefix AS airline_icao,

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
LEFT JOIN
    flightroute_callsign flc
ON
    fl.flightroute_callsign_id = flc.flightroute_callsign_id
LEFT JOIN flightroute_callsign flc USING (flightroute_callsign_id)
LEFT JOIN airline ai USING (airline_id)

LEFT JOIN airport apo ON apo.airport_id= fl.airport_origin_id
LEFT JOIN country co_o ON co_o.country_id= apo.country_id
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
LEFT JOIN airport_icao_code aic_m ON aic_m.airport_icao_code_id  = apm.airport_icao_code_id
LEFT JOIN airport_iata_code aia_m ON aia_m.airport_iata_code_id  = apm.airport_iata_code_id
LEFT JOIN airport_name an_m ON an_m.airport_name_id  = apm.airport_name_id
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

WHERE
    flc.airline_id = (SELECT airline_id FROM airline WHERE icao_prefix = $1)
AND
    flc.icao_prefix_id = (SELECT flightroute_callsign_inner_id FROM flightroute_callsign_inner WHERE callsign = $2)"
    }

    /// Query a flightroute based on a callsign with is a valid IATA callsign
    /// The `DISTINCT` subquery is bad, and will crash!
    const fn get_query_iata() -> &'static str {
        r"
SELECT
    fl.flightroute_id,
    concat($1,$2) as callsign,
    concat(ai.iata_prefix, (SELECT callsign FROM flightroute_callsign_inner WHERE flightroute_callsign_inner_id = iata_prefix_id))  AS callsign_iata,
    concat(ai.icao_prefix, (SELECT callsign FROM flightroute_callsign_inner WHERE flightroute_callsign_inner_id = icao_prefix_id))  AS callsign_icao,

    ai.airline_name,
    ai.airline_callsign,
    ai.iata_prefix AS airline_iata,
    ai.icao_prefix AS airline_icao,
    (SELECT country_name FROM COUNTRY where country_id = ai.country_id) as airline_country_name,
    (SELECT country_iso_name FROM COUNTRY where country_id = ai.country_id) as airline_country_iso_name,

    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apo.airport_id ) AS origin_airport_longitude,
          
    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apm.airport_id ) AS midpoint_airport_longitude,
              
    ( SELECT tmp.country_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_country_name,
    ( SELECT tmp.country_iso_name FROM airport oa JOIN country tmp ON oa.country_id = tmp.country_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_country_iso_name,
    ( SELECT tmp.municipality FROM airport oa JOIN airport_municipality tmp ON oa.airport_municipality_id = tmp.airport_municipality_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_municipality,
    ( SELECT tmp.icao_code FROM airport oa JOIN airport_icao_code tmp ON oa.airport_icao_code_id = tmp.airport_icao_code_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_icao_code,
    ( SELECT tmp.iata_code FROM airport oa JOIN airport_iata_code tmp ON oa.airport_iata_code_id = tmp.airport_iata_code_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_iata_code,
    ( SELECT tmp.name FROM airport oa JOIN airport_name tmp ON oa.airport_name_id = tmp.airport_name_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_name,
    ( SELECT tmp.elevation FROM airport oa JOIN airport_elevation tmp ON oa.airport_elevation_id = tmp.airport_elevation_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_elevation,
    ( SELECT tmp.latitude FROM airport oa JOIN airport_latitude tmp ON oa.airport_latitude_id = tmp.airport_latitude_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_latitude,
    ( SELECT tmp.longitude FROM airport oa JOIN airport_longitude tmp ON oa.airport_longitude_id = tmp.airport_longitude_id WHERE oa.airport_id = apd.airport_id ) AS destination_airport_longitude

FROM flightroute fl
LEFT JOIN flightroute_callsign flc USING (flightroute_callsign_id)
LEFT JOIN flightroute_callsign_inner fci ON fci.flightroute_callsign_inner_id = flc.callsign_id
LEFT JOIN airline ai USING (airline_id)
LEFT JOIN airport apo ON apo.airport_id = fl.airport_origin_id
LEFT JOIN airport apm ON apm.airport_id = fl.airport_midpoint_id
LEFT JOIN airport apd ON apd.airport_id = fl.airport_destination_id

WHERE 
    flc.airline_id = (SELECT DISTINCT(ai.airline_id) FROM flightroute_callsign flc LEFT JOIN airline ai ON flc.airline_id = ai.airline_id WHERE ai.iata_prefix = $1 LIMIT 1)
AND
    flc.icao_prefix_id = (SELECT flightroute_callsign_inner_id FROM flightroute_callsign_inner WHERE callsign = $2)"
    }

    /// Transaction to insert a new flightroute
    async fn _insert_scraped_flightroute(
        transaction: &mut Transaction<'_, Postgres>,
        scraped_flightroute: &ScrapedFlightroute,
    ) -> Result<(), AppError> {
        if let Some(airline_id) =
            ModelAirline::get_by_icao_callsign(transaction, &scraped_flightroute.callsign_icao)
                .await?
        {
            let callsign_inner_query = "INSERT INTO flightroute_callsign_inner(callsign) VALUES($1) ON CONFLICT (callsign) DO NOTHING";
            sqlx::query(callsign_inner_query)
                .bind(scraped_flightroute.callsign_icao.get_suffix())
                .execute(&mut *transaction)
                .await?;
            sqlx::query(callsign_inner_query)
                .bind(scraped_flightroute.callsign_iata.get_suffix())
                .execute(&mut *transaction)
                .await?;

            let icao_prefix = "SELECT flightroute_callsign_inner_id AS id FROM flightroute_callsign_inner WHERE callsign = $1";
            let icao_prefix = sqlx::query_as::<_, Id>(icao_prefix)
                .bind(scraped_flightroute.callsign_icao.get_suffix())
                .fetch_one(&mut *transaction)
                .await?;

            let iata_prefix = "SELECT flightroute_callsign_inner_id AS id FROM flightroute_callsign_inner WHERE callsign = $1";
            let iata_prefix = sqlx::query_as::<_, Id>(iata_prefix)
                .bind(scraped_flightroute.callsign_iata.get_suffix())
                .fetch_one(&mut *transaction)
                .await?;

            let flighroute_callsign = "INSERT INTO flightroute_callsign(airline_id, iata_prefix_id, icao_prefix_id) VALUES($1, $2, $3) RETURNING flightroute_callsign_id AS id";

            let flighroute_callsign_id = sqlx::query_as::<_, Id>(flighroute_callsign)
                .bind(airline_id.airline_id)
                .bind(iata_prefix.id)
                .bind(icao_prefix.id)
                .fetch_one(&mut *transaction)
                .await?;
            let query = r"
INSERT INTO
    flightroute
        (airport_origin_id, airport_destination_id, flightroute_callsign_id)
    VALUES (
        (SELECT aa.airport_id FROM airport aa JOIN airport_icao_code aic ON aa.airport_icao_code_id = aic.airport_icao_code_id WHERE aic.icao_code = $2),
        (SELECT aa.airport_id FROM airport aa JOIN airport_icao_code aic ON aa.airport_icao_code_id = aic.airport_icao_code_id WHERE aic.icao_code = $3),
        $1
    )";
            sqlx::query(query)
                .bind(flighroute_callsign_id.id)
                .bind(&scraped_flightroute.origin)
                .bind(&scraped_flightroute.destination)
                .execute(&mut *transaction)
                .await?;
        }
        Ok(())
    }

    /// Insert, and return, a new flightroute
    #[cfg(not(test))]
    pub async fn insert_scraped_flightroute(
        db: &PgPool,
        scraped_flightroute: ScrapedFlightroute,
    ) -> Result<Option<Self>, AppError> {
        let mut transaction = db.begin().await?;
        Self::_insert_scraped_flightroute(&mut transaction, &scraped_flightroute).await?;
        transaction.commit().await?;
        let output = Self::get(db, &scraped_flightroute.callsign_icao).await?;
        Ok(output)
    }

    /// Insert, and return, a new flightroute, will rollback after returning flightroute
    #[cfg(test)]
    pub async fn insert_scraped_flightroute(
        db: &PgPool,
        scraped_flightroute: ScrapedFlightroute,
    ) -> Result<Option<Self>, AppError> {
        let mut transaction = db.begin().await?;
        Self::_insert_scraped_flightroute(&mut transaction, &scraped_flightroute).await?;
        let output = Self::_get(&mut transaction, &scraped_flightroute.callsign_icao).await;
        transaction.rollback().await?;
        Ok(output)
    }
}

/// Run tests with
///
/// cargo watch -q -c -w src/ -x 'test model_flightroute '
#[cfg(test)]
#[allow(clippy::pedantic, clippy::nursery, clippy::unwrap_used)]
mod tests {
    use crate::{db_postgres, parse_env::AppEnv};

    async fn setup() -> (AppEnv, PgPool) {
        let app_env = AppEnv::get_env();
        let db = db_postgres::db_pool(&app_env).await.unwrap();
        (app_env, db)
    }

    use super::*;
    #[tokio::test]
    async fn model_flightroute_scraped_flightroute_transaction() {
        let setup = setup().await;

        let mut transaction = setup.1.begin().await.unwrap();

        let scraped_flightroute = ScrapedFlightroute {
            callsign_icao: Callsign::Icao(("ANA".to_owned(), "666".to_owned())),
            callsign_iata: Callsign::Iata(("NH".to_owned(), "460".to_owned())),
            origin: "ROAH".to_owned(),
            destination: "RJTT".to_owned(),
        };

        ModelFlightroute::_insert_scraped_flightroute(&mut transaction, &scraped_flightroute)
            .await
            .unwrap();

        let result =
            ModelFlightroute::_get(&mut transaction, &scraped_flightroute.callsign_icao).await;
        assert!(result.is_some());
        let result = result.unwrap();

        let expected = ModelFlightroute {
            flightroute_id: result.flightroute_id,
            callsign: "ANA666".to_owned(),
            callsign_iata: Some("NH460".to_owned()),
            callsign_icao: Some("ANA666".to_owned()),
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

        // Cancel transaction, so can continually re-test with this route
        transaction.rollback().await.unwrap();
    }
}
