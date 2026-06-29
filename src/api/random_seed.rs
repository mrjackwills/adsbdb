use std::collections::VecDeque;

use fred::clients::Pool;
use sqlx::PgPool;
use tokio::sync::oneshot;

use crate::{
    api::{AppError, Callsign, Validate},
    db_postgres::{ModelAircraft, ModelAirline, ModelFlightroute},
    db_redis::{RedisKey, insert_cache},
};

pub enum RandomMsg {
    Aircraft(oneshot::Sender<ModelAircraft>),
    AircraftQuery(oneshot::Sender<Vec<ModelAircraft>>),
    CallSign(oneshot::Sender<ModelFlightroute>),
    CallsignQuery(oneshot::Sender<Vec<ModelFlightroute>>),
    AirlineSign(oneshot::Sender<ModelAirline>),
    AirlinesignQuery(oneshot::Sender<Vec<ModelAirline>>),
}

struct RandomMsgHandler {
    aircraft_query: Option<()>,
    aircraft: VecDeque<ModelAircraft>,
    callsign_query: Option<()>,
    callsign: VecDeque<ModelAircraft>,
    airline_query: Option<()>,
    airline: VecDeque<ModelAircraft>,
    postgres: PgPool,
    redis: Pool,
}

impl RandomMsgHandler {
    async fn cache_aircraft(redis: &Pool, aircraft: &ModelAircraft) -> Result<(), AppError> {
        tokio::try_join!(
            insert_cache(redis, Some(aircraft), RedisKey::ModeS(&aircraft.mode_s),),
            insert_cache(
                redis,
                Some(aircraft),
                RedisKey::Registration(&aircraft.registration),
            )
        )?;
        Ok(())
    }

    //    async fn cache_airline(redis: &Pool, airline: &ModelAirline) -> Result<(), AppError> {
        // tokio::try_join!(
        //     insert_cache(redis, Some(airline), RedisKey::Airline(&airline),),
        // )?;
        // Ok(())
    // }

       async fn cache_callsign(&mut self, redis: &Pool, flightroute: &ModelFlightroute) -> Result<(), AppError> {
        // tokio::try_join!(
        //     insert_cache(redis, Some(callsign), RedisKey::Callsign(&aircraft.mode_s),),
        //     insert_cache(
        //         redis,
        //         Some(aircraft),
        //         RedisKey::Registration(&aircraft.registration),
        //     )
        // )?;
		  if let Ok(callsign) = Callsign::validate(flightroute.callsign.as_ref()) {
            insert_cache(
                &self.redis,
                Some(&flightroute),
                RedisKey::Callsign(&callsign),
            )
            .await?;
        }

        if let Some(callsign_iata) = flightroute.callsign_iata.as_ref()
            && let Ok(callsign) = Callsign::validate(callsign_iata)
        {
            insert_cache(
                &self.redis,
                Some(&flightroute),
                RedisKey::Callsign(&callsign),
            )
            .await?;
        }
        if let Some(callsign) = flightroute.callsign_icao.as_ref()
            && let Ok(c) = Callsign::validate(callsign)
        {
            insert_cache(&self.redis, Some(&flightroute), RedisKey::Callsign(&c)).await?;
        }

        Ok(())
    }

    //
    //  async_channel::Sender<()>
    pub fn start() {

        // if self.aircraft.len() < 100 & aircraft_get = None {
        // set aircrafte get to some, and spawn the query, with a sender
        // }
        // seed tthe vecs, start the message handler
    }

    /// Get random aircraft, and insert into cache using mode_s
    //  Result<ModelAircraft, AppError>
    async fn find_random_aircraft(&mut self) {

        // let key = RedisKey::RandomAircraft.to_string();
        // if let Some(aircraft) = state.redis.spop::<Option<String>, &str>(&key, None).await? {
        //     let aircraft = serde_json::from_str::<ModelAircraft>(&aircraft)?;
        //     Self::cache_aircraft(&state.redis, &aircraft).await?;
        //     return Ok(aircraft);
        // }

        // let mut aircraft_vec =
        //     ModelAircraft::get_random_vec(&state.postgres, &state.url_prefix).await?;

        // let response_aircraft = aircraft_vec.pop();

        // // Ideally shoud do this in the incoming_request_thread, as multiple incoming requests could cause race conditions here
        // // Use another thread and another message handler?
        // // Send a message of RandomAircradt, RandomCallsign, on each msg received in the thread, check vec size and return
        // // don't even need to use redis, can just keep it all in memory, spawn 1000 at start, if size < 100, created new vec and merge

        // for aircraft in aircraft_vec {
        //     if let Ok(serialized) = serde_json::to_string(&aircraft) {
        //         state
        //             .redis
        //             .sadd::<(), &str, String>(&key, serialized)
        //             .await?;
        //     }
        // }

        // let aircraft =
        //     response_aircraft.ok_or_else(|| AppError::Internal(S!("Random Query Error")))?;

        // Self::cache_aircraft(&state.redis, &aircraft).await?;

        // Ok(aircraft)
    }

    /// Get random aircraft, and insert into cache using mode_s
    /// Result<ModelAircraft, AppError>
    async fn find_random_callsign() {
        // let key = RedisKey::RandomAircraft.to_string();
        // if let Some(aircraft) = state.redis.spop::<Option<String>, &str>(&key, None).await? {
        //     let aircraft = serde_json::from_str::<ModelAircraft>(&aircraft)?;
        //     Self::cache_aircraft(&state.redis, &aircraft).await?;
        //     return Ok(aircraft);
        // }

        // let mut aircraft_vec =
        //     ModelAircraft::get_random_vec(&state.postgres, &state.url_prefix).await?;

        // let response_aircraft = aircraft_vec.pop();

        // // Ideally shoud do this in the incoming_request_thread, as multiple incoming requests could cause race conditions here
        // // Use another thread and another message handler?
        // // Send a message of RandomAircradt, RandomCallsign, on each msg received in the thread, check vec size and return
        // // don't even need to use redis, can just keep it all in memory, spawn 1000 at start, if size < 100, created new vec and merge

        // for aircraft in aircraft_vec {
        //     if let Ok(serialized) = serde_json::to_string(&aircraft) {
        //         state
        //             .redis
        //             .sadd::<(), &str, String>(&key, serialized)
        //             .await?;
        //     }
        // }

        // let aircraft =
        //     response_aircraft.ok_or_else(|| AppError::Internal(S!("Random Query Error")))?;

        // Self::cache_aircraft(&state.redis, &aircraft).await?;

        // Ok(aircraft)
    }
}
