export const RedisKeys = {
	// Keys
	MODE_S_CACHE: 'cache::mode_s',
	CALLSIGN_CACHE: 'cache::callsign',
	LIMITER: 'limiter',

	// hash names
	CACHED_DATA: 'cached_data',
	UNKNOWN_AIRCRAFT: 'unknown_aircraft',
	UNKNOWN_CALLSIGN: 'unknown_callsign',

	// data to store in hash
	UNKNOWN: 'unknown',

} as const;

export type RedisKeys = typeof RedisKeys[keyof typeof RedisKeys]