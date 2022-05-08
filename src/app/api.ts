import { env } from '../const/env';
import { corsAsync } from '../config/cors';
import { errorHandler } from '../lib/errorHandler';
import { Rest } from '../config/rest';

import cors from 'cors';
import express from 'express';
import morgan from 'morgan';

const api = express();

if (env.MODE_ENV_DEV || env.SHOW_LOGS && !env.MODE_ENV_TEST) api.use(morgan('dev'));
api.enable('trust proxy');
api.use(cors(corsAsync));

api.use(express.json());
api.use(express.urlencoded({
	extended: true
}));
api.use('/', Rest);
api.use(errorHandler);

export { api };
