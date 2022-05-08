import { log } from '../../config/log';
import { TestHelper } from '../testHelper';
import { afterAll, describe, expect, it } from 'vitest';

const testHelper = new TestHelper();

describe('log test runner', () => {

	afterAll(async () => testHelper.afterAll());

	it('Expect verbose log to increase in db', async () => {
		expect.assertions(1);
		const randomMessage = await testHelper.randomHex();
		const latestId = await testHelper.query_selectErrorLatest();
		log.verbose(randomMessage);
		await testHelper.sleep();
		const result = await testHelper.query_selectError(randomMessage);
		expect(Number(result.error_log_id)).toBeGreaterThan(Number(latestId?.error_log_id||0));
	});

	it('Expect warn log to increase in db', async () => {
		expect.assertions(1);
		const randomMessage = await testHelper.randomHex();
		const latestId = await testHelper.query_selectErrorLatest();
		log.warn(randomMessage);
		await testHelper.sleep();
		const result = await testHelper.query_selectError(randomMessage);
		expect(Number(result.error_log_id)).toBeGreaterThan(Number(latestId?.error_log_id||0));
	});

	it('Expect error log to increase in db', async () => {
		expect.assertions(1);
		const randomMessage = await testHelper.randomHex();
		const latestId = await testHelper.query_selectErrorLatest();
		log.error(randomMessage);
		await testHelper.sleep();
		const result = await testHelper.query_selectError(randomMessage);
		expect(Number(result.error_log_id)).toBeGreaterThan(Number(latestId?.error_log_id||0));
	});

	it('Expect error log to increase in db', async () => {
		expect.assertions(8);
		const randomError = `${Date.now()}`;
		const now = Date.now();
		try {
			throw Error(randomError);
		} catch (e) {
			log.error(e);
		}
		
		// Sleep required due to the way errors are handled
		await testHelper.sleep();

		const selectResult = await testHelper.query_selectError(randomError);
		expect(selectResult).toBeDefined();
		expect(isNaN(Number(selectResult.error_log_id))).toBeFalsy();
		expect(Number(selectResult.error_log_id)).toBeGreaterThan(1);
		expect(selectResult.level).toStrictEqual('error');
		expect(selectResult.message).toStrictEqual(randomError);
		expect(selectResult.uuid).toBeNull();
		expect(selectResult.http_code).toBeNull();
		expect(now - selectResult.timestamp.getTime()).toBeLessThan(50);
	});
});