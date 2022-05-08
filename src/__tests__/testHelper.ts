/* eslint-disable @typescript-eslint/ban-types */
import { api } from '../app/api';
import { postgresql } from '../config/db_postgres';
import { randomBytes } from 'crypto';
import { Redis } from '../config/db_redis';
// import { RedisKeys } from '../types/enum_redis';
import { TErrorLog, TCallsignQuery, CallSign } from '../types';
import Axios, { AxiosError, AxiosInstance } from 'axios';
import format from 'pg-format';
import http from 'http';
import { axios_service } from '../lib/axios';
import { page_html } from './scraper_html';

import { vi } from 'vitest';

vi.mock('../lib/axios');

type test_aircraft = { [ K in
	'mode_s' |
	'registered_owner' |
	'registered_owner_operator_flag_code' |
	'registered_owner_country_name' |
	'registered_owner_country_iso_name' |
	'manufacturer' |
	'type' |
	'icao_type' |
	'url_photo' |
	'url_photo_thumbnail']: string }
	
abstract class Constants {

	readonly mockedAxiosAircraftPhoto = vi.mocked(axios_service.get_photo);
	readonly mockedAxiosIcaoScraper = vi.mocked(axios_service.get_flightroute);
	readonly mocked_flightroute_param = 'RYR544';
	readonly mocked_photo_param ='400A0B';

	readonly #v = '/v0';
	readonly url_online = `${this.#v}/online`;
	readonly url_aircraft = `${this.#v}/aircraft`;
	readonly url_callsign = `${this.#v}/callsign`;
	
	readonly scraper_html_string = page_html;
	readonly axios_port = 9899;
	readonly response_empty = { response: '' };
	readonly response_unknown = { response: 'unknown endpoint' };
	readonly response_invalid_modeS = { response: 'Aircraft modeS string invalid' };
	readonly internalError = 'Internal server error';
	readonly logErrorMessage = 'jest_error_test';
	readonly semver_regex = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;
}

abstract class Aircraft extends Constants {

