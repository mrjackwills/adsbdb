import { postgresql } from '../config/db_postgres';
import { Redis } from '../config/db_redis';

const exit = async (): Promise<void> => {
	try {
		Redis.disconnect();
		postgresql.end();
	} finally {
		process.exit();
	}
};

export const handleProcessExit = async (): Promise<void> => {
	process.stdin.resume();
	process.on('exit', async () => exit());
	process.on('SIGINT', () => exit());
	process.on('SIGUSR1', () => exit());
	process.on('SIGUSR2', () => exit());
	process.on('uncaughtException', () => exit());
};
