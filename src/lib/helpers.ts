import { Request } from 'express';
import { UUID } from 'types';
import { randomUUID } from 'crypto';

export const extractIp = (req: Request): string => {
	const x_real_ip = req.headers['x-real-ip'];
	const remoteAddress = req.socket.remoteAddress;
	return x_real_ip && typeof x_real_ip === 'string' ? String(x_real_ip) : String(remoteAddress) ?? 'UNKNOWN';
};

export const generateUUID = (): UUID => <UUID>randomUUID({ disableEntropyCache: true });