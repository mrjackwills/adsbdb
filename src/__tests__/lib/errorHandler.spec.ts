import httpMocks from 'node-mocks-http';
import { errorHandler } from '../../lib/errorHandler';
import { TestHelper } from '../testHelper';
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest';

const testHelper = new TestHelper();

describe('errorHandler', () => {
	const mocked_next = vi.fn();
	const mocked_error = { message: 'error from errorHandler.spec.ts' };
	const fakeReq = httpMocks.createRequest();

	beforeEach(async () => {
		mocked_next.mockReset();
	});

	afterAll(async () => {
		// Have to sleep due to the way errors are handled internally
		await testHelper.sleep();
		testHelper.afterAll();
	});

	it('Should call next when no error', () => {
		expect.assertions(1);
		const fakeRes = httpMocks.createResponse();
		errorHandler(null, fakeReq, fakeRes, mocked_next);
		expect(mocked_next).toHaveBeenCalled();
	});

	it(`Shouldn't call next when an error is passed`, () => {
		expect.assertions(1);
		const fakeRes = httpMocks.createResponse();
		errorHandler(mocked_error, fakeReq, fakeRes, mocked_next);
		expect(mocked_next).not.toHaveBeenCalled();
	});

	it(`Should return json server error 500 when error`, () => {
		expect.assertions(2);
		const fakeRes = httpMocks.createResponse();
		errorHandler(mocked_error, fakeReq, fakeRes, mocked_next);
		const jsonData = fakeRes._getJSONData();
		const statusCode = fakeRes.statusCode;
		expect(statusCode).toEqual(500);
		expect(jsonData.response).toMatch(testHelper.response_empty.response);
	});

});
