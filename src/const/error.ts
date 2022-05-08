export const ErrorMessages = {
	INTERNAL: 'Internal server error',
	INVALID_CALLSIGN: 'Invalid callsign',
	INVALID_DATA: 'Invalid data',
	INVALID_MODE_S: 'Aircraft modeS string invalid',
	MALFORMED_JSON: 'Malformed JSON request',
	LARGE_PAYLOAD: 'Payload too large',
	RATE_LIMITED: 'Rate-limited',
	TYPE: 'TypeError',
	UNKNOWN_AIRCRAFT: 'unknown aircraft',
	UNKNOWN_CALLSIGN: 'unknown callsign',
	UNKNOWN_ENDPOINT: 'unknown endpoint',
} as const;

export type ErrorMessages = typeof ErrorMessages[keyof typeof ErrorMessages]