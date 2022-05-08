import { isCallSign, isModeS } from '../../types/typeGuard';
import { TestHelper } from '../testHelper';

import { describe, expect, it } from 'vitest';

const testHelper = new TestHelper();

describe('typeguards runner', () => {

	describe('isCallSign runner', () => {
	
		it('random uppercase hex of 4-8 length returns true', async () => {
			expect.assertions(1);
			const randomNumber = testHelper.randomNumber(4, 8);
			const randomString = await testHelper.randomHex(randomNumber);
			const callsign = isCallSign(randomString.toUpperCase());
			expect(callsign).toBeTruthy();
		});

		it('random uppercase hex of 4-8 length returns true', async () => {
			expect.assertions(1);
			const randomNumber = testHelper.randomNumber(4, 8);
			const randomString = await testHelper.randomHex(randomNumber);
			const callsign = isCallSign(randomString.toUpperCase());
			expect(callsign).toBeTruthy();
		});

		it('random hex of 9+ length returns false', async () => {
			expect.assertions(1);
			const randomNumber = testHelper.randomNumber(9, 20);
			const randomString = await testHelper.randomHex(randomNumber);
			const callsign = isCallSign(randomString);
			expect(callsign).toBeFalsy();
		});

		it.concurrent('empty string callsign returns false', async () => {
			expect.assertions(1);
			const callsign = isCallSign('');
			expect(callsign).toBeFalsy();
		});

		it.concurrent('null callsign returns false', async () => {
			expect.assertions(1);
			const callsign = isCallSign(null);
			expect(callsign).toBeFalsy();
		});

		it.concurrent('random boolean callsign returns false', async () => {
			expect.assertions(1);
			const callsign = isCallSign(testHelper.randomBoolean);
			expect(callsign).toBeFalsy();
		});
	});

	describe('isModeS runner', () => {
	
		it('random hex of 6 length returns true', async () => {
			expect.assertions(1);
			const randomString = await testHelper.randomHex(6);
			const callsign = isModeS(randomString);
			expect(callsign).toBeTruthy();
		});

		it('random uppercase hex of 6 length returns true', async () => {
			expect.assertions(1);
			const randomString = await testHelper.randomHex(6);
			const callsign = isModeS(randomString.toUpperCase());
			expect(callsign).toBeTruthy();
		});

		it('random hex of  > 6 length returns false', async () => {
			expect.assertions(1);
			const randomNumber = testHelper.randomNumber(7, 100);
			const randomString = await testHelper.randomHex(randomNumber);
			const callsign = isModeS(randomString);
			expect(callsign).toBeFalsy();
		});

		it('none hex 6 length string returns false', async () => {
			expect.assertions(1);
			const randomString = await testHelper.randomHex(5);
			const callsign = isModeS(`${randomString}z`);
			expect(callsign).toBeFalsy();
		});

		it.concurrent('empty string callsign returns false', async () => {
			expect.assertions(1);
			const callsign = isModeS('');
			expect(callsign).toBeFalsy();
		});

		it.concurrent('null callsign returns false', async () => {
			expect.assertions(1);
			const callsign = isModeS(null);
			expect(callsign).toBeFalsy();
		});

		it.concurrent('random boolean callsign returns false', async () => {
			expect.assertions(1);
			const callsign = isModeS(testHelper.randomBoolean);
			expect(callsign).toBeFalsy();
		});
	});
});