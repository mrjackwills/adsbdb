import { env } from '../const/env';
import { Pool } from 'pg';

// types.setTypeParser(20, BigInt)

// const parseBigIntArray = types.getTypeParser(1016);
// types.setTypeParser(1016, a => parseBigIntArray(a).map(BigInt));

export const postgresql = new Pool({
	user: env.PG_USER,
	host: env.PG_HOST,
	database: env.PG_DATABASE,
	password: env.PG_PASS,
	port: env.PG_PORT,
	max: 20,
	idleTimeoutMillis: 30000,
	connectionTimeoutMillis: 2000,
});
