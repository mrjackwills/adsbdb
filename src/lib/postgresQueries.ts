import { AircraftId, CallSign, ModeS, PhotoId, TAircraftPhoto, TCallsignQuery, TInsertCallsign, TModeSQueryResult } from '../types';
import { log } from '../config/log';
import { LogEntry } from 'winston';
import { Pool } from 'pg';
import { postgresql } from '../config/db_postgres';
import format from 'pg-format';

const wrap = <T> () => function (_target: PostgresQueries, propertyKey: string, descriptor: PropertyDescriptor): void {
	const original = descriptor.value;
	descriptor.value = async function (...args: Array<T>): Promise<unknown> {
		const start = Date.now();
		log;
		try {
			const result = await original.call(this, ...args);
			log.debug(`${propertyKey} wrap: ${Date.now()-start}ms`);
			return result;
		} catch (e) {
			log.error(e);
			return;
		}
	};
};

class PostgresQueries {

	#db!: Pool;

	constructor (db: Pool) {
		this.#db = db;
	}

	async insert_error (data: LogEntry): Promise<void> {
		try {
			if (!data.message || !data.level || !data.timestamp) return;
			const query = format('INSERT INTO error_log(timestamp, level, message, stack, uuid) VALUES(%1$L, %2$L, %3$L, %4$L, %5$L)',
				data.timestamp, data.level, data.message, data.stack, data.uuid);
			await this.#db.query(query);
		
		} catch (e) {
			// eslint-disable-next-line no-console
			console.log(e);
		}
	}

	@wrap()
	async insert_photo (data: TAircraftPhoto): Promise<PhotoId> {
		const query = format('INSERT INTO aircraft_photo(url_photo, url_photo_thumbnail) VALUES(%1$L, %2$L) RETURNING aircraft_photo_id;',
			data.url_photo, data.url_photo_thumbnail);
		const { rows } = await this.#db.query(query);
		return rows[0].aircraft_photo_id;
	}

	@wrap()
	async insert_callsign ({ callsign, origin_icao, destination_icao }: TInsertCallsign): Promise<void> {
		const Client = await this.#db.connect();
		try {
			await Client.query('BEGIN');
			const callsignInsert = format('INSERT INTO flightroute_callsign(callsign) VALUES (%1$L) RETURNING flightroute_callsign_id', callsign);
			const { rows } = await Client.query(callsignInsert);
			const callsignId = rows[0].flightroute_callsign_id;
			const flightrouteInsert = format(
				// eslint-disable-next-line indent
`INSERT INTO
	flightroute(flightroute_callsign_id, airport_origin_id, airport_destination_id)
VALUES (
	%1$L,
	(SELECT aa.airport_id FROM airport aa JOIN airport_icao_code aic ON aa.airport_icao_code_id = aic.airport_icao_code_id WHERE aic.icao_code = %2$L),
	(SELECT aa.airport_id FROM airport aa JOIN airport_icao_code aic ON aa.airport_icao_code_id = aic.airport_icao_code_id WHERE aic.icao_code = %3$L)
	)`,
				callsignId, origin_icao, destination_icao);
			await Client.query(flightrouteInsert);
			await Client.query('COMMIT');
		} catch (e) {
			await Client.query('ROLLBACK');
			log.error(e);
		} finally {
			Client.release();
		}
	}
	
	@wrap()
	async update_aircraft_with_photo (photoId: PhotoId, aircraftId: AircraftId): Promise<void> {
		const query = format('UPDATE aircraft SET aircraft_photo_id = %1$L WHERE aircraft_id = %2$L;',
			photoId, aircraftId);
		await this.#db.query(query);
	}

	@wrap()
	async select_callsign (callsign: CallSign):Promise<TCallsignQuery|undefined> {

		const query = format (
			// eslint-disable-next-line indent
`SELECT
	%1$L as callsign,
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
JOIN
	flightroute_callsign flc
ON
	fl.flightroute_callsign_id = flc.flightroute_callsign_id
JOIN
	airport apo
ON
	fl.airport_origin_id = apo.airport_id
LEFT JOIN
	airport apm
ON
	fl.airport_midpoint_id = apm.airport_id
JOIN
	airport apd
ON
	fl.airport_destination_id = apd.airport_id
WHERE 
	flc.callsign = %1$L`, callsign);
		const { rows } = await this.#db.query(query);
		return rows[0];
	}

	@wrap()
	async select_mode_s (modeS: ModeS): Promise<TModeSQueryResult|undefined> {
		const query = format(
		// eslint-disable-next-line indent
`SELECT
	aa.aircraft_id,
	%1$L AS mode_s,
	aro.registered_owner,
	aof.operator_flag_code AS registered_owner_operator_flag_code,
	co.country_name AS registered_owner_country_name, co.country_iso_name AS registered_owner_country_iso_name,
	am.manufacturer,
	at.type,
	ait.icao_type,
	ap.url_photo, ap.url_photo_thumbnail
FROM
	aircraft aa
JOIN
	aircraft_mode_s ams
ON
	aa.aircraft_mode_s_id = ams.aircraft_mode_s_id
JOIN
	country co
ON
	aa.country_id = co.country_id
JOIN
	aircraft_type at
ON
	aa.aircraft_type_id = at.aircraft_type_id
JOIN
	aircraft_registered_owner aro
ON
	aa.aircraft_registered_owner_id = aro.aircraft_registered_owner_id
JOIN
	aircraft_icao_type ait
ON
	aa.aircraft_icao_type_id = ait.aircraft_icao_type_id
JOIN
	aircraft_manufacturer am 
ON
	aa.aircraft_manufacturer_id = am.aircraft_manufacturer_id
JOIN
	aircraft_operator_flag_code aof
ON
	aa.aircraft_operator_flag_code_id = aof.aircraft_operator_flag_code_id
LEFT JOIN
	aircraft_photo ap
ON
	aa.aircraft_photo_id = ap.aircraft_photo_id
WHERE
	ams.mode_s = %1$L;`,
			modeS.toUpperCase());
		const { rows } = await this.#db.query(query);
		return rows[0];
	}
}

export const postgresQueries = new PostgresQueries(postgresql);