	readonly known_aircraft: Array<test_aircraft> = [
		{
			mode_s: 'A7E152',
			registered_owner: 'Delta Air Lines',
			registered_owner_operator_flag_code: 'DAL',
			registered_owner_country_name: 'United States',
			registered_owner_country_iso_name: 'US',
			manufacturer: 'Boeing',
			type: '717 231',
			icao_type: 'B712',
			url_photo: '/001/637/001637043.jpg',
			url_photo_thumbnail: '//001/637/001637043.jpg'
		}, {
			mode_s: '4247E3',
			registered_owner: 'ExecuJet Australia P/L',
			registered_owner_operator_flag_code: 'GLEX',
			registered_owner_country_name: 'Cayman Islands',
			registered_owner_country_iso_name: 'KY',
			manufacturer: 'Bombardier',
			type: 'Global Express XRS',
			icao_type: 'GLEX',
			url_photo: '',
			url_photo_thumbnail: ''
		},
		{
			mode_s: 'A44F3B',
			registered_owner: 'NetJets',
			registered_owner_operator_flag_code: 'EJA',
			registered_owner_country_name: 'United States',
			registered_owner_country_iso_name: 'US',
			manufacturer: 'Cessna',
			type: 'Citation Sovereign',
			icao_type: 'C680',
			url_photo: '/001/572/001572354.jpg',
			url_photo_thumbnail: '//001/572/001572354.jpg'
		},
		{
			mode_s: '009D80',
			registered_owner: 'Dadas World of Hardware CC',
			registered_owner_operator_flag_code: 'C500',
			registered_owner_country_name: 'South Africa',
			registered_owner_country_iso_name: 'ZA',
			manufacturer: 'Cessna',
			type: 'Citation I',
			icao_type: 'C500',
			url_photo: '',
			url_photo_thumbnail: ''
		},
		{
			mode_s: 'AA886A',
			registered_owner: 'West Star Construction Inc',
			registered_owner_operator_flag_code: 'C55B',
			registered_owner_country_name: 'United States',
			registered_owner_country_iso_name: 'US',
			manufacturer: 'Cessna',
			type: 'Citation Bravo',
			icao_type: 'C55B',
			url_photo: '',
			url_photo_thumbnail: ''
		},
		{
			mode_s: 'A69B96',
			registered_owner: 'D & D Aviation LLC',
			registered_owner_operator_flag_code: 'C525',
			registered_owner_country_name: 'United States',
			registered_owner_country_iso_name: 'US',
			manufacturer: 'Cessna',
			type: 'Citation CJ1',
			icao_type: 'C525',
			url_photo: '',
			url_photo_thumbnail: ''
		},
		{
			mode_s: '40622B',
			registered_owner: 'easyJet Airline',
			registered_owner_operator_flag_code: 'EZY',
			registered_owner_country_name: 'United Kingdom',
			registered_owner_country_iso_name: 'GB',
			manufacturer: 'Airbus',
			type: 'A320 214',
			icao_type: 'A320',
			url_photo: '',
			url_photo_thumbnail: '',
		},
		{
			mode_s: '7816F5',
			registered_owner: 'Air China',
			registered_owner_operator_flag_code: 'CCA',
			registered_owner_country_name: 'China',
			registered_owner_country_iso_name: 'CN',
			manufacturer: 'Airbus',
			type: 'A320 271NSL',
			icao_type: 'A20N',
			url_photo: '',
			url_photo_thumbnail: ''
		},
		{
			mode_s: '87C841',
			registered_owner: 'Japan Maritime Self-Defense Force',
			registered_owner_operator_flag_code: 'P1',
			registered_owner_country_name: 'Japan',
			registered_owner_country_iso_name: 'JP',
			manufacturer: 'Kawasaki',
			type: 'P-1',
			icao_type: 'P1',
			url_photo: '',
			url_photo_thumbnail: ''
		},
		{
			mode_s: '7342A2',
			registered_owner: 'Corporate',
			registered_owner_operator_flag_code: 'CL64',
			registered_owner_country_name: 'Iran, Islamic Republic of',
			registered_owner_country_iso_name: 'IR',
			manufacturer: 'Bombardier',
			type: 'Challenger 604',
			icao_type: 'CL64',
			url_photo: '',
			url_photo_thumbnail: ''
		}
	];
}

