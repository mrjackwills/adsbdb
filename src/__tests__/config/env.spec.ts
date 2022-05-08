import { env } from '../../const/env';

import { describe, expect, it } from 'vitest';

describe('ENV test runner', () => {

	// const OLD_ENV = process.env;
	
	// const injectEnv = (name: string, value = ''): void => {
	// jest.resetModules();
	// process.env = { ...OLD_ENV, [name]: value };
	// };

	// beforeEach(async (): Promise<void> => {
	// process.env = OLD_ENV;
	// });
		
	it('Expect all envs to be valid', async () => {
		expect.assertions(22);
		expect(env.API_HOST).toBeTruthy();
		expect(env.API_PORT).toBeTruthy();
		expect(env.APP_NAME).toBeTruthy();
		expect(env.DOMAIN).toBeTruthy();
		expect(env.LOCATION_LOG_COMBINED).toBeTruthy();
		expect(env.LOCATION_LOG_ERROR).toBeTruthy();
		expect(env.LOCATION_LOG_EXCEPTION).toBeTruthy();
		expect(env.MODE_ENV_DEV).toBeFalsy();
		expect(env.MODE_ENV_PRODUCTION).toBeFalsy();
		expect(env.MODE_ENV_TEST).toBeTruthy();
		expect(env.PG_DATABASE).toBeTruthy();
		expect(env.PG_HOST).toBeTruthy();
		expect(env.PG_PASS).toBeTruthy();
		expect(env.PG_USER).toBeTruthy();
		expect(env.REDIS_HOST).toBeTruthy();
		expect(env.REDIS_PASSWORD).toBeTruthy();
		expect(env.SHOW_LOGS).toBeTruthy();
		expect(env.URL_CALLSIGN).toBeTruthy();
		expect(env.URL_AIRCRAFT_PHOTO).toBeTruthy();
		expect(typeof env.PG_PORT).toBe('number');
		expect(typeof env.REDIS_DATABASE).toBe('number');
		expect(typeof env.REDIS_PORT).toBe('number');
	});

	// it('Throws error when APP_NAME is missing', async () => {
	// 	injectEnv('APP_NAME');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.APP_NAME');
	// 		}
	// 	}
	// });

	// it('Throws error when API_HOST is missing', async () => {
	// 	injectEnv('API_HOST');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.API_HOST');
	// 		}
	// 	}
	// });

	// it('Throws error when API_PORT is missing', async () => {
	// 	injectEnv('API_PORT');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.API_PORT || isNaN');
	// 		}
	// 	}
	// });
	
	// it('Throws error when API_PORT isNaN', async () => {
	// 	injectEnv('API_PORT', 'zz');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.API_PORT || isNaN');
	// 		}
	// 	}
	// });

	// it('Throws error when DOMAIN is missing', async () => {
	// 	injectEnv('DOMAIN');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.DOMAIN');
	// 		}
	// 	}
	// });
	
	// it('Throws error when COOKIE_SECRET is missing', async () => {
	// 	injectEnv('COOKIE_SECRET');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.COOKIE_SECRET');
	// 		}
	// 	}
	// });
	
	// it('Throws error when DOMAIN is missing', async () => {
	// 	injectEnv('DOMAIN');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.DOMAIN');
	// 		}
	// 	}
	// });

	// it('Throws error when LOCATION_LOG_COMBINED is missing', async () => {
	// 	injectEnv('LOCATION_LOG_COMBINED');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.LOCATION_LOG_COMBINED');
	// 		}
	// 	}
	// });

	// it('Throws error when LOCATION_LOG_ERROR is missing', async () => {
	// 	injectEnv('LOCATION_LOG_ERROR');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.LOCATION_LOG_ERROR');
	// 		}
	// 	}
	// });

	// it('Throws error when LOCATION_LOG_EXCEPTION is missing', async () => {
	// 	injectEnv('LOCATION_LOG_EXCEPTION');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.LOCATION_LOG_EXCEPTION');
	// 		}
	// 	}
	// });

	// it('Throws error when PG_DATABASE is missing', async () => {
	// 	injectEnv('PG_DATABASE');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.PG_DATABASE');
	// 		}
	// 	}
	// });

	// it('Throws error when PG_HOST is missing', async () => {
	// 	injectEnv('PG_HOST');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.PG_HOST');
	// 		}
	// 	}
	// });

	// it('Throws error when PG_PASS is missing', async () => {
	// 	injectEnv('PG_PASS');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.PG_PASS');
	// 		}
	// 	}
	// });

	// it('Throws error when PG_PORT is missing', async () => {
	// 	injectEnv('PG_PORT');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.PG_PORT || isNaN');
	// 		}
	// 	}
	// });

	// it('Throws error when PG_PORT isNaN', async () => {
	// 	injectEnv('PG_PORT', 'zz');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.PG_PORT || isNaN');
	// 		}
	// 	}
	// });

	// it('Throws error when PG_USER is missing', async () => {
	// 	injectEnv('PG_USER');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.PG_USER');
	// 		}
	// 	}
	// });
	
	// it('Throws error when REDIS_DATABASE is missing', async () => {
	// 	injectEnv('REDIS_DATABASE');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.REDIS_DATABASE || isNaN');
	// 		}
	// 	}
	// });

	// it('Throws error when REDIS_DATABASE is not a number', async () => {
	// 	injectEnv('REDIS_DATABASE', 'zz');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.REDIS_DATABASE || isNaN');
	// 		}
	// 	}
	// });

	// it('Throws error when REDIS_HOST is missing', async () => {
	// 	injectEnv('REDIS_HOST');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.REDIS_HOST');
	// 		}
	// 	}
	// });
	
	// it('Throws error when REDIS_PASS is missing', async () => {
	// 	injectEnv('REDIS_PASS');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.REDIS_PASS');
	// 		}
	// 	}
	// });
	
	// it('Throws error when REDIS_PORT is missing', async () => {
	// 	injectEnv('REDIS_PORT');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.REDIS_PORT || isNaN');
	// 		}
	// 	}
	// });

	// it('Throws error when REDIS_DATABASE isNaN', async () => {
	// 	injectEnv('REDIS_DB', 'zz');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('isNaN(env.REDIS_DATABASE)');
	// 		}
	// 	}
	// });

	// it('Throws error when REDIS_PORT isNaN', async () => {
	// 	injectEnv('REDIS_PORT', 'zz');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.REDIS_PORT || isNaN');
	// 		}
	// 	}
	// });

	// it('Throws error when URL_CALLSIGN is missing', async () => {
	// 	injectEnv('URL_CALLSIGN');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.URL_CALLSIGN');
	// 		}
	// 	}
	// });

	// it('Throws error when URL_AIRCRAFT_PHOTO is missing', async () => {
	// 	injectEnv('URL_AIRCRAFT_PHOTO');
	// 	try {
	// 		await import('../../config/env');
	// 	} catch (e) {
	// 		if (e instanceof Error) {
	// 			expect(e).toBeInstanceOf(Error);
	// 			expect(e.message).toEqual('!env.URL_AIRCRAFT_PHOTO');
	// 		}
	// 	}
	// });

});