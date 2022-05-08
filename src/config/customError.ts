import { ErrorMessages } from '../const/error';
import { httpCodes } from '../const/httpCode';

class ErrorWithStatus extends Error {
	constructor (public httpCode: httpCodes, public message: string) {
		super();
	}
}

class TypeErrorWithStatus extends TypeError {
	constructor (public httpCode: httpCodes, public message: string) {
		super();
	}
}

export const customError = (httpCode?: httpCodes, message?: ErrorMessages | number): ErrorWithStatus => {
	const errorCode = httpCode ?? httpCodes.INTERNAL_SERVER_ERROR;
	const errorMessage = message ?? ErrorMessages.INTERNAL;
	return new ErrorWithStatus(errorCode, String(errorMessage));
};

export const customTypeError = (message: string, httpCode?: httpCodes): TypeErrorWithStatus => {
	const errorCode = httpCode ?? httpCodes.INTERNAL_SERVER_ERROR;
	const errorMessage = message ?? ErrorMessages.TYPE;
	return new TypeErrorWithStatus(errorCode, errorMessage);
};