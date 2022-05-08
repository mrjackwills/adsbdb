import { TestHelper } from './testHelper';
import format from 'pg-format';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const testHelper = new TestHelper();

describe('Test api init', () => {

	afterAll(async () => testHelper.afterAll());
	beforeAll(async () => testHelper.beforeAll());

	describe(`Valid postgres connection`, () => {

		it('should have name dev_adsbdb', async () => {
			expect.assertions(1);
			const query = format(`SELECT current_database()`);
			const { rows } = await testHelper.postgres.query(query);
			expect(rows[0]).toEqual({ current_database: 'dev_adsbdb' });
		});

	});

	describe(`Valid redis connection`, () => {

		it('return PONG from a PING command', async () => {
			expect.assertions(1);
			const pong = await testHelper.redis.ping();
			expect(pong).toEqual('PONG');

		});
	});

	describe(`ROUTE - /:random`, () => {

		it('GET should return empty response status 404', async () => {
			expect.assertions(2);
			const random = Date.now();
			try {
				await testHelper.axios.get(`/${random}`);
			} catch (err) {
				const e = testHelper.axiosE(err);
				expect(e.response?.status).toEqual(404);
				expect(e.response?.data).toEqual({ response: 'unknown endpoint' });
		
			}
		
		});
	});
});