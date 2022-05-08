import { ErrorMessages } from '../const/error';
import { ErrorRequestHandler } from 'express';
import { generateUUID } from './helpers';
import { httpCodes } from '../const/httpCode';
import { log } from '../config/log';
import { Send } from './send';

/**
 ** Global error handler
 */
export const errorHandler: ErrorRequestHandler = (e, _req, res, next): void => {
	if (e) {
		e.uuid = generateUUID();
		log.debug(e);
		if (e instanceof SyntaxError && Object.prototype.hasOwnProperty.call(e, 'body')) {
			Send({ res, response: ErrorMessages.MALFORMED_JSON, status: httpCodes.BAD_REQUEST });
		}
		else if (e.httpCode) {
			switch (e.httpCode) {
			case httpCodes.NOT_FOUND: {
				const message = e.message ?? ErrorMessages.UNKNOWN_ENDPOINT;
				Send({ res, status: httpCodes.NOT_FOUND, response: message });
				break;
			}
			case httpCodes.BAD_REQUEST: {
				const message = e.message ?? ErrorMessages.INVALID_DATA;
				Send({ res, response: message, status: httpCodes.BAD_REQUEST });
				break;
			}
			case httpCodes.PAYLOAD_TOO_LARGE: {
				Send({ res, status: httpCodes.PAYLOAD_TOO_LARGE });
				break;
			}
			case httpCodes.TOO_MANY_REQUESTS: {
				Send({ res, status: httpCodes.TOO_MANY_REQUESTS });
				break;
			}
			default: {
				Send({ res, status: httpCodes.BAD_REQUEST });
				break;
			}
			}
		} else {
			log.error(e);
			const message = `${ErrorMessages.INTERNAL}: ${e.uuid}`;
			Send({ res, status: httpCodes.INTERNAL_SERVER_ERROR, response: message });
		}
	} else {
		next();
	}
};