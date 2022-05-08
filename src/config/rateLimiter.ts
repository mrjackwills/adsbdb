import { customError } from './customError';
import { ErrorMessages } from '../const/error';
import { extractIp } from '../lib/helpers';
import { httpCodes } from '../const/httpCode';
import { env } from '../const/env';
import { RateLimiterRedis, IRateLimiterStoreOptions, RateLimiterRes } from 'rate-limiter-flexible';
import { Redis } from '../config/db_redis';
import { RedisKeys } from '../const/redis';
import { RequestHandler } from 'express';

const redisOpts: IRateLimiterStoreOptions= {
	keyPrefix: RedisKeys.LIMITER,
	storeClient: Redis,
	points: 600,
	duration: 60,
	// 5 minute block
	blockDuration: 60 * 5,
	execEvenly: false,
	// inmemoryBlockOnConsumed: 60 * 12,
	// inmemoryBlockDuration: 60 * 60 * 6,
};
export const limiter = new RateLimiterRedis(redisOpts);

const sharedLimiter = async (key: string, points: number): Promise<void> => {
	const currentPoints = await limiter.get(key);
	if (currentPoints && redisOpts.points && currentPoints.consumedPoints >= redisOpts.points * 6) {
		await limiter.block(key, 60 * 15);
		await limiter.penalty(key, 360);
	}
	await limiter.consume(key, points);
};

export const rateLimiter: RequestHandler = async (req, _res, next): Promise<void> => {
	try {
		if (env.MODE_ENV_TEST && !process.env.limitTest) return next();
		const key = extractIp(req);
		await sharedLimiter(key, 1);
		next();
	} catch (e) {
		const message = e instanceof RateLimiterRes ? e.msBeforeNext : ErrorMessages.INTERNAL;
		const code = e instanceof RateLimiterRes && e.msBeforeNext? httpCodes.TOO_MANY_REQUESTS : httpCodes.INTERNAL_SERVER_ERROR;
		const error = customError(code, message);
		next(error);
	}
};
