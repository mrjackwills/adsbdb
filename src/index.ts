import { api } from './app/api';
import { env } from './const/env';
import { handleProcessExit } from './lib/exitProcess';
import { log } from './config/log';
import http from 'http';

const __main__ = async (): Promise<void> => {
	await handleProcessExit();
	const server = http.createServer(api);
	server.listen(env.API_PORT, env.API_HOST, () => log.verbose(`${env.APP_NAME} ${env.DOMAIN} :${env.API_PORT} api server started`));
};

__main__();