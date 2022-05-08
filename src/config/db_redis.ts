import { log } from './log';
import { env } from '../const/env';
import ioredis, { RedisOptions } from 'ioredis';

const redisOptions: RedisOptions = {
	port: env.REDIS_PORT,
	host: env.REDIS_HOST,
	family: 4,
	password: env.REDIS_PASSWORD,
	db: env.REDIS_DATABASE,
	retryStrategy (times) {
		const delay = 3000;
		if (times === 20) process.exit();
		return delay;
	},
};

const Redis = new ioredis(redisOptions);

Redis.on('connect', () => log.verbose(`redis connected [${redisOptions.db}] @ redis://${redisOptions.host}:${redisOptions.port}`)) ;
Redis.on('error', (e) => log.error(e, { log: 'redis connection error' }));

export { Redis };