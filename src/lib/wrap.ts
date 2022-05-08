import { RequestHandler } from 'express';

export const wrap = (fn:RequestHandler): RequestHandler => async function (req, res, next): Promise<void> {
	try {
		await fn(req, res, next);
	} catch (e) {
		next(e);
	}
};