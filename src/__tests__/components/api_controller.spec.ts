import { TestHelper } from '../testHelper';
import { api_version } from '../../config/api_version';

import { afterAll, beforeAll, beforeEach, describe, expect, it } from 'vitest';

// AIRCRAFT_PARAM_MODE_S = '/aircraft/:mode_s',
// 	BASE = '/',
// 	CALLSIGN_PARAM_CALLSIGN = '/callsign/:callsign',
// 	CATCHALL = '*',
// 	ONLINE = testHelper.url_online,

const testHelper = new TestHelper();

describe('Incognito test runner', () => {
			
	beforeAll(async () => testHelper.beforeAll());
	afterAll(async () => testHelper.afterAll());

	describe(`ROUTE /online`, () => {

		afterAll(async () => {
			await testHelper.redis.flushdb();
		});

		it('DELETE 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.delete(testHelper.url_online);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});

		it('PATCH 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.patch(testHelper.url_online);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});

		it('POST 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.post(testHelper.url_online);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});

		it('PUT 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.put(testHelper.url_online);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});

		it('GET returns 200 api_version & uptime', async () => {
			expect.assertions(3);
			const result = await testHelper.axios.get(testHelper.url_online);
			expect(result.status).toEqual(200);
			expect(result.data.response.api_version).toStrictEqual(api_version);
			expect(result.data.response.uptime).toMatch(/(\d+(?:\.\d+)?)/);
		});
		
	});

	describe(`ROUTE /aircraft`, () => {

		beforeEach(async () => testHelper.beforeEach());

		afterAll(async () => {
			await testHelper.redis.flushdb();
		});

		it('DELETE 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.delete(testHelper.url_online);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});

		it('GET 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.get(testHelper.url_aircraft);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});

		it('PATCH 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.patch(testHelper.url_aircraft);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});

		it('POST 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.post(testHelper.url_aircraft);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});

		it('PUT 404', async () => {
			expect.assertions(2);
			try {
				await testHelper.axios.put(testHelper.url_aircraft);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual(testHelper.response_unknown);
			}
		});
	});
	
	describe(`ROUTE /aircraft/:modeS `, () => {

		beforeEach(async () => {
			await testHelper.beforeEach();

		});

		it('GET invalid modeS 400 bad response', async () => {
			expect.assertions(2);
			try {
				const randomString = await testHelper.randomHex(3);
				await testHelper.axios.get(`${testHelper.url_aircraft}/${randomString}`);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(400);
				expect(e.response?.data).toEqual(testHelper.response_invalid_modeS);
			}
		});

		it('Returns known aircraft details', async () => {
			expect.assertions(1);
			const randomAircraft = testHelper.randomAircraft();
			const result = await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}`);
			expect(result.data.response.aircraft).toMatchObject(randomAircraft);
		});
		
		it('Calls aircraft photo axios call (mock)', async () => {
			expect.assertions(2);
			const randomAircraft = testHelper.randomAircraft_noPhoto();
			const pre = testHelper.mockedAxiosAircraftPhoto.mock.calls.length;
			await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}`);
			const post = testHelper.mockedAxiosAircraftPhoto.mock.calls.length;
			expect(pre).toEqual(0);
			expect(post).toEqual(1);
		});
			
		it('places aircraft into cache', async () => {
			expect.assertions(2);
			const randomAircraft = testHelper.randomAircraft();
			await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}`);
			const cached_aircraft = await testHelper.redis.hget(`cache::mode_s:${randomAircraft.mode_s}`, 'cached_data');
			if (!cached_aircraft) throw Error('!cached aircraft');
			const parsed_aircraft = JSON.parse(cached_aircraft);
			expect(parsed_aircraft.aircraft_id).toBeTruthy();
			expect(parsed_aircraft).toMatchObject({ ...randomAircraft, aircraft_id: parsed_aircraft.aircraft_id });
		});

		it('aircraft cache of 7 days', async () => {
			expect.assertions(1);
			const randomAircraft = testHelper.randomAircraft();
			await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}`);
			const cache_ttl = await testHelper.redis.ttl(`cache::mode_s:${randomAircraft.mode_s}`);
			// Minus 10 seconds just for a bit of leeway
			const seven_days = 60 * 60 * 24 * 7 - 10;
			expect(cache_ttl).toBeGreaterThanOrEqual(seven_days);
		});
		
		// TODO test gets inserted into db
		
	});

	describe(`ROUTE /aircraft/:modeS `, () => {

		afterAll(async () => {
			await testHelper.redis.flushdb();
		});

		beforeEach(async () => {
			await testHelper.beforeEach();

		});

		it('GET invalid modeS 400 bad response', async () => {
			expect.assertions(2);
			try {
				const randomString = await testHelper.randomHex(3);
				await testHelper.axios.get(`${testHelper.url_aircraft}/${randomString}`);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(400);
				expect(e.response?.data).toEqual(testHelper.response_invalid_modeS);
			}
		});

		it('Returns known aircraft details', async () => {
			expect.assertions(1);
			const randomAircraft = testHelper.randomAircraft();
			const result = await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}`);
			expect(result.data.response.aircraft).toMatchObject(randomAircraft);
		});
		
		it('Calls aircraft photo axios call (mock)', async () => {
			expect.assertions(2);
			const randomAircraft = testHelper.randomAircraft_noPhoto();
			const pre = testHelper.mockedAxiosAircraftPhoto.mock.calls.length;
			await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}`);
			const post = testHelper.mockedAxiosAircraftPhoto.mock.calls.length;
			expect(pre).toEqual(0);
			expect(post).toEqual(1);
		});
		
		// TODO test gets inserted into db

	});

	describe(`ROUTE /aircraft/:modeS?callsign=:callsign `, () => {

		afterAll(async () => {
			await testHelper.redis.flushdb();
		});

		beforeEach(async () => {
			await testHelper.beforeEach();

		});

		it('GET return correct aircraft and flightroute info', async () => {
			expect.assertions(2);
			const randomAircraft = testHelper.randomAircraft();
			const randomFlightroute = testHelper.randomFlightroute();
			const result = await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}?callsign=${randomFlightroute.callsign}`);
			expect(result.data.response.aircraft).toMatchObject(randomAircraft);
			expect(result.data.response.flightroute).toMatchObject(randomFlightroute);
		});

		it('Flightroute data saved into cache', async () => {
			expect.assertions(1);
			const randomAircraft = testHelper.randomAircraft();
			const randomFlightroute = testHelper.randomFlightroute();
			await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}?callsign=${randomFlightroute.callsign}`);
			const cached_callsign = await testHelper.redis.hget(`cache::callsign:${randomFlightroute.callsign}`, 'cached_data');
			const parsed_callsign = JSON.parse(cached_callsign);
			expect(parsed_callsign).toMatchObject(randomFlightroute);
		});

		it('Both aircraft and flightroute data saved into cache', async () => {
			expect.assertions(3);
			const randomAircraft = testHelper.randomAircraft();
			const randomFlightroute = testHelper.randomFlightroute();
			await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}?callsign=${randomFlightroute.callsign}`);
			const cached_callsign = await testHelper.redis.hget(`cache::callsign:${randomFlightroute.callsign}`, 'cached_data');
			const parsed_callsign = JSON.parse(cached_callsign);
			const cached_aircraft = await testHelper.redis.hget(`cache::mode_s:${randomAircraft.mode_s}`, 'cached_data');
			if (!cached_aircraft) throw Error('!cached aircraft');
			const parsed_aircraft = JSON.parse(cached_aircraft);
			expect(parsed_aircraft.aircraft_id).toBeTruthy();
			expect(parsed_aircraft).toMatchObject({ ...randomAircraft, aircraft_id: parsed_aircraft.aircraft_id });
			expect(parsed_callsign).toMatchObject(randomFlightroute);
		});

		it('GET Flightroute cache to have a ttl of 7 days', async () => {
			expect.assertions(1);
			const randomAircraft = testHelper.randomAircraft();
			const randomFlightroute = testHelper.randomFlightroute();
			await testHelper.axios.get(`${testHelper.url_aircraft}/${randomAircraft.mode_s}?callsign=${randomFlightroute.callsign}`);
			const cache_ttl = await testHelper.redis.ttl(`cache::callsign:${randomFlightroute.callsign}`);
			// Minus 10 seconds just for a bit of leeway
			const seven_days = 60 * 60 * 24 * 7 - 10;
			expect(cache_ttl).toBeGreaterThanOrEqual(seven_days);
		});
	});

	describe(`ROUTE /callsign/:callsign `, () => {

		afterAll(async () => {
			await testHelper.redis.flushdb();
		});

		beforeEach(async () => {
			await testHelper.beforeEach();
		});

		it('GET return correct flightroute info', async () => {
			expect.assertions(1);
			const randomFlightroute = testHelper.randomFlightroute();
			const result = await testHelper.axios.get(`${testHelper.url_callsign}/${randomFlightroute.callsign}`);
			expect(result.data.response.flightroute).toMatchObject(randomFlightroute);
		});

		it('Flightroute data saved into cache', async () => {
			expect.assertions(1);
			const randomFlightroute = testHelper.randomFlightroute();
			await testHelper.axios.get(`${testHelper.url_callsign}/${randomFlightroute.callsign}`);
			const cached_callsign = await testHelper.redis.hget(`cache::callsign:${randomFlightroute.callsign}`, 'cached_data');
			const parsed_callsign = JSON.parse(cached_callsign);
			expect(parsed_callsign).toMatchObject(randomFlightroute);
		});

		it('GET Flightroute cache to have a ttl of 7 days', async () => {
			expect.assertions(1);
			const randomFlightroute = testHelper.randomFlightroute();
			await testHelper.axios.get(`${testHelper.url_callsign}/${randomFlightroute.callsign}`);
			const cache_ttl = await testHelper.redis.ttl(`cache::callsign:${randomFlightroute.callsign}`);
			// Minus 10 seconds just for a bit of leeway
			const seven_days = 60 * 60 * 24 * 7 - 10;
			expect(cache_ttl).toBeGreaterThanOrEqual(seven_days);
		});
	});

});