abstract class FlightRoutes extends Aircraft {
	readonly known_flightroutes: Array<TCallsignQuery> = [
		{
			callsign: <CallSign>'TOM35MR',
			origin_airport_country_name: 'Spain',
			origin_airport_country_iso_name: 'ES',
			origin_airport_municipality: 'Palma De Mallorca',
			origin_airport_icao_code: 'LEPA',
			origin_airport_iata_code: 'PMI',
			origin_airport_name: 'Palma de Mallorca Airport',
			origin_airport_elevation: 27,
			origin_airport_latitude: '39.551701',
			origin_airport_longitude: '2.73881',
			destination_airport_country_name: 'United Kingdom',
			destination_airport_country_iso_name: 'GB',
			destination_airport_municipality: 'Bristol',
			destination_airport_icao_code: 'EGGD',
			destination_airport_iata_code: 'BRS',
			destination_airport_name: 'Bristol Airport',
			destination_airport_elevation: 622,
			destination_airport_latitude: '51.382702',
			destination_airport_longitude: '-2.71909'
		},
		{
			callsign: <CallSign>'FDX1624',
			origin_airport_country_name: 'United States',
			origin_airport_country_iso_name: 'US',
			origin_airport_municipality: 'Greensboro',
			origin_airport_icao_code: 'KGSO',
			origin_airport_iata_code: 'GSO',
			origin_airport_name: 'Piedmont Triad International Airport',
			origin_airport_elevation: 925,
			origin_airport_latitude: '36.097801',
			origin_airport_longitude: '-79.937302',
			destination_airport_country_name: 'United States',
			destination_airport_country_iso_name: 'US',
			destination_airport_municipality: 'Indianapolis',
			destination_airport_icao_code: 'KIND',
			destination_airport_iata_code: 'IND',
			destination_airport_name: 'Indianapolis International Airport',
			destination_airport_elevation: 797,
			destination_airport_latitude: '39.7173',
			destination_airport_longitude: '-86.294403'
		},
		{
			callsign: <CallSign>'LOT3934',
			origin_airport_country_name: 'Poland',
			origin_airport_country_iso_name: 'PL',
			origin_airport_municipality: 'Goleniow',
			origin_airport_icao_code: 'EPSC',
			origin_airport_iata_code: 'SZZ',
			origin_airport_name: 'Szczecin-Goleniów "Solidarność" Airport',
			origin_airport_elevation: 154,
			origin_airport_latitude: '53.584702',
			origin_airport_longitude: '14.9022',
			destination_airport_country_name: 'Poland',
			destination_airport_country_iso_name: 'PL',
			destination_airport_municipality: 'Warsaw',
			destination_airport_icao_code: 'EPWA',
			destination_airport_iata_code: 'WAW',
			destination_airport_name: 'Warsaw Chopin Airport',
			destination_airport_elevation: 362,
			destination_airport_latitude: '52.1656990051',
			destination_airport_longitude: '20.967100143399996'
		},
		{
			callsign: <CallSign>'AAR786',
			origin_airport_country_name: 'Austria',
			origin_airport_country_iso_name: 'AT',
			origin_airport_municipality: 'Vienna',
			origin_airport_icao_code: 'LOWW',
			origin_airport_iata_code: 'VIE',
			origin_airport_name: 'Vienna International Airport',
			origin_airport_elevation: 600,
			origin_airport_latitude: '48.110298',
			origin_airport_longitude: '16.5697',
			destination_airport_country_name: 'Korea, Republic of',
			destination_airport_country_iso_name: 'KR',
			destination_airport_municipality: 'Seoul',
			destination_airport_icao_code: 'RKSI',
			destination_airport_iata_code: 'ICN',
			destination_airport_name: 'Incheon International Airport',
			destination_airport_elevation: 23,
			destination_airport_latitude: '37.46910095214844',
			destination_airport_longitude: '126.45099639892578'
		},
		{
			callsign: <CallSign>'DAL04',
			origin_airport_country_name: 'United Kingdom',
			origin_airport_country_iso_name: 'GB',
			origin_airport_municipality: 'London',
			origin_airport_icao_code: 'EGKK',
			origin_airport_iata_code: 'LGW',
			origin_airport_name: 'London Gatwick Airport',
			origin_airport_elevation: 202,
			origin_airport_latitude: '51.148102',
			origin_airport_longitude: '-0.190278',
			destination_airport_country_name: 'United States',
			destination_airport_country_iso_name: 'US',
			destination_airport_municipality: 'New York',
			destination_airport_icao_code: 'KJFK',
			destination_airport_iata_code: 'JFK',
			destination_airport_name: 'John F Kennedy International Airport',
			destination_airport_elevation: 13,
			destination_airport_latitude: '40.639801',
			destination_airport_longitude: '-73.7789'
		},
		{
			callsign: <CallSign>'CFG6010',
			origin_airport_country_name: 'Germany',
			origin_airport_country_iso_name: 'DE',
			origin_airport_municipality: 'Düsseldorf',
			origin_airport_icao_code: 'EDDL',
			origin_airport_iata_code: 'DUS',
			origin_airport_name: 'Düsseldorf Airport',
			origin_airport_elevation: 147,
			origin_airport_latitude: '51.289501',
			origin_airport_longitude: '6.76678',
			destination_airport_country_name: 'Spain',
			destination_airport_country_iso_name: 'ES',
			destination_airport_municipality: 'Palma De Mallorca',
			destination_airport_icao_code: 'LEPA',
			destination_airport_iata_code: 'PMI',
			destination_airport_name: 'Palma de Mallorca Airport',
			destination_airport_elevation: 27,
			destination_airport_latitude: '39.551701',
			destination_airport_longitude: '2.73881'
		},
		{
			callsign: <CallSign>'FDX2369',
			origin_airport_country_name: 'United States',
			origin_airport_country_iso_name: 'US',
			origin_airport_municipality: 'Los Angeles',
			origin_airport_icao_code: 'KLAX',
			origin_airport_iata_code: 'LAX',
			origin_airport_name: 'Los Angeles International Airport',
			origin_airport_elevation: 125,
			origin_airport_latitude: '33.942501',
			origin_airport_longitude: '-118.407997',
			destination_airport_country_name: 'United States',
			destination_airport_country_iso_name: 'US',
			destination_airport_municipality: 'Memphis',
			destination_airport_icao_code: 'KMEM',
			destination_airport_iata_code: 'MEM',
			destination_airport_name: 'Memphis International Airport',
			destination_airport_elevation: 341,
			destination_airport_latitude: '35.04240036010742',
			destination_airport_longitude: '-89.97669982910156'
		},
	
		{
			callsign: <CallSign>'UPS636',
			origin_airport_country_name: 'United States',
			origin_airport_country_iso_name: 'US',
			origin_airport_municipality: 'Louisville',
			origin_airport_icao_code: 'KSDF',
			origin_airport_iata_code: 'SDF',
			origin_airport_name: 'Louisville Muhammad Ali International Airport',
			origin_airport_elevation: 501,
			origin_airport_latitude: '38.1744',
			origin_airport_longitude: '-85.736',
			destination_airport_country_name: 'United States',
			destination_airport_country_iso_name: 'US',
			destination_airport_municipality: 'St Louis',
			destination_airport_icao_code: 'KSTL',
			destination_airport_iata_code: 'STL',
			destination_airport_name: 'St Louis Lambert International Airport',
			destination_airport_elevation: 618,
			destination_airport_latitude: '38.748697',
			destination_airport_longitude: '-90.370003'
		},
	
		{
			callsign: <CallSign>'NAX32U',
			origin_airport_country_name: 'Norway',
			origin_airport_country_iso_name: 'NO',
			origin_airport_municipality: 'Ålesund',
			origin_airport_icao_code: 'ENAL',
			origin_airport_iata_code: 'AES',
			origin_airport_name: 'Ålesund Airport, Vigra',
			origin_airport_elevation: 69,
			origin_airport_latitude: '62.5625',
			origin_airport_longitude: '6.1197',
			destination_airport_country_name: 'Spain',
			destination_airport_country_iso_name: 'ES',
			destination_airport_municipality: 'Alicante',
			destination_airport_icao_code: 'LEAL',
			destination_airport_iata_code: 'ALC',
			destination_airport_name: 'Alicante-Elche Miguel Hernández Airport',
			destination_airport_elevation: 142,
			destination_airport_latitude: '38.2822',
			destination_airport_longitude: '-0.558156'
		},
		{
			callsign: <CallSign>'EIN949',
			origin_airport_country_name: 'Spain',
			origin_airport_country_iso_name: 'ES',
			origin_airport_municipality: 'Málaga',
			origin_airport_icao_code: 'LEMG',
			origin_airport_iata_code: 'AGP',
			origin_airport_name: 'Málaga-Costa del Sol Airport',
			origin_airport_elevation: 53,
			origin_airport_latitude: '36.6749',
			origin_airport_longitude: '-4.49911',
			destination_airport_country_name: 'United Kingdom',
			destination_airport_country_iso_name: 'GB',
			destination_airport_municipality: 'Belfast',
			destination_airport_icao_code: 'EGAA',
			destination_airport_iata_code: 'BFS',
			destination_airport_name: 'Belfast International Airport',
			destination_airport_elevation: 268,
			destination_airport_latitude: '54.6575012207',
			destination_airport_longitude: '-6.2158298492399995'
		}
	];
}

