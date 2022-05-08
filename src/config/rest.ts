import { env } from '../const/env';
import { customError } from './customError';
import { ErrorMessages } from '../const/error';
import { get_online, get_modeS, get_callsign } from '../components/api_controller';
import { httpCodes } from '../const/httpCode';
import { rateLimiter } from '../config/rateLimiter';
import { RequestHandler, Router } from 'express';
import { routes } from '../const/route';
import { wrap } from '../lib/wrap';

const notFound: RequestHandler = (_req, _res) => {
	throw customError(httpCodes.NOT_FOUND, ErrorMessages.UNKNOWN_ENDPOINT);
};

const prefixVersion = (route: routes): string => `/v${env.API_MAJOR_VERSION}${route}`;

const Rest = Router({ mergeParams: true, strict: true });

Rest.use(rateLimiter);

Rest.get(prefixVersion(routes.AIRCRAFT_PARAM_MODE_S), wrap(get_modeS));

Rest.get(prefixVersion(routes.CALLSIGN_PARAM_CALLSIGN), wrap(get_callsign));

Rest.get(prefixVersion(routes.ONLINE), wrap(get_online));

Rest.all(routes.CATCHALL, notFound);

export { Rest };
