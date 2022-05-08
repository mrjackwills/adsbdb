import { httpCodes } from '../const/httpCode';
import { Response } from 'express';
import { ErrorMessages } from '../const/error';

type Branded<K, T> = K & { __brand: T }
type CustomType<T> = Branded<string, T>
export type AircraftId = CustomType<'AircraftId'>
export type AircraftCacheKeyName = CustomType<'AircraftCacheKeyName'>
export type CallsignCacheKeyName = CustomType<'CallsignCacheKeyName'>
export type CallSign = CustomType<'CallSign'>
export type CallSignQuery = CustomType<'CallsignQuery'>
export type ModeS = CustomType<'ModeS'>
export type PhotoId = CustomType<'PhotoId'>
export type UUID = CustomType<'UUID'>
export type ICAO = CustomType<'ICAO'>

type response_callsign = { flightroute: TCallsignQuery|undefined }
type response = {api_version: string, uptime: string} | ErrorMessages | TErrorWitthUuid | { aircraft: TModeSQueryResultFrontend & response_callsign } | response_callsign

type TSend = {
	res: Response,
	status?: httpCodes;
	response?: response
}

export type TFsend = (x: TSend) => Promise<void>
export type TLoggerColors = { readonly [index in TLogLevels]: string };
export type TLogLevels = 'debug' | 'error' | 'verbose' | 'warn'

export type TErrorLog = { [ K in 'error_log_id' | 'message' | 'stack' | 'uuid'] : string } & { timestamp: Date, level: TLogLevels, http_code: number}

export type TParam = { mode_s: ModeS, callsign: CallSign }

export type TModeSQueryResultFrontend = { mode_s: ModeS }
	& { [ K in 'registered_owner'| 'registered_owner_operator_flag_code' |'registered_owner_country_name' | 'registered_owner_country_iso_name' | 'manufacturer' | 'type' | 'icao_type']: string }
	& { [ K in 'url_photo' | 'url_photo_thumbnail' ]: string | null }

export type TModeSQueryResult = { aircraft_id: AircraftId } & TModeSQueryResultFrontend

export type TAircraftPhoto = {[ K in 'url_photo' | 'url_photo_thumbnail' | 'photographer']: string}

export type TErrorWitthUuid = `${ErrorMessages}: ${UUID}`

type key_start = 'origin'|'destination'
type key_end = 'country_name'|'country_iso_name'|'municipality'|'icao_code'|'iata_code'|'name'|'longitude'|'latitude'
type key_elevation = `${key_start}_airport_elevation`

type key_full =`${key_start}_airport_${key_end}`
type midpoint_key =`midpoint_airport_${key_end}`

export type TCallsignQuery = { callsign: CallSign }
	& { [ K in key_full ]: string }
	& { [ K in key_elevation] : number }
	& { [ K in midpoint_key]? : string } & { midpoint_airport_elevation?: number }

export type TScrapedIcao = { [ K in 'origin_icao' | 'destination_icao']: ICAO|undefined}

export type TInsertCallsign = { callsign: CallSign } & { [ K in 'origin_icao' | 'destination_icao']: ICAO }

type scraperPhotoArray = { [ K in 'image'|'link' | 'photographer']: string }
export type TScraperPhoto = { [ K in 'status'| 'count']: number} & {data: Array<scraperPhotoArray> }