abstract class Helpers extends FlightRoutes {

	sleep (ms = 250): Promise<void> {
		return new Promise((resolve) => setTimeout(resolve, ms));
	}

	randomNumber (min=1, max=1000): number {
		return Math.floor(Math.random() * (max - min) + min);
	}

	randomAircraft (): test_aircraft {
		return this.known_aircraft[Math.floor(Math.random() * this.known_aircraft.length)];
	}

	randomAircraft_noPhoto (): test_aircraft {
		const aircraft = this.randomAircraft();
		if (aircraft.url_photo) return this.randomAircraft_noPhoto();
		else return aircraft;
	}

	randomFlightroute (): TCallsignQuery {
		return this.known_flightroutes[Math.floor(Math.random() * this.known_flightroutes.length)];
	}

	async randomHex (num=32): Promise<string> {
		return new Promise((resolve, reject) => {
			randomBytes(num, (e, buff) => {
				if (e) reject(e);
				resolve(buff.toString('hex').substring(0, num));
			});
		});
	}

	get randomBoolean (): boolean {
		return Math.random() > .5;
	}
}

abstract class Server extends Helpers {

	postgres = postgresql;
	redis = Redis;

	#server?: http.Server;

	createServer (): void {
		if (this.#server) this.closeSever();
		this.#server = http.createServer(api);
		this.#server.listen(this.axios_port, '127.0.0.1', () => undefined);
	}

	closeSever (): void {
		this.#server?.close();
	}
}

abstract class BaseAxios extends Server {

