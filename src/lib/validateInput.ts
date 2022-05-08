import { customTypeError } from '../config/customError';
import { ErrorMessages } from '../const/error';
import { httpCodes } from '../const/httpCode';
import { Schema, attempt, ValidationError } from 'joi';

export const validateInput = <T> (input: unknown, schema: Schema): T|undefined => {
	try {
		const validatedInput = attempt(input, schema);
		return validatedInput;
	} catch (e) {
		if (e instanceof ValidationError) {
			if (e._original?.password) e._original.password = null;
			const message = e.details[0]?.context?.label?? ErrorMessages.INVALID_DATA;
			const errorToThrow = customTypeError(message, httpCodes.BAD_REQUEST);
			throw errorToThrow;
		}
		return;
	}
};