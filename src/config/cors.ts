import { CorsOptions, CorsOptionsDelegate } from 'cors';

export const corsAsync: CorsOptionsDelegate = async (_req, next): Promise<void> => {
	const corsOptions: CorsOptions = {
		credentials: true,
		origin: true
	};
	next(null, corsOptions);
};