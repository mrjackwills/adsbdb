import { log } from '../config/log';
import { parse } from 'secure-json-parse';
import { Redis } from '../config/db_redis';
import { RedisKeys } from '../const/redis';
import { AircraftCacheKeyName, CallSign, CallsignCacheKeyName, ModeS, TCallsignQuery, TModeSQueryResult } from '../types';
import * as ioredis from 'ioredis';

const wrap = <T> () => function (_target: RedisQueries, propertyKey: string, descriptor: PropertyDescriptor): void {
	const original = descriptor.value;
	descriptor.value = async function (...args: Array<T>): Promise<unknown> {
		const start = Date.now();
		try {
			const result = await original.call(this, ...args);
			return result;
		} catch (e) {
			log.error(e);
			return;
		}
		finally {
			log.debug(`${propertyKey} wrap: ${Date.now()-start}ms`);
		}
	};
};

class RedisQueries {

	#db!: ioredis.Redis;
	#ONE_MINUTE = 60;
	#ONE_HOUR = this.#ONE_MINUTE* 60;
	#ONE_DAY = this.#ONE_HOUR * 24;
	
	constructor (db: ioredis.Redis) {
		this.#db = db;
	}

	#create_aircraft_key (x: ModeS): AircraftCacheKeyName {
		return <AircraftCacheKeyName>`${RedisKeys.MODE_S_CACHE}:${x}`;
	}

	#create_callsign_key (x: CallSign): CallsignCacheKeyName {
		return <CallsignCacheKeyName>`${RedisKeys.CALLSIGN_CACHE}:${x}`;
	}
	
	async #set_expire (key: AircraftCacheKeyName|CallsignCacheKeyName): Promise<void> {
		await this.#db.expire(key, this.#ONE_DAY * 7);
	}

	@wrap()
	async set_cache_unknown_aircraft (modeS: ModeS): Promise<void> {
		const key = this.#create_aircraft_key(modeS);
		await this.#db.hset(key, RedisKeys.UNKNOWN, RedisKeys.UNKNOWN_AIRCRAFT);
		await this.#set_expire(key);
	}

	@wrap()
	async get_aircraft_cache (modeS: ModeS): Promise<TModeSQueryResult|undefined> {
		const cached_data = await this.#db.hget(this.#create_aircraft_key(modeS), RedisKeys.CACHED_DATA);
		return cached_data ? parse(cached_data, undefined, { protoAction: 'remove', constructorAction: 'remove' }) : undefined;
	}

	@wrap()
	async has_aircraft_cache (modeS: ModeS): Promise<boolean> {
		const data = await this.#db.exists(this.#create_aircraft_key(modeS), RedisKeys.CACHED_DATA);
		return !!data;
	}

	@wrap()
	async set_aircraft_cache (modeS: ModeS, data: TModeSQueryResult): Promise<void> {
		const key = this.#create_aircraft_key(modeS);
		await this.#db.hset(key, RedisKeys.CACHED_DATA, JSON.stringify(data));
		await this.#set_expire(key);
	}

	@wrap()
	async set_cache_unknown_callsign (callsign: CallSign): Promise<void> {
		const key = this.#create_callsign_key(callsign);
		await this.#db.hset(key, RedisKeys.UNKNOWN, RedisKeys.UNKNOWN_CALLSIGN);
		await this.#set_expire(key);
	}

	@wrap()
	async get_callsign_cache (callsign: CallSign): Promise<TCallsignQuery|undefined> {
		const cached_data = await this.#db.hget(this.#create_callsign_key(callsign), RedisKeys.CACHED_DATA);
		return cached_data ? parse(cached_data, undefined, { protoAction: 'remove', constructorAction: 'remove' }) : undefined;
	}

	@wrap()
	async has_callsign_cache (callsign: CallSign): Promise<boolean> {
		const data = await this.#db.exists(this.#create_callsign_key(callsign), RedisKeys.CACHED_DATA);
		return !!data;
	}

	@wrap()
	async set_callsign_cache (callsign:CallSign, data: TCallsignQuery): Promise<void> {
		const key = this.#create_callsign_key(callsign);
		await this.#db.hset(key, RedisKeys.CACHED_DATA, JSON.stringify(data));
		await this.#set_expire(key);
	}
}

export const redisQueries = new RedisQueries(Redis);