	axios!: AxiosInstance;
	
	constructor () {
		super();
		this.axios = Axios.create({
			baseURL: `http://127.0.0.1:${this.axios_port}`,
			
			withCredentials: true,
			headers: {
				'Accept': 'application/json',
				'Content-Type': 'application/json; charset=utf-8',
				'Cache-control': 'no-cache'
			},
		});
	
		this.axios.interceptors.response.use(
			(config) => Promise.resolve(config),
			(error) => Promise.reject(error)
		);
	}

	axiosE (e: unknown): AxiosError {
		return <AxiosError>e;
	}
}

abstract class Queries extends BaseAxios {
	async query_selectErrorLatest (): Promise<TErrorLog> {
		const email_log_query = format('SELECT* FROM error_log ORDER BY timestamp DESC LIMIT 1');
		const { rows } = await this.postgres.query(email_log_query);
		return <TErrorLog>rows[0];
	}

	async query_selectError (message: string): Promise<TErrorLog> {
		const email_log_query = format('SELECT * FROM error_log WHERE message = %1$L', message);
		const { rows } = await this.postgres.query(email_log_query);
		return <TErrorLog>rows[0];
	}

	async cleanDB (): Promise<void> {
		const Client = await this.postgres.connect();
		try {
			await Client.query('BEGIN');
			const errorQuery = format(`DELETE FROM error_log WHERE timestamp >= NOW() - INTERVAL '5 minutes'`);
			await Client.query(errorQuery);
			await Client.query('COMMIT');
			await this.redis.flushdb();
		} catch (e) {
			await Client.query('ROLLBACK');
			throw e;
		} finally {
			Client.release();
		}
	}
}
export class TestHelper extends Queries {

	async beforeEach (): Promise<void> {
		await this.redis.flushdb();
		vi.clearAllMocks();
		this.mockedAxiosAircraftPhoto.mockImplementation(async (x) => {
			if (x !== this.mocked_photo_param) return;
			return {
				status: 200,
				count: 1,
				data: [
					{
						image: '/001/637/001637043.jpg',
					}
				]
			};
		});

		this.mockedAxiosIcaoScraper.mockImplementation(async (x) => x === this.mocked_flightroute_param ? this.scraper_html_string: this.randomHex(100));
		
	}

	async beforeAll (): Promise<void> {
		try {
			this.cleanDB();
			this.createServer();
		} catch (e) {
			// eslint-disable-next-line no-console
			console.log(e);
		}
	}

	async afterAll (): Promise<void> {
		try {
			this.closeSever();
			await this.cleanDB();
			await Redis.quit();
			Redis.disconnect();
			await postgresql.end();
		} catch (e) {
			// eslint-disable-next-line no-console
			console.log(e);
		}
	}

}
