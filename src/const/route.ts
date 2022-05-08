export const routes = {
	AIRCRAFT_PARAM_MODE_S: '/aircraft/:mode_s',
	BASE: '/',
	CALLSIGN_PARAM_CALLSIGN: '/callsign/:callsign',
	CATCHALL: '*',
	ONLINE: '/online',
} as const;

export type routes = typeof routes[keyof typeof routes]