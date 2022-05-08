import { api_version } from '../config/api_version';
import { scraper } from '../lib/scraper';
import { CallSign, TCallsignQuery, TModeSQueryResult, TModeSQueryResultFrontend } from '../types';
import { customError } from '../config/customError';
import { ErrorMessages } from '../const/error';
import { httpCodes } from '../const/httpCode';
import { isCallSign, isModeS } from '../types/typeGuard';
import { postgresQueries } from '../lib/postgresQueries';
import { redisQueries } from '../lib/redisQueries';
import { RequestHandler } from 'express';
import { Send } from '../lib/send';

const removeAircraftId = (input: TModeSQueryResult): TModeSQueryResultFrontend => {
	const { aircraft_id: _unused_aircraft_id, ...aircraft } = input;
	return aircraft;
};

const remove_null = (x: TCallsignQuery): TCallsignQuery => {
	for (const [ key, value ] of Object.entries(x)) if (!value) delete x[<keyof TCallsignQuery>key];
	return x;
};

const insert_and_return_callsign = async (callsign: CallSign): Promise<TCallsignQuery|undefined> => {
	const response = await scraper.flightroute(callsign);
	if (response?.destination_icao && response?.origin_icao) {
		await postgresQueries.insert_callsign({ callsign, origin_icao: response.origin_icao, destination_icao: response.destination_icao });
		const data = await postgresQueries.select_callsign(callsign);
		return data;
	} else return undefined;
};

const getFlightRoute = async (callsign: CallSign): Promise<TCallsignQuery|undefined> => {
	const hasCallsignCache = await redisQueries.has_callsign_cache(callsign);
	if (hasCallsignCache) {
		const data = await redisQueries.get_callsign_cache(callsign);
		return data;
	} else {
		const data_in_postgres = await postgresQueries.select_callsign(callsign);
		const output = data_in_postgres ?? await insert_and_return_callsign(callsign);
		output ? await redisQueries.set_callsign_cache(callsign, output): await redisQueries.set_cache_unknown_callsign(callsign);
		return output;
	}
};

export const get_callsign: RequestHandler = async (req, res) => {
	const callsign_input = req.params.callsign?.toUpperCase();
	const validQueryCallsign = isCallSign(callsign_input);
	if (!validQueryCallsign) throw customError(httpCodes.NOT_FOUND, ErrorMessages.INVALID_CALLSIGN);
	const flightroute = validQueryCallsign ? await getFlightRoute(callsign_input) : undefined;
	if (!flightroute) throw customError(httpCodes.NOT_FOUND, ErrorMessages.UNKNOWN_CALLSIGN);
	Send({ res, response: { flightroute: remove_null(flightroute) } });
};

export const get_modeS: RequestHandler = async (req, res) => {
	const modeS_input = req.params.mode_s;
	const validModeS = isModeS(modeS_input);
	if (!validModeS) throw customError(httpCodes.BAD_REQUEST, ErrorMessages.INVALID_MODE_S);

	const callsign_input = req.query?.callsign?.toString().toUpperCase();
	const validQueryCallsign = isCallSign(callsign_input);

	const hasAircraftCache = await redisQueries.has_aircraft_cache(modeS_input);
	
	const [ aircraft, flightroute ] = await Promise.all([
		hasAircraftCache ? redisQueries.get_aircraft_cache(modeS_input) : postgresQueries.select_mode_s(modeS_input),
		validQueryCallsign ? getFlightRoute(callsign_input) : undefined
	]);

	if (!aircraft) {
		if (!hasAircraftCache) await redisQueries.set_cache_unknown_aircraft(modeS_input);
		throw customError(httpCodes.NOT_FOUND, ErrorMessages.UNKNOWN_AIRCRAFT);
	}
	
	// Should really check for null, else keep hammering the image server!
	if (!aircraft.url_photo && !hasAircraftCache) {
		const aircraftPhoto = await scraper.photo(modeS_input);
		if (aircraftPhoto) {
			const photoId = await postgresQueries.insert_photo(aircraftPhoto);
			await postgresQueries.update_aircraft_with_photo(photoId, aircraft.aircraft_id);
		}
		// could insert n/a?
		aircraft.url_photo = aircraftPhoto?.url_photo ?? '';
		aircraft.url_photo_thumbnail = aircraftPhoto?.url_photo_thumbnail ?? '';
	}
	
	if (!hasAircraftCache) await redisQueries.set_aircraft_cache(modeS_input, aircraft);

	const output = { aircraft: removeAircraftId(aircraft), flightroute: flightroute ? remove_null(flightroute) : undefined };
	Send({ res, response: output });
	
};

export const get_online: RequestHandler = async (_req, res) => {
	Send({ res, response: { api_version, uptime: `${Math.trunc(process.uptime())}` } });
};