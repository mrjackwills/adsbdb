import { api_version } from '../config/api_version';
import { config } from 'dotenv';
import { resolve } from 'path';
config({ path: resolve(__dirname, '../../.env.local') });

const _env = process.env;
const _major = api_version.split('.')[0];

if (!_env.API_HOST) throw Error('!env.API_HOST');
if (!_env.API_PORT || isNaN(Number(_env.API_PORT))) throw Error('!env.API_PORT || isNaN');
if (isNaN(Number(_major))) throw Error('!major || isNaN');
if (!_env.DOMAIN) throw Error('!env.DOMAIN');

if (!_env.LOCATION_LOG_COMBINED) throw Error('!env.LOCATION_LOG_COMBINED');
if (!_env.LOCATION_LOG_ERROR) throw Error('!env.LOCATION_LOG_ERROR');
if (!_env.LOCATION_LOG_EXCEPTION) throw Error('!env.LOCATION_LOG_EXCEPTION');

if (!_env.PG_DATABASE) throw Error('!env.PG_DATABASE');
if (!_env.PG_HOST) throw Error('!env.PG_HOST');
if (!_env.PG_PASS) throw Error('!env.PG_PASS');
if (!_env.PG_PORT || isNaN(Number(_env.PG_PORT))) throw Error('!env.PG_PORT || isNaN');
if (!_env.PG_USER) throw Error('!env.PG_USER');

if (!_env.REDIS_DATABASE || isNaN(Number(_env.REDIS_DATABASE))) throw Error('!env.REDIS_DATABASE || isNaN');
if (!_env.REDIS_HOST) throw Error('!env.REDIS_HOST');
if (!_env.REDIS_PASSWORD) throw Error('!env.REDIS_PASSWORD');
if (!_env.REDIS_PORT || isNaN(Number(_env.REDIS_PORT))) throw Error('!env.REDIS_PORT || isNaN');

if (!_env.APP_NAME) throw Error('!env.APP_NAME');

if (!_env.URL_AIRCRAFT_PHOTO) throw Error('!env.URL_AIRCRAFT_PHOTO');
if (!_env.URL_CALLSIGN) throw Error('!env.URL_CALLSIGN');

export const env = {
	API_HOST: _env.API_HOST,
	API_PORT: Number(_env.API_PORT),
	APP_NAME: _env.APP_NAME,
	API_MAJOR_VERSION: Number(_major),
	DOMAIN: _env.DOMAIN,
	LOCATION_LOG_COMBINED: _env.LOCATION_LOG_COMBINED,
	LOCATION_LOG_ERROR: _env.LOCATION_LOG_ERROR,
	LOCATION_LOG_EXCEPTION: _env.LOCATION_LOG_EXCEPTION,
	MODE_ENV_DEV: _env.NODE_ENV === 'development',
	MODE_ENV_PRODUCTION: _env.NODE_ENV === 'production',
	MODE_ENV_TEST: _env.NODE_ENV === 'test',
	PG_DATABASE: _env.PG_DATABASE,
	PG_HOST: _env.PG_HOST,
	PG_PASS: _env.PG_PASS,
	PG_PORT: Number(_env.PG_PORT),
	PG_USER: _env.PG_USER,
	URL_AIRCRAFT_PHOTO: _env.URL_AIRCRAFT_PHOTO,
	URL_CALLSIGN: _env.URL_CALLSIGN,
	REDIS_DATABASE: Number(_env.REDIS_DATABASE),
	REDIS_HOST: _env.REDIS_HOST,
	REDIS_PASSWORD: _env.REDIS_PASSWORD,
	REDIS_PORT: Number(_env.REDIS_PORT),
	SHOW_LOGS: _env.SHOW_LOGS,
} as const;

export type Env = typeof env[keyof typeof env]