import { httpCodes } from '../const/httpCode';
import { TFsend } from '../types';

export const Send: TFsend = async ({ res, response = '', status = httpCodes.OK }): Promise<void> => {
	res.status(status).json({ response });
};