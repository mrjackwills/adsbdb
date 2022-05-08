import { createLogger, format, Logform, LogEntry, transports } from 'winston';
import { postgresQueries } from '../lib/postgresQueries';
import { env } from '../const/env';
import { TLoggerColors, TLogLevels } from '../types';
import Transport from 'winston-transport';

const { errors, combine, timestamp, splat } = format;

const consoleLogFormatter = (info: Logform.TransformableInfo): string => {
	const level = info.level as TLogLevels;
	const bgColor: TLoggerColors = {
		debug: `\x1b[46m`, // green
		error: `\x1b[41m`, // red
		verbose: `\x1b[42m`, // cyan
		warn: `\x1b[43m` // yellow
	};
	const fgColor: TLoggerColors = {
		debug: `\x1b[36m`,
		error: `\x1b[31m`,
		verbose: `\x1b[32m`,
		warn: `\x1b[33m`
	};
	const bgBlack = `\x1b[40m`;
	const fgWhite = `\x1b[37m`;
	const fgBlack = `\x1b[30m`;
	let formattedString = `${fgBlack}${bgColor[level]}${info.level.toUpperCase().padEnd(7, ' ')}${bgBlack}${fgColor[level]}${info.timestamp.substring(10, 23)} `;
	if (info.log) formattedString += `\n${JSON.stringify(info.log)}`;
	formattedString += info.stack ? `${info.stack}` : `${ JSON.stringify(info.message)}` ;
	formattedString += info.uuid ? `\n${info.uuid}` : '';
	formattedString += fgWhite;
	return formattedString;
};

// Custom postgres transport class
class PostgresTransport extends Transport {
	constructor (opts: Transport.TransportStreamOptions) {
		super(opts);
	}

	async log (info: LogEntry, callback: () => void): Promise<void> {
		await postgresQueries.insert_error(info);
		callback();
	}
}

export const log = createLogger({
	level: 'info',
	format: combine(
		timestamp(),
		errors({ stack: true }),
		splat(),
		format.json()
	),
	transports: [
		new transports.File({ filename: env.LOCATION_LOG_ERROR, level: 'error' }),
		new transports.File({ filename: env.LOCATION_LOG_COMBINED }),
		new PostgresTransport({ level: 'verbose' })
	],
	exitOnError: false,
});

if (env.MODE_ENV_DEV || env.SHOW_LOGS && !env.MODE_ENV_TEST) {
	log.add(
		new transports.Console({
			handleExceptions: true,
			level: 'debug',
			format: combine(
				timestamp({ format: 'YYYY-MM-DD HH:mm:ss.SSS' }),
				errors({ stack: true }),
				splat(),
				format.printf((info) => consoleLogFormatter(info))
			),
		})
	);
}
