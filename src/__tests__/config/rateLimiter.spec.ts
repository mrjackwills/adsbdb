import { TestHelper } from '../testHelper';
import { afterAll, beforeAll, beforeEach, describe, expect, it } from 'vitest';

const testHelper = new TestHelper();

describe('RateLimit using testHelper', () => {
	
	const injectEnv = (): void => {
		process.env.limitTest = 'true';
	};
	
	const multipleRequest = async (numberOfRequests: number): Promise<void> => {
		for (const _i of new Array(numberOfRequests)) {
			try {
				/* eslint-disable no-await-in-loop */
				await testHelper.sleep(2);
				/* eslint-disable no-await-in-loop */
				await testHelper.axios.get(testHelper.url_online);
			} catch (e) {
				// console.log(e)
			}
		}
	};

	beforeAll(async () => testHelper.beforeAll());

	beforeEach(async () => {
		await testHelper.redis.flushdb();
		injectEnv();
	});

	afterAll(async () => testHelper.afterAll());

	it('req object IP address limiter 429 response, 60 second block, after 600 requests', async () => {
		expect.assertions(3);
		injectEnv();
		try {
			await multipleRequest(600);
			await testHelper.axios.get(testHelper.url_online);
		} catch (err) {
			const e = testHelper.axiosE(err);
			expect(e.response?.data).toEqual(testHelper.response_empty);
			expect(e.response?.status).toEqual(429);
		}
		const numberPoints = await testHelper.redis.get(`limiter:127.0.0.1`);
		expect(Number(numberPoints)).toBeGreaterThanOrEqual(600);
	});

});